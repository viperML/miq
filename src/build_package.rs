use std::io::Write;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use color_eyre::eyre::{bail, Context};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use nix::libc::uid_t;
use nix::mount::{mount, MsFlags};
use nix::sched::CloneFlags;
use nix::unistd::{Gid, Pid, Uid};
use tokio::io::copy;
use tokio_process_stream::{Item, ProcessLineStream};
use tracing::{debug, span, trace, Level};

use crate::db::DbConnection;
use crate::mem_app::MemApp;
use crate::schema_eval::{Build, Package};
use crate::*;

const BUILD_SCRIPT_LOC: &str = "/build-script";

#[async_trait]
impl Build for Package {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(
        &self,
        rebuild: bool,
        conn: &Mutex<DbConnection>,
        pb: ProgressBar,
    ) -> Result<()> {
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

        pb.set_message(self.name.clone());
        pb.set_style(ProgressStyle::with_template("{msg:.blue}>> {spinner}")?);
        pb.enable_steady_tick(Duration::from_millis(500));

        crate::build::clean_path(&path)?;
        let _build_dir = tempfile::tempdir()?;
        let build_path = _build_dir.path().to_owned();

        let _sandbox_dir = tempfile::tempdir()?;
        let sandbox_path = _sandbox_dir.path().to_owned();

        trace!(?sandbox_path, ?build_path);

        let ppid = Pid::this();

        let mut cmd = tokio::process::Command::new("/bin/bash");
        cmd.args(["--norc", "--noprofile"]);
        cmd.arg(BUILD_SCRIPT_LOC);

        cmd.env_clear();
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

        cmd.kill_on_drop(true);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let _self = self.clone();
        unsafe {
            cmd.pre_exec(move || {
                let pid = Pid::this();
                let _span = span!(Level::DEBUG, "child", ?pid, ?ppid);
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

                let busybox = mem_app::BUSYBOX
                    .as_ref()
                    .map_err(|e| Into::<io::Error>::into(*e))?;

                _self.sandbox_setup(bash, busybox, &sandbox_path, &build_path)?;

                debug!("pre_exec done");
                Ok(())
            })
        };

        let child = cmd.spawn()?;

        let log_file_path = format!("/miq/log/{}.log", self.result.deref());
        let log_file = tokio::fs::File::create(&log_file_path).await?;
        let mut log_writer = tokio::io::BufWriter::new(log_file);

        let mut procstream = ProcessLineStream::try_from(child)?;
        while let Some(item) = procstream.next().await {
            use owo_colors::OwoColorize;
            let msg = match item {
                Item::Stdout(line) | Item::Stderr(line) => line,
                Item::Done(Ok(exit)) => {
                    if exit.success() {
                        format!("miq: exit ok")
                    } else {
                        bail!(eyre!("Exit not successful").wrap_err(exit));
                    }
                }
                Item::Done(Err(exit)) => bail!(exit),
            };
            let pretty = format!("{}>>{}", self.name.blue(), msg.bright_black());
            copy(&mut msg.as_bytes(), &mut log_writer).await?;
            copy(&mut "\n".as_bytes(), &mut log_writer).await?;
            pb.println(&pretty);
            pb.tick();
        }

        match path.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", path),
            Err(e) => bail!(e),
        }

        conn.lock().unwrap().add(&path)?;
        pb.finish_and_clear();
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
        trace!(?self, ?uid, ?gid);

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
