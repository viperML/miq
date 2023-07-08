use core::mem::MaybeUninit;
use std::ffi::OsStr;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::libc::{self};
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::sys::stat::Mode;
use tracing::warn;

#[derive(Debug)]
pub struct SemaphoreHandle<'sem> {
    pub sem: Semaphore,
    shm_name: &'sem OsStr,
}

#[derive(Debug, Clone, Copy)]
pub struct Semaphore {
    ptr: *mut libc::sem_t,
}

unsafe impl<'s> Send for Semaphore {}

impl<'sem> SemaphoreHandle<'sem> {
    pub fn new(name: &'sem OsStr) -> nix::Result<Self> {
        Ok(Self {
            sem: Semaphore::new(name)?,
            shm_name: name,
        })
    }
}

impl Semaphore {
    pub(crate) fn new(name: &OsStr) -> nix::Result<Self> {
        let fd: RawFd = nix::sys::mman::shm_open(
            // "/miq_semaphore",
            name,
            OFlag::O_RDWR | OFlag::O_CREAT,
            Mode::S_IWUSR | Mode::S_IRUSR,
        )?;
        let fd = unsafe { OwnedFd::from_raw_fd(fd.into_raw_fd()) };

        nix::unistd::ftruncate(fd.as_raw_fd(), size_of::<libc::sem_t>() as _)?;

        let addr = unsafe {
            nix::sys::mman::mmap(
                None,
                NonZeroUsize::new_unchecked(size_of::<libc::sem_t>()),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        }? as *mut libc::sem_t;

        unsafe {
            let err = nix::libc::sem_init(addr, 0, 0);
            Errno::result(err)?;
        };

        let sem = unsafe { std::mem::transmute(addr) };

        Ok(Semaphore { ptr: sem })
    }

    pub(crate) fn wait(&mut self) -> nix::Result<()> {
        loop {
            let ret = unsafe { nix::libc::sem_trywait(self.ptr) };
            let status = Errno::result(ret);
            let value = self.read()?;
            warn!(?value, ?status);
            match status {
                Ok(_) => break,
                Err(Errno::EAGAIN) => warn!("Waiting"),
                Err(other) => return Err(other),
            }
            // std::thread::sleep(Duration::from_millis(100));
        }

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
        warn!("Dropping semaphore");
        let err = unsafe { libc::sem_destroy(self.sem.ptr) };
        Errno::result(err).map_err(|_| Errno::last()).unwrap();
        nix::sys::mman::shm_unlink(self.shm_name).unwrap();
    }
}
