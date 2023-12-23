use std::{ptr, mem, arch::asm};

pub struct OSImpl;

extern "C" {
    fn mach_port_deallocate(_: libc::c_uint, _: u32) -> libc::c_int;
    fn native_send_thread_signal(tid:u32, signal: u32) -> u32;
}

impl OSImpl {
    pub fn thread_id() -> u64 {
        unsafe {
            let port = libc::mach_thread_self();
            mach_port_deallocate(libc::mach_task_self(), port);
            port as _
        }
    }

    pub fn send_thread_alarm(tid: u32, alarm:u32) -> bool {
        unsafe {
            native_send_thread_signal(tid, alarm) == 0
        }
    }
}

#[allow(non_camel_case_types)]
type thread_array_t = *mut u32;

pub struct OSThreadListImpl {
    task: libc::task_t,
    thread_array: Option<thread_array_t>,
    thread_count: usize,
    thread_index: usize,
}

impl OSThreadListImpl {
    pub fn new() -> Self {
        Self {
            task: 0,
            thread_array: None,
            thread_count: 0,
            thread_index: 0,
        }
    }
    fn ensure_thread_array(&mut self) {
        if self.thread_array.is_none() {
            self.thread_count = 0;
            self.thread_index = 0;
            let mut thread_array = ptr::null_mut();
            let mut count = 0u32;
            unsafe {
                libc::task_threads(self.task, &mut thread_array as *mut _, &mut count);   
            }
            self.thread_array = Some(thread_array);
            self.thread_count = count as _;
        }
    }

    pub fn rewind(&mut self) {
        self.thread_array.map(|thr_arr| {
            unsafe {
                for i in 0..self.thread_count {
                    mach_port_deallocate(self.task, *thr_arr.add(i));
                }
                libc::vm_deallocate(self.task, thr_arr as _, mem::size_of::<u32>()*self.thread_count);
            }
        });
        self.thread_array.take();
    }

    pub fn next(&mut self) -> Option<u32> {
        self.ensure_thread_array();
        let idx = self.thread_index;
        if idx < self.thread_count {
            self.thread_array.map(|arr| {
                self.thread_index += 1;
                unsafe {*arr.offset(idx as _)}
            })
        } else {
            None
        }
    }

}

