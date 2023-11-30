#[cfg(target_os = "macos")]
mod os_macos;
#[cfg(target_os = "macos")]
use os_macos::*;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;

pub struct OS;

impl OS {
    pub fn thread_id() -> u32 {
        OSImpl::thread_id()
    }
}
