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

#[derive(Debug)]
pub struct MemApp {
    pub fd: OwnedFd,
}

impl MemApp {
    pub fn new(name: &str, bytes: &[u8]) -> nix::Result<Self> {
        let len = bytes.len();
        let fd: RawFd = nix::sys::memfd::memfd_create(
            &CString::new(name).unwrap(),
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
            let src = addr_of!(*bytes) as _;
            std::ptr::copy_nonoverlapping(src, dst, len);
        }
        Ok(MemApp { fd })
    }
}

pub static BASH: Lazy<nix::Result<MemApp>> = Lazy::new(|| {
    let bytes = include_bytes!("../vendor/bash");
    let result = MemApp::new("bash", bytes)?;
    Ok(result)
});


pub static BUSYBOX: Lazy<nix::Result<MemApp>> = Lazy::new(|| {
    let bytes = include_bytes!("../vendor/busybox");
    let result = MemApp::new("busybox", bytes)?;
    Ok(result)
});
