use core::panic;
use std::{
    io::{BufRead, BufReader},
    os::{
        fd::{AsFd, AsRawFd},
        unix::process::CommandExt,
    },
    path::PathBuf,
    process::{exit, Command},
};

use libc::{prctl, PR_SET_PDEATHSIG, SIGKILL};
use nix::{
    mount::{mount, MsFlags},
    sched::{unshare, CloneFlags},
    sys::wait::{waitpid, WaitStatus},
    unistd::fork,
};
use nix::{unistd::pivot_root, NixPath};
use tempfile::tempdir;
use tracing::{debug, info};

use color_eyre::{eyre::bail, Result};

#[derive(Debug)]
pub struct SandBox {}

static NONE_STR: Option<&'static str> = None;

impl SandBox {
    pub fn run(&self, cmd: &mut Command) -> Result<()> {
        let (pipe_reader, pipe_writer) = os_pipe::pipe()?;

        let workdir_handle = tempdir()?;
        let workdir = workdir_handle.path();
        debug!("workdir={:?}", workdir_handle);

        // let newroot = workdir.join("newroot");
        // debug!("newroot={:?}", newroot);

        unsafe {
            prctl(PR_SET_PDEATHSIG, SIGKILL);
        }

        match unsafe { fork() }? {
            nix::unistd::ForkResult::Parent { child } => {
                drop(pipe_writer);
                let reader = BufReader::new(pipe_reader);

                for line in reader.lines() {
                    let line = line?;
                    eprintln!("{}", line);
                }

                let status = waitpid(child, None);
                info!("Child died: {:?}", status);

                if let Ok(WaitStatus::Exited(_, 0)) = status {
                    return Ok(());
                } else {
                    // TODO make this prettier
                    bail!("Bad exit: {:?}", status);
                }
            }
            nix::unistd::ForkResult::Child => {
                drop(pipe_reader);
                nix::unistd::dup2(pipe_writer.as_fd().as_raw_fd(), nix::libc::STDOUT_FILENO)?;
                nix::unistd::dup2(pipe_writer.as_fd().as_raw_fd(), nix::libc::STDERR_FILENO)?;

                println!("Setting up workdir...");

                // TODO: sandbox
                // Rebind a lot of folders like /dev into newroot

                // unshare(CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS)
                //     .context("While unshare the process")?;

                // mount(
                //     Some(workdir),
                //     workdir,
                //     NONE_STR,
                //     MsFlags::MS_PRIVATE | MsFlags::MS_BIND,
                //     NONE_STR,
                // )
                // .context("While re-mounting the workdir")?;

                std::env::set_current_dir(workdir)?;

                println!("Workdir ready");

                let result = cmd.exec();

                // Only run if execution is abnormal, otherwise process is transfered
                bail!(result);
            }
        }
    }
}
