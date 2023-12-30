use std::collections::HashMap;
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::{mem, ptr};
use std::{sync::atomic::AtomicBool, time::Duration};

use crate::{c_str, code_cache::{CodeCache, CodeBlob}};
use crate::jvmti::{
    JNIEnv, JVMTI_THREAD_NORM_PRIORITY, JvmtiEnv
};
use crate::jvmti_native::{jthread, jvmtiThreadInfo};
use crate::os::OS;
use crate::signal_prof::{SigactionFn, SignalProf};
use crate::spinlock::SpinLock;
use crate::stack_walker::{StackWalker, StackContext};
use crate::symbol_parser::SymbolParser;
use crate::vm::{JVMPICallTrace, JVMPICallFrame, ASGCTCallFrameType};
use crate::vm_struct::VMThread;
use crate::walker_trace::WalkerTrace;
use crate::{circle_queue::CircleQueue, get_vm_mut, log_info, VM};

const DEFAULT_MIN_SIGNAL: u32 = 100_000_000;
const DEFAULT_MAX_SIGNAL: u32 = 500_000_000;

const STATUS_CHECK_PERIOD: u32 = 100;

pub const MAX_CODE_CACHE_ARRAY: u32 = 2048;

const CONCURRENCY_LEVEL: usize = 16;

pub const MAX_TRACE_DEEP: usize = 128;

struct ThreadInfo {
    jthread_id: u64,
    name: String,
}

pub struct Profiler {
    sigprof: SignalProf,
    running: AtomicBool,
    queue: CircleQueue,
    calltrace_buffer: Vec<Vec<JVMPICallFrame>>,
    code_caches: Vec<CodeCache>,
    walker_trace: WalkerTrace,
    locks: Vec<SpinLock>,
    jthreads: Mutex<HashMap<u64, ThreadInfo>>,
}

impl Profiler {
    pub fn new() -> Self {
        let sigprof = SignalProf::new(DEFAULT_MIN_SIGNAL, DEFAULT_MAX_SIGNAL);
        let running = AtomicBool::new(false);
        let queue = CircleQueue::new();
        let mut calltrace_buffer = Vec::new();
        let walker_trace = WalkerTrace::new();
        (0..CONCURRENCY_LEVEL)
            .for_each(|_| calltrace_buffer.push(Vec::new()));
        let locks = (0..CONCURRENCY_LEVEL)
            .map(|_| SpinLock::new())
            .collect();
        Self {
            locks,
            queue,
            sigprof,
            running,
            walker_trace,
            calltrace_buffer,
            code_caches: Vec::new(),
            jthreads: Mutex::new(HashMap::new()),
        }
    }

    #[inline(always)]
    pub fn update_symbols(&mut self, parse_kernel: bool) {
        SymbolParser::instance().parse_libraries(&mut self.code_caches, parse_kernel);
    }

    pub fn find_lib_by_address(&self, addr: *const i8) -> Option<&'static CodeCache> {
        self.code_caches
            .iter()
            .find(|code_cache| code_cache.contains(addr))
            .map(|c| unsafe { mem::transmute(c) })
    }

    pub fn start(&mut self, jni: JNIEnv) {
        if self.running.load(Ordering::Acquire) {
            return;
        }
        self.update_symbols(false);
        let jthr = VM::new_java_thread(jni, c_str!("Agent Profiler Thread")).unwrap();
        let jvmti = get_vm_mut().jvmti();
        jvmti.run_agent_thread(
            jthr,
            Some(VM::agent_profiler_run),
            ptr::null() as _,
            JVMTI_THREAD_NORM_PRIORITY as _,
        );
        self.running.store(true, Ordering::Release);
    }

    pub fn stop(&mut self) {
        log_info!("INFO: profiler stop.");
        self.sigprof.update_interval_by_val(0);
        self.walker_trace.stop();
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

    #[inline(always)]
    pub fn find_native_method(&self, pc: *const i8) ->Option<&CodeBlob> {
        self.find_library_by_address(pc)
            .and_then(|cc| cc.binary_search(pc))
    }

    #[inline(always)]
    pub fn find_library_by_address(&self, pc: *const i8) -> Option<&CodeCache> {
        self.code_caches.iter().find(|cc| cc.contains(pc))
    }

    #[inline(always)]
    pub fn push_trace(&mut self, trace: &JVMPICallTrace) {
        self.queue.push(trace);
    }

    pub(crate) fn run(&mut self) {
        log_info!("INFO: profiler start.");
        self.walker_trace.run();
    }

    pub fn get_java_async_trace(&mut self, ucontext:  *mut libc::c_void) {
        let vm = get_vm_mut();
        let tid = OS::thread_id();
        let lock_idx = self.get_lock_index(tid) as usize;
        self.locks.get(lock_idx).map(|l| {
            l.try_lock()
        });
        let mut jvmti_trace = MaybeUninit::<JVMPICallTrace>::uninit();
        let mut call_chan = [ptr::null(); MAX_TRACE_DEEP];
        let mut java_ctx = StackContext::new();
        unsafe {
            let chan = StackWalker::walk_frame(ucontext as _, &mut call_chan, &mut java_ctx);
            self.convert_native_trace(chan, lock_idx);
            (vm.asgc())(jvmti_trace.as_mut_ptr(), MAX_TRACE_DEEP as _, ucontext);
            self.push_trace(&*jvmti_trace.as_ptr());
        }
        self.locks.get(lock_idx).map(|l| {
            l.unlock()
        });
    }

    fn get_lock_index(&self, tid: u32) -> u32 {
        let mut tid = tid;
        tid ^= tid >> 8;
        tid ^= tid >> 4;
        (tid as usize % CONCURRENCY_LEVEL) as u32
    }

    fn convert_native_trace(&mut self, call_chan: &[*const ()], idx: usize) {
        let mut call_trace = call_chan.iter().filter_map(|cc| {
            let nm = self.find_native_method(*cc as _);
            nm.map(|nm| {
                println!("{}", nm.name_str());
                JVMPICallFrame {
                    bci: ASGCTCallFrameType::BCINativeFrame.into(),
                    method_id: nm.name_ptr() as _
                }
            })
        }).collect::<Vec<_>>();
        self.calltrace_buffer
            .get_mut(idx)
            .map(|buf| {
                buf.truncate(0);
                buf.append(&mut call_trace);
            });
    }

    pub unsafe fn update_thread_info(&mut self, jvmti: JvmtiEnv, jni: JNIEnv, thread: jthread) {
        VMThread::from_java_thread(&jni, thread).map(|vm_thr| {
            let jthread_id = VMThread::jthread_id(&jni, thread);
            let mut thr_info = MaybeUninit::<jvmtiThreadInfo>::uninit();
            let os_tid = vm_thr.os_thread_id();
            if os_tid > 0 {
                let info_ptr = thr_info.as_mut_ptr();
                if jvmti.get_thread_info(thread, info_ptr) == 0 {
                    let thr_name = CStr::from_ptr((*info_ptr).name);
                    let name = std::str::from_utf8_unchecked(thr_name.to_bytes()).into();
                    self.set_thread_info(vm_thr.os_thread_id() as _, ThreadInfo { 
                        jthread_id, 
                        name,
                    });
                }
            }
        });
    }

    #[inline(always)]
    fn set_thread_info(&mut self, nthrad_id:u64, thr_info: ThreadInfo) {
        let _  = self.jthreads.get_mut().map(|jthreads| {
            jthreads.insert(nthrad_id, thr_info)
        });
    }

    #[inline]
    pub fn set_signal_action(&mut self, sa_fn: SigactionFn) -> bool {
        self.sigprof.set_action(sa_fn)
    }
}
