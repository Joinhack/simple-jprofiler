pub struct OSImpl;

extern "C" {
    fn mach_port_deallocate(_:libc::c_uint, _: u32) -> libc::c_int;
}

impl OSImpl {
    pub fn thread_id() -> u32  {
        unsafe {
            let port = libc::mach_thread_self();
            mach_port_deallocate(
                libc::mach_task_self(), 
                port
            );
            port
        }
    }
}