use std::convert::Infallible;
use std::ffi::CString;
use std::fmt::Debug;
use std::io::Write;
use std::num::NonZeroUsize;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::ptr::addr_of;
use std::sync::Mutex;

use async_trait::async_trait;
use color_eyre::eyre::{bail, Context};
use nix::libc::uid_t;
use nix::mount::{mount, MsFlags};
use nix::sched::CloneFlags;
use nix::sys::memfd::MemFdCreateFlag;
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::unistd::{ftruncate, Gid, Pid, Uid};
use once_cell::sync::Lazy;
use tracing::{error, warn, Level};

use crate::db::DbConnection;
use crate::schema_eval::{Build, Package};
use crate::semaphore::SemaphoreHandle;
use crate::*;

const STACK_SIZE: usize = 1024 * 1024;

const BASH_BYTES: &[u8] = include_bytes!("../vendor/bash");

#[derive(Debug)]
struct MemApp {
    fd: OwnedFd,
}

static BASH: Lazy<nix::Result<MemApp>> = Lazy::new(|| {
    let len = BASH_BYTES.len();
    let fd: RawFd = nix::sys::memfd::memfd_create(
        &CString::new("bash").unwrap(),
        MemFdCreateFlag::empty(), // MemFdCreateFlag::MFD_CLOEXEC,
    )?;
    let fd = unsafe { OwnedFd::from_raw_fd(fd.into_raw_fd()) };

    ftruncate(fd.as_raw_fd(), len as _)?;

    unsafe {
        let dst = nix::sys::mman::mmap(
            None,
            NonZeroUsize::new(len).unwrap(),
            ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )?;
        info!(?dst);
        let src = addr_of!(*BASH_BYTES) as _;
        std::ptr::copy_nonoverlapping(src, dst, len);
    }
    Ok(MemApp { fd })
});

#[async_trait]
impl Build for Package {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<()> {
        let path = self.result.store_path();
        let path = path.as_path();
        let _path_str = path.to_str().unwrap();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(());
            }
        }

        crate::build::clean_path(&path)?;
        std::fs::create_dir(&path)?;
        let _build_dir = tempfile::tempdir()?;
        let build_path = _build_dir.path();

        let _sandbox_dir = tempfile::tempdir()?;
        let sandbox_path = _sandbox_dir.path();

        warn!(?sandbox_path, ?build_path);

        let mut child_stack = Vec::with_capacity(STACK_SIZE);
        unsafe { child_stack.set_len(STACK_SIZE) };
        let ref mut child_stack = child_stack.into_boxed_slice();

        let shm_path = PathBuf::from(format!("/miq-semaphore-{}", &self.name));
        let shm_path = shm_path.as_os_str();

        let mut semaphore = SemaphoreHandle::new(&shm_path)?;
        warn!("{:?}", semaphore);

        let bash = BASH.as_ref()?;

        let child_pid = nix::sched::clone(
            Box::new(move || {
                let pid = Pid::this();
                let _span = tracing::span!(Level::WARN, "child", ?pid);
                let _enter = _span.enter();

                warn!("{:?}", semaphore.sem);
                semaphore.sem.wait().unwrap();

                let res = self.build_sandbox(&bash, sandbox_path, build_path);
                match res {
                    Ok(_) => unreachable!(),
                    Err(e) => {
                        error!(?e);
                        -1
                    }
                }
            }),
            child_stack,
            CloneFlags::empty()
                | CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET,
            Some(nix::libc::SIGCHLD),
        )?;

        // Set UID/GID mappings
        {
            let uid_inside: uid_t = 0;
            let uid_outside = Uid::current();
            let uid_count = 1;
            let uid_map_contents = format!("{} {} {}", uid_inside, uid_outside, uid_count);
            let mut uid_map = std::fs::File::create(format!("/proc/{}/uid_map", child_pid))?;
            uid_map.write_all(&uid_map_contents.as_bytes())?;

            let mut f = std::fs::File::create(format!("/proc/{}/setgroups", child_pid))?;
            f.write_all("deny".as_bytes())?;

            let gid_inside: uid_t = 0;
            let gid_outside = Gid::current();
            let gid_count = 1;
            let gid_map_contents = format!("{} {} {}", gid_inside, gid_outside, gid_count);
            let mut gid_map = std::fs::File::create(format!("/proc/{}/gid_map", child_pid))?;
            gid_map.write_all(&gid_map_contents.as_bytes())?;

            semaphore.sem.post()?;
            warn!("Semaphore written!: {:?}", semaphore.sem.read());
        }

        let pidfd = async_pidfd::AsyncPidFd::from_pid(child_pid.as_raw())?;
        let exit = pidfd.wait().await?.status();
        warn!(?exit);

        if !exit.success() {
            bail!(exit);
        } else {
            info!("Return OK");
        }

        match path.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", path),
            Err(e) => bail!(e),
        }

        // bail!("TODO");
        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}

const NONE_NIX: Option<&str> = None;

impl Package {
    #[tracing::instrument(ret, err, level = "debug")]
    fn build_sandbox(
        &self,
        bash: &MemApp,
        sandbox_path: &Path,
        build_path: &Path,
    ) -> nix::Result<Infallible> {
        let uid = Uid::effective();
        let my_pid = Pid::this();
        warn!(?self, ?uid);

        mount(
            Some(sandbox_path),
            sandbox_path,
            NONE_NIX,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            NONE_NIX,
        )?;

        for element in ["dev", "etc", "run", "tmp", "var", "sys", "miq", "proc"] {
            let new_path = sandbox_path.join(element);
            std::fs::create_dir(&new_path).unwrap();
            mount(
                Some(&Path::new("/").join(element)),
                &new_path,
                NONE_NIX,
                MsFlags::MS_BIND | MsFlags::MS_REC,
                NONE_NIX,
            )?;
        }

        {
            let new_path = sandbox_path.join("build");
            std::fs::create_dir(&new_path).unwrap();
            mount(
                Some(build_path),
                &new_path,
                NONE_NIX,
                MsFlags::MS_BIND | MsFlags::MS_REC,
                NONE_NIX,
            )?;
        }

        {
            let bin_path = sandbox_path.join("bin");
            std::fs::create_dir(&bin_path).unwrap();
            mount(
                NONE_NIX,
                &bin_path,
                Some("tmpfs"),
                MsFlags::empty(),
                NONE_NIX,
            ).unwrap();
            let original_bash = format!("/proc/{}/fd/{}", my_pid, bash.fd.as_raw_fd());
            let symlink_bash = bin_path.join("sh");
            std::os::unix::fs::symlink(original_bash, symlink_bash).unwrap();
        }

        // pivot root
        {
            nix::unistd::chdir(sandbox_path)?;
            nix::unistd::pivot_root(sandbox_path, sandbox_path)?;
            nix::unistd::chdir("/build")?;
        }

        std::fs::write("/build-script", &self.script).unwrap();

        // exec bash
        {
            let argv = [
                "bash", //  "-c", "set"
                "--norc",
                "--noprofile",
                "/build-script",
                // "-c",
                // "set -x && exit 69",
            ]
            .into_iter()
            .map(|e| CString::new(e).unwrap())
            .collect::<Vec<_>>();

            let mut envp = [
                "HOME=/build",
                &format!("PREFIX={}", &self.result.store_path().to_string_lossy()),
                &format!("miq_out={}", &self.result.store_path().to_string_lossy()),
                "TMP=/tmp",
                "TEMP=/temp",
                "TMPDIR=/tmp",
                "TEMPDIR=/temp",
                "PS1=$PWD # ",
            ]
            .into_iter()
            .map(|e| CString::new(e).unwrap())
            .collect::<Vec<_>>();

            for (key, value) in &self.env {
                let elem = format!("{}={}", key, value);
                let elem = CString::new(elem).unwrap();
                envp.push(elem);
            }

            nix::unistd::fexecve(bash.fd.as_raw_fd(), &argv, &envp)
        }
    }
}
