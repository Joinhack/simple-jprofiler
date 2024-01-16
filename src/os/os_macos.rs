use std::{
    mem::{self, MaybeUninit},
    arch::asm,
    ptr
};

use super::ThreadState;

pub struct OSImpl;

extern "C" {
    fn mach_port_deallocate(_: libc::c_uint, _: u32) -> libc::c_int;
}

impl OSImpl {
    pub fn thread_id() -> u32 {
        unsafe {
            let port = libc::mach_thread_self();
            mach_port_deallocate(libc::mach_task_self(), port);
            port
        }
    }

    
    unsafe fn native_send_thread_signal(tid: u32, signal: u32) -> bool {
        #[cfg(target_arch="aarch64")]
        {
            let mut input_tid_rs = tid;
            asm!(
                "svc #0x80",
                inlateout("x0") input_tid_rs,
                in("x1") signal,
                in("x16") 328,
                options(nomem)
            );
            input_tid_rs == 0
        }
        #[cfg(not(target_arch="aarch64"))]
        {
            let mut svr = 0x2000148;
            asm!(
                "syscall",
                inlateout("ax") svr,
                in("di") tid,
                in("si") signal,
                out("cx") _,
                out("r11") _,
                options(nomem)
            );
            svr == 0
        }
    }

    pub fn send_thread_alarm(tid: u32, alarm: u32) -> bool {
        unsafe { Self::native_send_thread_signal(tid, alarm) }
    }

    pub unsafe fn thread_state(tid: u32) -> ThreadState {
        let mut info = MaybeUninit::<libc::thread_basic_info>::uninit();
        let mut info_size = mem::size_of::<libc::thread_basic_info>();
        if libc::thread_info(
            tid as _,
            libc::THREAD_BASIC_INFO as _,
            info.as_mut_ptr() as _,
            &mut info_size as *mut _ as _,
        ) != 0
        {
            return ThreadState::Invalid;
        }
        if (*info.as_ptr()).run_state == libc::TH_STATE_RUNNING {
            ThreadState::Running
        } else {
            ThreadState::Sleeping
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

impl Drop for OSThreadListImpl {
    fn drop(&mut self) {
        self.rewind();
    }
}

impl OSThreadListImpl {
    pub fn new() -> Self {
        let task = unsafe { libc::mach_task_self() };
        Self {
            task,
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
        self.thread_array.map(|thr_arr| unsafe {
            for i in 0..self.thread_count {
                mach_port_deallocate(self.task, *thr_arr.add(i));
            }
            libc::vm_deallocate(
                self.task,
                thr_arr as _,
                mem::size_of::<u32>() * self.thread_count,
            );
        });
        self.thread_array.take();
    }

    pub fn next(&mut self) -> Option<u32> {
        self.ensure_thread_array();
        let idx = self.thread_index;
        if idx < self.thread_count {
            self.thread_array.map(|arr| {
                self.thread_index += 1;
                unsafe { *arr.offset(idx as _) }
            })
        } else {
            None
        }
    }
}
