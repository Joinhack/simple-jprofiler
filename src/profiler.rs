use std::{ptr, mem};
use std::sync::atomic::Ordering;
use std::{sync::atomic::AtomicBool, time::Duration};

use crate::code_cache::CodeCache;
use crate::jvmti::{JNIEnv, JVMTI_THREAD_NORM_PRIORITY};
use crate::signal_prof::{SigactionFn, SignalProf};
use crate::symbol_parser::SymbolParser;
use crate::vm::JVMPICallTrace;
use crate::{c_str, log_error};
use crate::{circle_queue::CircleQueue, get_vm_mut, log_info, VM};

const DEFAULT_MIN_SIGNAL: u32 = 10;
const DEFAULT_MAX_SIGNAL: u32 = 100;

const STATUS_CHECK_PERIOD: u32 = 100;

pub const MAX_CODE_CACHE_ARRAY: u32 = 2048;

pub struct Profiler {
    sigprof: SignalProf,
    running: AtomicBool,
    queue: CircleQueue,
    code_caches: Vec<CodeCache>,
}

impl Profiler {
    pub fn new() -> Self {
        let sigprof = SignalProf::new(DEFAULT_MIN_SIGNAL, DEFAULT_MAX_SIGNAL);
        let running = AtomicBool::new(false);
        let queue = CircleQueue::new();
        Self {
            queue,
            sigprof,
            running,
            code_caches: Vec::new()
        }
    }

    #[inline(always)]
    pub fn update_symbols(&mut self) {
        SymbolParser::instance().parse_libraries(&mut self.code_caches);
    }
    
    pub fn find_lib_by_address(&self, addr: *const i8) -> Option<&'static CodeCache> {
        self.code_caches.iter()
            .find(|code_cache| code_cache.contains(addr))
            .map(|c| unsafe {
                mem::transmute(c)   
            })
    }

    pub fn start(&mut self, jni: JNIEnv) {
        self.update_symbols();
        let jthr = VM::new_java_thread(jni, c_str!("Agent Profiler Thread")).unwrap();
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
        let mut count = 0;
        loop {
            while self.queue.pop() {
                count += 1;
            }
            if count >= 200 {
                if !self.sigprof.update_interval() {
                    log_error!("ERROR: update interval error");
                    return;
                }
                count = 0;
            }
            if !self.running.load(Ordering::Relaxed) {
                while self.queue.pop() {}
                break;
            }
            self.sleep_peroid(1);
        }
    }

    #[inline]
    pub fn set_signal_action(&mut self, sa_fn: SigactionFn) -> bool {
        self.sigprof.set_action(sa_fn)
    }
}
