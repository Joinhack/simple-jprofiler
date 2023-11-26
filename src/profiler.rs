use std::ptr;
use std::sync::atomic::Ordering;
use std::{
    sync::atomic::AtomicBool, time::Duration
};

use crate::jvmti::{JNIEnv, JVMTI_THREAD_NORM_PRIORITY};
use crate::log_error;
use crate::signal_prof::{SigactionFn, SignalProf};
use crate::vm::JVMPICallTrace;
use crate::{
    get_vm_mut, log_info, VM, circle_queue::CircleQueue
};

const DEFAULT_SIGNAL: u32 = 1;

const STATUS_CHECK_PERIOD: u32 = 100;

pub struct Profiler {
    sigprof: SignalProf,
    running: AtomicBool,
    queue: CircleQueue,
}

impl Profiler {
    pub fn new() -> Self {
        let sigprof = SignalProf::new(DEFAULT_SIGNAL, DEFAULT_SIGNAL);
        let running = AtomicBool::new(false);
        let queue = CircleQueue::new();
        Self { 
            sigprof, 
            running,
            queue
        }
    }

    pub fn start(&mut self, jni: JNIEnv) {
        let jthr = VM::new_java_thread(jni, "Agent Profiler Thread").unwrap();
        let jvmti = get_vm_mut().jvmti();
        jvmti.run_agent_thread(
            jthr,
            Some(VM::agent_profiler_run),
            ptr::null() as _,
            JVMTI_THREAD_NORM_PRIORITY as _,
        );
        self.sigprof.update_interval();
        self.running.store(true, Ordering::Release);
    }

    pub fn stop(&mut self) {
        log_info!("INFO: profiler stop.");
        self.sigprof.update_interval_by_val(0);
        self.running.store(false, Ordering::Release);
    }

    fn sleep_peroid(&self, d: u32) {
        let loop_times = d / STATUS_CHECK_PERIOD;
        let remainder = d % STATUS_CHECK_PERIOD;
        let mut count = 0;
        while count < loop_times && self.running.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_micros(STATUS_CHECK_PERIOD as _));
            count += 1;
        }
        if remainder > 0 {
            std::thread::sleep(Duration::from_micros(STATUS_CHECK_PERIOD as _));
        }
    }

    pub fn push_trace(&mut self, trace: &JVMPICallTrace) {
        self.queue.push(trace);
    }

    pub(crate) fn run(&mut self) {
        log_info!("INFO: profiler start.");
        loop {
            let mut count = 0;
            while self.queue.pop() {
                count += 1;
            }
            println!("ss");
            if count > 200 {
                if !self.sigprof.update_interval() {
                    log_error!("ERROR: update interval error");
                    return;
                }
            }
            if !self.running.load(Ordering::Relaxed) {
                
                while self.queue.pop() {
                }
            }
            println!("pop");
            self.sleep_peroid(1);
            println!("---pop");
        }
    }

    #[inline]
    pub fn set_signal_action(&mut self, sa_fn: SigactionFn) -> bool {
        self.sigprof.set_action(sa_fn)
    }
}
