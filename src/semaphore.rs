use core::mem::MaybeUninit;
use std::ffi::{CStr, CString, OsStr};
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::prelude::OsStrExt;
use std::ptr::addr_of;

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::libc::{self};
use nix::sys::memfd::MemFdCreateFlag;
use nix::sys::mman::{shm_open, MapFlags, ProtFlags};
use nix::sys::stat::Mode;
use nix::unistd::ftruncate;
use tracing::{instrument, trace, warn};

#[derive(Debug)]
pub struct SemaphoreHandle<'sem> {
    pub sem: Semaphore,
    shm_name: &'sem OsStr,
    fd: OwnedFd,
}

#[derive(Debug, Clone, Copy)]
pub struct Semaphore {
    ptr: *mut libc::sem_t,
}

unsafe impl<'s> Send for Semaphore {}
unsafe impl<'s> Sync for Semaphore {}

impl<'sem> SemaphoreHandle<'sem> {
    pub fn new(name: &'sem OsStr) -> nix::Result<Self> {
        let (sem, fd) = Semaphore::new(name)?;
        Ok(Self {
            sem,
            shm_name: name,
            fd,
        })
    }
}

impl Semaphore {
    pub(crate) fn new(name: &OsStr) -> nix::Result<(Self, OwnedFd)> {
        let len = size_of::<libc::sem_t>();

        // let fd: RawFd = nix::sys::memfd::memfd_create(
        //     &CString::new(name.as_bytes()).unwrap(),
        //     MemFdCreateFlag::MFD_CLOEXEC,
        // )?;

        let fd = shm_open(
            name,
            OFlag::O_CREAT | OFlag::O_TRUNC | OFlag::O_RDWR,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?;

        let fd = unsafe { OwnedFd::from_raw_fd(fd.into_raw_fd()) };

        ftruncate(fd.as_raw_fd(), len as _)?;

        let ptr = unsafe {
            nix::sys::mman::mmap(
                None,
                NonZeroUsize::new(len).unwrap(),
                ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )? as *mut libc::sem_t
        };

        unsafe {
            Errno::result(nix::libc::sem_init(ptr, 1, 0))?;
        }

        Ok((Semaphore { ptr }, fd))
    }

    #[instrument(ret, err, level = "warn")]
    pub(crate) fn wait(&mut self) -> nix::Result<()> {
        warn!("Start wait semaphore");
        Errno::result(unsafe { nix::libc::sem_wait(self.ptr) })?;
        Ok(())
    }

    pub(crate) fn post(&mut self) -> nix::Result<()> {
        unsafe {
            let err = nix::libc::sem_post(self.ptr);
            Errno::result(err)?;
        }
        Ok(())
    }

    pub(crate) fn read(&mut self) -> nix::Result<i32> {
        let sval = unsafe {
            let mut sval = MaybeUninit::uninit();
            let err = libc::sem_getvalue(self.ptr, sval.as_mut_ptr());
            Errno::result(err)?;
            sval.assume_init()
        };
        Ok(sval)
    }
}

impl<'a> Drop for SemaphoreHandle<'a> {
    fn drop(&mut self) {
        trace!("Dropping semaphore");
        nix::unistd::close(self.fd.as_raw_fd()).expect("Dropping memfd");
    }
}
