#[cfg(target_os = "macos")]
mod os_macos;
#[cfg(target_os = "macos")]
use os_macos::*;
#[cfg(target_os = "linux")]
mod os_linux;
#[cfg(target_os = "linux")]
use os_linux::*;

pub enum ThreadState {
    Invalid,
    Running,
    Sleeping,
}

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
    pub fn send_thread_alarm(tid: u32, alarm:u32) {
        OSImpl::send_thread_alarm(tid, alarm);
    }

    #[inline(always)]
    pub fn thread_id() -> u32 {
        OSImpl::thread_id()
    }

    pub fn thread_state(tid: u32) -> ThreadState {
        unsafe {
            OSImpl::thread_state(tid)
        }
    }
}
