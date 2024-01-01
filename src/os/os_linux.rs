pub struct OSImpl;

impl OSImpl {
    pub fn thread_id() -> u32 {
        unsafe { libc::syscall(libc::SYS_gettid) }
    }
}
