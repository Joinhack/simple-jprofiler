use core::num;
use std::collections::HashMap;
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::{mem, ptr};
use std::sync::atomic::AtomicBool;

use crate::cstr_2_str;
use crate::frame_name::FrameName;
use crate::jvmti::{JNIEnv, JvmtiEnv, JVMTI_THREAD_NORM_PRIORITY};
use crate::jvmti_native::{jthread, jvmtiThreadInfo, jmethodID};
use crate::os::OS;
use crate::signal_prof::{SigactionFn, SignalProf};
use crate::spinlock::SpinLock;
use crate::stack_frame::StackFrame;
use crate::stack_walker::{StackContext, StackWalker};
use crate::symbol_parser::SymbolParser;
use crate::vm::{
    JVMPICallFrame, JVMPICallTrace, MAX_FRAMES, MAX_NATIVE_FRAMES, RESERVED_FRAMES, BCI_THREADID, BCI_NATIVE_FRAME
};
use crate::vm_struct::VMThread;
use crate::walker_trace::WalkerTrace;
use crate::{
    c_str,
    code_cache::{CodeBlob, CodeCache},
};
use crate::{circle_queue::CircleQueue, get_vm_mut, log_info, VM};

const DEFAULT_MIN_SIGNAL: u32 = 100_000_000;
const DEFAULT_MAX_SIGNAL: u32 = 500_000_000;

pub const MAX_CODE_CACHE_ARRAY: u32 = 2048;

const CONCURRENCY_LEVEL: usize = 16;

pub struct ThreadInfo {
    pub jthread_id: u64,
    pub name: String,
}

pub struct Profiler {
    sigprof: SignalProf,
    running: AtomicBool,
    queue: CircleQueue,
    calltrace_buffer: Vec<Vec<JVMPICallFrame>>,
    code_caches: Vec<CodeCache>,
    walker_trace: WalkerTrace,
    locks: Vec<SpinLock>,
    stub_lock: SpinLock,
    runtime_stub: CodeCache,
    call_stub_begin: *const i8,
    call_stub_end: *const i8,
    max_frames: usize,
    jthreads: Mutex<HashMap<u64, ThreadInfo>>,
}

impl Profiler {
    pub fn new() -> Self {
        let sigprof = SignalProf::new(DEFAULT_MIN_SIGNAL, DEFAULT_MAX_SIGNAL);
        let running = AtomicBool::new(false);
        let queue = CircleQueue::new();
        let mut calltrace_buffer = Vec::new();
        let walker_trace = WalkerTrace::new();
        let max_frames = MAX_FRAMES;
        let deeps = max_frames + MAX_NATIVE_FRAMES + RESERVED_FRAMES;
        (0..CONCURRENCY_LEVEL).for_each(
            |_| calltrace_buffer.push(vec![Default::default(); deeps])
        );
        let locks = (0..CONCURRENCY_LEVEL).map(
            |_| SpinLock::new()
        ).collect();
        let runtime_stub = CodeCache::new(c_str!("[stubs]"), -1 as _);
        Self {
            locks,
            queue,
            sigprof,
            running,
            walker_trace,
            max_frames,
            runtime_stub,
            call_stub_begin: ptr::null(),
            call_stub_end: ptr::null(),
            calltrace_buffer,
            code_caches: Vec::new(),
            stub_lock: SpinLock::new(),
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
        self.walker_trace.stop();
        self.running.store(false, Ordering::Release);
    }

    pub unsafe fn add_runtime_stub(&mut self, name: *const i8, address: *const i8, len: u32) {
        self.stub_lock.lock().map(|_| {
            self.runtime_stub.add(address, len as _, name, true);
        });
        let name_str = cstr_2_str!(name);
        if name_str == "call_stub" {
            self.call_stub_begin = address;
            self.call_stub_end = address.add(len as _);
        }
        get_vm_mut().update_heap_bounds(address, address.add(len as _));
    }

    pub unsafe fn add_java_method(&mut self, address: *const i8, len: u32) {
        get_vm_mut().update_heap_bounds(address, address.add(len as _));
    }

    #[inline(always)]
    pub fn find_native_method(&self, pc: *const i8) -> Option<&CodeBlob> {
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

    pub fn get_call_trace(&mut self, ucontext: *mut libc::c_void) {
        let tid = OS::thread_id();
        let lock_idx = self.get_lock_index(tid) as usize;
        self.locks.get(lock_idx).map(|l| l.try_lock());
        
        let mut java_ctx = StackContext::new();
        unsafe {
            let frame_buff = self
                .calltrace_buffer
                .get_mut(lock_idx)
                .expect("get idx calltrace buffer fail");
            let frame_buf_ptr = frame_buff.as_mut_ptr();
            let num_frames = self.get_native_trace(ucontext, frame_buf_ptr, &mut java_ctx);
            let num_frames = num_frames + self.get_java_async_trace(ucontext, frame_buf_ptr.add(num_frames));
            let num_frames = num_frames + self.make_frame(frame_buf_ptr.add(num_frames), BCI_THREADID, tid as _);
            self.walk_frames(frame_buf_ptr, num_frames);
        }
        self.locks.get(lock_idx).map(|l| l.unlock());
    }

    unsafe fn make_frame(
        &mut self, 
        frame_buf_ptr: *mut JVMPICallFrame,
        frame_type: i32,
        methid: jmethodID
    ) -> usize {
        (*frame_buf_ptr).bci = frame_type.into();
        (*frame_buf_ptr).method_id = methid;
        1
    }

    /// walk the native call trace
    #[inline]
    unsafe fn get_native_trace(
        &mut self,
        ucontext: *mut libc::c_void, 
        frame_buf_ptr: *mut JVMPICallFrame,
        java_ctx: &mut StackContext,
    ) -> usize {
        let mut call_chan = [ptr::null(); MAX_NATIVE_FRAMES];
        let chan = StackWalker::walk_frame(ucontext as _, &mut call_chan, java_ctx);
        self.convert_native_trace(chan, frame_buf_ptr)
    }

    /// get the java async trace, call the AsyncGetCallTrace
    unsafe fn get_java_async_trace(
        &mut self, 
        ucontext: *mut libc::c_void, 
        frame_buf_ptr: *mut JVMPICallFrame
    ) -> usize {
        let vm = get_vm_mut();
        let jni = vm.get_jni_env();
        if jni.is_none() {
            //Not java thread.
            return 0;
        }
        let mut curr_frame = StackFrame::new(ucontext as _);
        // when exit the block will restore the pc, sp, fp by drop
        let _saved_frame = curr_frame.save_frame(true);
        let jni = jni.unwrap();
        let mut call_trace = JVMPICallTrace::new(jni.inner(), frame_buf_ptr);
        //call AsyncGetCallTrace.
        (vm.asgc())(&mut call_trace as *mut _, self.max_frames as _, ucontext);
        if call_trace.num_frames > 0 {
            return call_trace.num_frames as _;
        }
        0
    }

    fn get_lock_index(&self, tid: u32) -> u32 {
        let mut tid = tid;
        tid ^= tid >> 8;
        tid ^= tid >> 4;
        (tid as usize % CONCURRENCY_LEVEL) as u32
    }

    unsafe fn convert_native_trace(
        &mut self, 
        call_chan: &[*const ()], 
        frame_buf_ptr: *mut JVMPICallFrame
    ) -> usize {
        let mut prev_call = ptr::null();
        let mut num_frames = 0;
        for cc in call_chan {
            let nm = self.find_native_method(*cc as _);
            if let Some(nm) = nm {
                prev_call = nm as _;
                let frame_ptr = frame_buf_ptr.add(num_frames);
                num_frames += 1;
                (*frame_ptr).bci = BCI_NATIVE_FRAME;
                (*frame_ptr).method_id = nm as *const _  as _;
            }
        }
        num_frames
    }

    unsafe fn walk_frames(&self, frame_buf_ptr: *mut JVMPICallFrame, nums: usize) {
        let mut frame_name = FrameName::new(&self.jthreads);
        for idx in 0..nums {
            let name = frame_name.name(&(*frame_buf_ptr.add(idx)));
            println!("{name}");
        }
        println!("----------------------------------------------");
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
                    self.set_thread_info(
                        vm_thr.os_thread_id() as _,
                        ThreadInfo { jthread_id, name },
                    );
                }
            }
        });
    }

    #[inline(always)]
    fn set_thread_info(&mut self, nthrad_id: u64, thr_info: ThreadInfo) {
        let _ = self
            .jthreads
            .get_mut()
            .map(|jthreads| jthreads.insert(nthrad_id, thr_info));
    }

    #[inline]
    pub fn set_signal_action(&mut self, sa_fn: SigactionFn) -> bool {
        self.sigprof.set_action(sa_fn)
    }
}
