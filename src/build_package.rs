use std::convert::Infallible;
use std::ffi::{CString, OsString};
use std::fmt::Debug;
use std::fs::create_dir_all;
use std::io::Write;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::process::Stdio;
use std::ptr::addr_of;
use std::sync::Mutex;

use async_trait::async_trait;
use color_eyre::eyre::{bail, Context};
use futures::StreamExt;
use nix::libc::uid_t;
use nix::mount::{mount, MsFlags};
use nix::sched::CloneFlags;
use nix::sys::memfd::MemFdCreateFlag;
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::unistd::{ftruncate, Gid, Pid, Uid};
use once_cell::sync::Lazy;
use tokio_process_stream::ProcessLineStream;
use tracing::{debug, error, instrument, span, warn, Level};

use crate::build::check_path;
use crate::db::DbConnection;
use crate::mem_app::{MemApp, BUSYBOX};
use crate::schema_eval::{Build, Package};
use crate::semaphore::SemaphoreHandle;
use crate::*;

const STACK_SIZE: usize = 1024 * 1024;

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
        let _build_dir = tempfile::tempdir()?;
        let build_path = _build_dir.path().to_owned();

        let _sandbox_dir = tempfile::tempdir()?;
        let sandbox_path = _sandbox_dir.path().to_owned();

        warn!(?sandbox_path, ?build_path);

        let ppid = Pid::this();

        let mut cmd = tokio::process::Command::new("/bin/bash");
        cmd.args(["--norc", "--noprofile", "/build-script"]);
        cmd.kill_on_drop(true);
        cmd.envs([
            ("HOME", "/build"),
            ("PREFIX", path.to_str().unwrap()),
            ("miq_out", path.to_str().unwrap()),
            ("TMP", "/tmp"),
            ("TEMP", "/temp"),
            ("TMPDIR", "/tmp"),
            ("TEMPDIR", "/temp"),
            ("PS1", "$PWD # "),
            ("PATH", "/usr/bin:/bin"),
        ]);
        cmd.envs(&self.env);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let _self = self.clone();
        unsafe {
            cmd.pre_exec(move || {
                // let sandbox_path = sandbox_path.clone();
                let pid = Pid::this();
                let _span = span!(Level::WARN, "child", ?pid, ?ppid);
                let _enter = _span.enter();
                let uid_outside = Uid::current();
                let uid_inside: uid_t = 0;
                let uid_count = 1;
                let uid_map_contents = format!("{} {} {}", uid_inside, uid_outside, uid_count);

                let gid_inside: uid_t = 0;
                let gid_outside = Gid::current();
                let gid_count = 1;
                let gid_map_contents = format!("{} {} {}", gid_inside, gid_outside, gid_count);

                nix::sched::unshare(
                    CloneFlags::CLONE_NEWUSER
                        | CloneFlags::CLONE_NEWNET
                        | CloneFlags::CLONE_FS
                        | CloneFlags::CLONE_NEWNS,
                )?;

                let mut uid_map = std::fs::File::create(format!("/proc/{}/uid_map", pid))?;
                uid_map.write_all(&uid_map_contents.as_bytes())?;

                let mut setgroups = std::fs::File::create(format!("/proc/{}/setgroups", pid))?;
                setgroups.write_all("deny".as_bytes())?;

                let mut gid_map = std::fs::File::create(format!("/proc/{}/gid_map", pid))?;
                gid_map.write_all(&gid_map_contents.as_bytes())?;

                let bash = mem_app::BASH
                    .as_ref()
                    .map_err(|e| Into::<io::Error>::into(*e))?;
                let busybox = mem_app::BASH
                    .as_ref()
                    .map_err(|e| Into::<io::Error>::into(*e))?;

                _self.sandbox_setup(bash, busybox, &sandbox_path, &build_path)?;

                warn!("DONE");

                Ok(())
            })
        };

        let child = cmd.spawn()?;

        let log_file_path = format!("/miq/log/{}.log", self.result.deref());
        let err_msg = format!("Creating logfile at {}", log_file_path);
        let mut log_file = std::fs::File::create(log_file_path).wrap_err(err_msg)?;

        let mut procstream = ProcessLineStream::try_from(child)?;
        while let Some(item) = procstream.next().await {
            use owo_colors::OwoColorize;
            match item {
                tokio_process_stream::Item::Stdout(line) => {
                    let msg = format!("{}>>{}", self.name.blue(), line.bright_black());
                    println!("{}", msg);
                    log_file.write_all(line.as_bytes())?;
                    log_file.write_all(b"\n")?;
                }
                tokio_process_stream::Item::Stderr(line) => {
                    let msg = format!("{}>>{}", self.name.blue(), line.bright_black());
                    println!("{}", msg);
                    log_file.write_all(line.as_bytes())?;
                    log_file.write_all(b"\n")?;
                }
                tokio_process_stream::Item::Done(Ok(exit)) => {
                    if exit.success() {
                        debug!("Build OK");
                    } else {
                        bail!(eyre!("Exit not successful").wrap_err(exit));
                    }
                }
                tokio_process_stream::Item::Done(Err(exit)) => bail!(exit),
            }
        }

        match path.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", path),
            Err(e) => bail!(e),
        }

        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}

const NONE_NIX: Option<&str> = None;

impl Package {
    #[tracing::instrument(ret, err, level = "debug")]
    fn sandbox_setup(
        &self,
        bash: &MemApp,
        busybox: &MemApp,
        sandbox_path: &Path,
        build_path: &Path,
    ) -> nix::Result<()> {
        let uid = Uid::effective();
        let gid = Gid::effective();
        let my_pid = Pid::this();
        warn!(?self, ?uid, ?gid);

        mount(
            Some(sandbox_path),
            sandbox_path,
            NONE_NIX,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            NONE_NIX,
        )
        .unwrap();

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
            )?;
            std::os::unix::fs::symlink(
                format!("/proc/{}/fd/{}", my_pid, bash.fd.as_raw_fd()),
                bin_path.join("bash"),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("/bin/bash"), bin_path.join("sh")).unwrap();
        }

        {
            let usr_path = sandbox_path.join("usr").join("bin");
            std::fs::create_dir_all(&usr_path).unwrap();
            mount(
                NONE_NIX,
                &usr_path,
                Some("tmpfs"),
                MsFlags::empty(),
                NONE_NIX,
            )
            .unwrap();
            std::os::unix::fs::symlink(
                format!("/proc/{}/fd/{}", my_pid, busybox.fd.as_raw_fd()),
                usr_path.join("busybox"),
            )
            .unwrap();

            for applet in crate::busybox::BUSYBOX_APPLETS {
                std::os::unix::fs::symlink(format!("/usr/bin/busybox"), usr_path.join(applet))
                    .unwrap();
            }
        }

        // pivot root
        {
            nix::unistd::chdir(sandbox_path)?;
            nix::unistd::pivot_root(sandbox_path, sandbox_path)?;
            nix::unistd::chdir("/build")?;
        }

        std::fs::write("/build-script", &self.script).unwrap();

        Ok(())
    }
}
