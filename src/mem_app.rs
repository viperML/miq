use std::ffi::CString;
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::ptr::addr_of;

use nix::sys::memfd::MemFdCreateFlag;
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::unistd::ftruncate;
use once_cell::sync::Lazy;

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
                ProtFlags::PROT_WRITE | ProtFlags::PROT_READ,
                MapFlags::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )?;
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
