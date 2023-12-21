#[cfg(target_os = "macos")]
mod os_macos;
#[cfg(target_os = "macos")]
use os_macos::*;
#[cfg(target_os = "linux")]
mod os_linux;
#[cfg(target_os = "linux")]
use os_linux::*;

pub struct OS;

pub struct OSThreadList(OSThreadListImpl);

impl OSThreadList {
    
    pub fn new() -> Self {
        Self(OSThreadListImpl::new())
    }

    #[inline(always)]
    pub fn rewind(&mut self) {
        self.0.rewind();
    }

    #[inline(always)]
    pub fn next(&mut self) -> Option<u32> {
        self.0.next()
    }
}

impl OS {
    pub fn thread_id() -> u64 {
        OSImpl::thread_id()
    }
}
