use std::{net::TcpListener, os::fd::AsRawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::mem;
use std::io::{Read};
use std::ptr;

use crate::jvmti::{JNIEnv, JVMTI_THREAD_NORM_PRIORITY};
use crate::{get_vm_mut, log_info};
use crate::vm::VM;

pub struct CtrlSvr {
    listener: TcpListener,
    running: AtomicBool,
}

impl CtrlSvr {
    pub fn new(port: u32) -> Self {
        let addr = format!("0.0.0.0:{port}");
        let listener = TcpListener::bind(addr).unwrap();
        let raw_fd = listener.as_raw_fd();
        let running = AtomicBool::new(false);
        unsafe {
            let opt: libc::c_int = 1;
            let opt_ptr = &opt as *const libc::c_int;
            let opt_len = mem::size_of_val(&opt);
            libc::setsockopt(
                raw_fd, 
                libc::SOL_SOCKET, 
                libc::SO_REUSEADDR, 
                opt_ptr as _, 
                opt_len as _
            );
        }
        Self {
            running,
            listener
        }
    }

    pub fn start(&mut self, jni: JNIEnv) {
        let jthr = VM::new_java_thread(jni, "Agent Controller Thread").unwrap();
        let jvmti = get_vm_mut().jvmti();
        jvmti.run_agent_thread(jthr, Some(VM::ctrl_svr_start), ptr::null() as _, JVMTI_THREAD_NORM_PRIORITY as _);
    }

    pub fn run(&mut self) {
        log_info!("INFO: control svr start.");
        self.running.store(true, Ordering::Relaxed);
        let mut buf = [0u8; 1024];
        while self.running.load(Ordering::Relaxed) {
            let (mut peer_stream, _) = self.listener.accept().unwrap();
            while let Ok(n) = peer_stream.read(&mut buf) {
                let cmd = unsafe {
                    std::str::from_utf8_unchecked(&buf[0..n])
                };
                if cmd.starts_with("start") {
                    get_vm_mut().start_prof()
                }

                if cmd.starts_with("stop") {
                    get_vm_mut().stop_prof()
                }

                if cmd.starts_with("quit") {
                    break;
                }
            }
        }
    }

}