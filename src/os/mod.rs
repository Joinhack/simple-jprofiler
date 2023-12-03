#[cfg(target_os = "macos")]
mod os_macos;
#[cfg(target_os = "macos")]
use os_macos::*;
#[cfg(target_os = "linux")]
mod os_linux;
#[cfg(target_os = "linux")]
use os_linux::*;

pub struct OS;

impl OS {
    pub fn thread_id() -> u64 {
        OSImpl::thread_id()
    }
}
