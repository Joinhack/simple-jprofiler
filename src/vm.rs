use crate::ctrl_svr::CtrlSvr;
use crate::jvmti::{JNIEnv, JNIEnvPtr, JavaVM, JvmtiEnv, JvmtiEnvPtr, JvmtiEventCallbacks, self};
use crate::jvmti_native::{
    jfieldID, jint, jmethodID, jthread, jvmtiAddrLocationMap, JVMTI_ENABLE,
    JVMTI_EVENT_COMPILED_METHOD_LOAD, JVMTI_EVENT_DYNAMIC_CODE_GENERATED, JVMTI_EVENT_THREAD_END,
    JVMTI_EVENT_THREAD_START, JVMTI_EVENT_VM_INIT, JVMTI_EVENT_CLASS_LOAD, jclass, jvmtiCapabilities, JVMTI_EVENT_CLASS_PREPARE,
};
use crate::profiler::Profiler;
use crate::vm_struct::{CodeHeap, VMStruct};
use crate::{c_str, check_null, get_vm_mut, jni_method, log_error, get_vm, cstr_2_str};
use std::mem::{self, MaybeUninit};
use std::ptr::{self, null, null_mut};

pub const JNI_VERSION_1_6: i32 = 0x00010006;
pub const JNI_EDETACHED: i32 = -2;
pub const JNI_EVERSION: i32 = -3;
pub const DEFAUTLT_CTRL_PORT: u32 = 5000;

pub const MAX_NATIVE_FRAMES: usize = 128;
pub const RESERVED_FRAMES: usize = 4;
pub const MAX_FRAMES: usize = 2048;

pub const ASGCTFAIL_TICKS_NO_CLASS_LOAD: i32 = -1;
pub const ASGCTFAIL_TICKS_GCACTIVE: i32 = -2;
pub const ASGCTFAIL_TICKS_UNKNOWN_NOT_JAVA: i32 = -3;
pub const ASGCTFAIL_TICKS_NOT_WALKABLE_JAVA: i32 = -4;
pub const ASGCTFAIL_TICKS_UNKNOWN_JAVA: i32 = -5;


pub const BCI_NATIVE_FRAME: i32 = -10;
pub const BIC_ALLOC:i32 = -11;
pub const BCI_ALLOC_OUTSIDE_TLAB: i32 = -12;
pub const BCI_LIVE_OBJECT: i32 = -13;
pub const BCI_LOCK: i32 = -14;
pub const BCI_PARK: i32 = -15;
pub const BCI_THREADID: i32 = -16;
pub const BCI_ERROR: i32 = -17;
pub const BCI_INSTRUMENT: i32 = -18;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct JVMPICallFrame {
    pub bci: jint,
    pub method_id: jmethodID,
}

impl Default for JVMPICallFrame {
    fn default() -> Self {
        Self {
            bci: 0,
            method_id: ptr::null_mut(),
        }
    }
}


#[derive(Copy, Clone)]
#[repr(C)]
pub struct JVMPICallTrace {
    // JNIEnv of the thread from which we grabbed the trace
    pub env: JNIEnvPtr,
    // < 0 if the frame isn't walkable
    pub num_frames: jint,
    // The frames, callee first.
    pub frames: *mut JVMPICallFrame,
}

impl JVMPICallTrace {
    pub fn new(env: JNIEnvPtr, frames: *mut JVMPICallFrame) -> Self {
        Self {
            env,
            frames,
            num_frames: 0,
        }
    }
}

impl Default for JVMPICallTrace {
    fn default() -> Self {
        Self {
            env: ptr::null_mut(),
            num_frames: 0,
            frames: ptr::null_mut(),
        }
    }
}

type AsgcType = unsafe extern "C" fn(*mut JVMPICallTrace, jint, *const libc::c_void);

pub struct VM {
    profiler: Profiler,
    jvmti: JvmtiEnv,
    jvm: JavaVM,
    hotspot_version: i32,
    asgc: AsgcType,
    ctrl_svr: CtrlSvr,
    vm_struct: VMStruct,
}

impl VM {
    #[inline(always)]
    pub fn jvm(&self) -> &JavaVM {
        &self.jvm
    }

    #[inline(always)]
    pub fn jvmti(&self) -> &JvmtiEnv {
        &self.jvmti
    }

    #[inline(always)]
    pub fn asgc(&self) -> AsgcType {
        self.asgc
    }

    pub(crate) fn new(jvm: JavaVM, jvmti: JvmtiEnv) -> Self {
        let profiler = Profiler::new();
        let asgc = Self::asgc_symbol();
        let ctrl_svr = CtrlSvr::new(DEFAUTLT_CTRL_PORT);
        Self {
            jvm,
            asgc,
            jvmti,
            profiler,
            ctrl_svr,
            hotspot_version: 0,
            vm_struct: VMStruct::new(),
        }
    }

    pub fn initial(&mut self, attach: bool) {
        self.profiler.update_symbols(false);
        let libjvm = self.profiler.find_lib_by_address(self.asgc as *const i8);
        self.profiler.set_signal_action(Self::prof_signal_handle);
        self.resovle_hotspot_version();
        self.vm_struct.initial(libjvm);
        //can_get_bytecodes = 1;
	    //can_get_constant_pool = 1;
	    //can_get_source_file_name = 1;
	    //can_generate_compiled_method_load_events = 1;
	    //can_generate_all_class_hook_events= 1;
        //can_retransform_classes = 1;
	    //can_retransform_any_class = 1;
	    //can_get_line_numbers = 1;
	    //can_generate_monitor_events = 1;
        let mut caps :jvmtiCapabilities = unsafe {
            std::mem::zeroed()
        };
        caps._bindgen_bitfield_1_ = 0x9c001809;
        caps._bindgen_bitfield_2_ = 0x868;
        let _ = self.jvmti.add_capabilities(&caps);
        let mut jvmti_callback: JvmtiEventCallbacks = unsafe { std::mem::zeroed() };
        jvmti_callback.VMInit = Some(Self::jvm_init);
        jvmti_callback.ClassLoad = Some(Self::jvm_class_load);
        jvmti_callback.ClassPrepare = Some(Self::jvm_class_prepare);
        jvmti_callback.ThreadStart = Some(Self::jvm_thread_start);
        jvmti_callback.ThreadEnd = Some(Self::jvm_thread_end);
        jvmti_callback.DynamicCodeGenerated = Some(Self::jvm_dynamic_code_generated);
        jvmti_callback.CompiledMethodLoad = Some(Self::jvm_compiled_method_load);
        self.jvmti
            .set_event_callbacks(
                &jvmti_callback,
                std::mem::size_of::<JvmtiEventCallbacks>() as _,
            )
            .unwrap();
        macro_rules! jvmti_enable {
            ($p: expr, $param: expr) => {
                self.jvmti
                    .set_event_notification_mode(JVMTI_ENABLE, $p, $param)
                    .unwrap();
            };
            ($p: expr) => {
                jvmti_enable!($p, ptr::null_mut())
            }
        }
        jvmti_enable!(JVMTI_EVENT_VM_INIT);
        jvmti_enable!(JVMTI_EVENT_CLASS_PREPARE);
        jvmti_enable!(JVMTI_EVENT_THREAD_START);
        jvmti_enable!(JVMTI_EVENT_THREAD_END);
        //JVMTI_EVENT_CLASS_LOAD must be enable, if this value is disable, the AsyncGetCallTrace will return -1
        jvmti_enable!(JVMTI_EVENT_CLASS_LOAD);
        jvmti_enable!(JVMTI_EVENT_COMPILED_METHOD_LOAD);

        self.jvmti.generate_events(JVMTI_EVENT_DYNAMIC_CODE_GENERATED);
        self.jvmti.generate_events(JVMTI_EVENT_COMPILED_METHOD_LOAD);

        if attach {
            let jvmti = &self.jvmti;
            let jni = self.get_jni_env().unwrap();
            self.load_all_method_ids(jvmti, &jni);
        }
    }

    unsafe extern "C" fn jvm_class_load(_jvmti: JvmtiEnvPtr, _jni: JNIEnvPtr, _thr: jthread, _class: jclass) {
        // Needed by AsyncGetCallTrace
    }

    /// load the jvm method, this useful for AsyncGetCallTrace
    /// if don't set the value, will get method_id is 0 in AsyncGetCallTrace
    fn load_method_ids(&self, jvmti: &JvmtiEnv, _jni: &JNIEnv, class: jclass) {
        let vm = get_vm();
        if vm.vm_struct.has_class_loader_data() {
            //todo!()
        }
        let mut method_count = 0;
        let mut methods: *mut jmethodID = null_mut();
        if let Some(0) = jvmti.get_class_methods(class, &mut method_count, &mut methods) {
            jvmti.deallocate(methods as _);
        }
    }

    fn load_all_method_ids(&self, jvmti: &JvmtiEnv, jni: &JNIEnv) {
        let mut count = 0;
        let mut classes = null_mut();
        if let Some(0) = jvmti.get_loaded_classes(&mut count, &mut classes) {
            for i in 0..count {
                unsafe {
                    self.load_method_ids(jvmti, jni, *classes.add(i as _));
                }
            }
            jvmti.deallocate(classes as _);
        }
    }

    unsafe extern "C" fn jvm_class_prepare(jvmti: JvmtiEnvPtr, jni: JNIEnvPtr, thr: jthread, class: jclass) {
        // Needed by AsyncGetCallTrace
        let jvmti = jvmti.into();
        let jni = jni.into();
        get_vm().load_method_ids(&jvmti, &jni, class);
    }

    unsafe extern "C" fn jvm_thread_start(jvmti: JvmtiEnvPtr, jni: JNIEnvPtr, thread: jthread) {
        get_vm_mut()
            .profiler_mut()
            .update_thread_info(jvmti.into(), jni.into(), thread);
    }

    unsafe extern "C" fn jvm_thread_end(jvmti: JvmtiEnvPtr, jni: JNIEnvPtr, thread: jthread) {
        get_vm_mut()
            .profiler_mut()
            .update_thread_info(jvmti.into(), jni.into(), thread);
    }

    unsafe extern "C" fn jvm_dynamic_code_generated(
        _jvmti: JvmtiEnvPtr,
        name: *const i8,
        address: *const libc::c_void,
        lenght: jint,
    ) {
        get_vm_mut()
            .profiler
            .add_runtime_stub(name, address as _, lenght as _);
    }

    unsafe extern "C" fn jvm_compiled_method_load(
        _jvmti: JvmtiEnvPtr,
        _method: jmethodID,
        code_size: jint,
        code_addr: *const libc::c_void,
        _map_length: jint,
        _map: *const jvmtiAddrLocationMap,
        _compile_info: *const libc::c_void,
    ) {
        get_vm_mut()
            .profiler
            .add_java_method(code_addr as _, code_size as _);
    }

    extern "C" fn jvm_init(jvmti: JvmtiEnvPtr, jni: JNIEnvPtr, _jthr: jthread) {
        let vm = get_vm_mut();
        vm.vm_struct.ready();
        let jvmti = jvmti.into();
        let jni = jni.into();
        vm.load_all_method_ids(&jvmti, &jni);
        vm.ctrl_svr.start(jni);
    }

    pub fn get_jni_env(&self) -> Option<JNIEnv> {
        let mut jni = MaybeUninit::<JNIEnvPtr>::uninit();
        let stat = self.jvm.get_env(&mut jni, JNI_VERSION_1_6);
        match stat {
            Some(JNI_EDETACHED | JNI_EVERSION) => None,
            _ => Some(unsafe { jni.assume_init() }.into()),
        }
    }

    pub extern "C" fn prof_signal_handle(
        _flags: libc::c_int,
        _info: *const libc::siginfo_t,
        ucontext: *mut libc::c_void,
    ) {
        let vm = get_vm_mut();
        vm.profiler.get_call_trace(ucontext);
    }

    pub fn hotspot_version(&self) -> i32 {
        self.hotspot_version
    }

    fn asgc_symbol() -> AsgcType {
        unsafe {
            let asgc = Self::find_so_symbol(c_str!("AsyncGetCallTrace"));
            assert!(
                asgc != ptr::null_mut(),
                "AsyncGetCallTrace not found symbol"
            );
            mem::transmute::<*const libc::c_void, AsgcType>(asgc)
        }
    }

    pub fn find_so_symbol(symbol: *const i8) -> *const libc::c_void {
        unsafe {
            libc::dlsym(libc::RTLD_DEFAULT, symbol as _)
        }
    }

    fn resovle_hotspot_version(&mut self) {
        let mut prop = null_mut();
        self.jvmti.get_system_property(c_str!("java.vm.version"), &mut prop).unwrap();
        unsafe {
            libc::printf(c_str!("%s"), prop);
        }
        let prop_str = cstr_2_str!(prop);
        if prop_str.starts_with("25.") {
            self.hotspot_version = 8;
        } else if prop_str.starts_with("24.") {
            self.hotspot_version = 7;
        } else if prop_str.starts_with("20.") {
            self.hotspot_version = 6;
        } else if let Ok(n) = i32::from_str_radix(prop_str, 10) {
            if n < 9 {
                self.hotspot_version = 9;
            }
        }
        self.jvmti.deallocate(prop);
    }

    #[inline(always)]
    pub fn profiler_mut(&mut self) -> &mut Profiler {
        &mut self.profiler
    }

    #[inline(always)]
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }

    #[inline(always)]
    pub fn start_prof(&mut self) {
        let jni = self.get_jni_env().unwrap();
        self.profiler.start(jni);
    }

    #[inline(always)]
    pub fn stop_prof(&mut self) {
        self.profiler.stop();
    }

    pub(crate) extern "C" fn ctrl_svr_start(
        _jvmti_env: JvmtiEnvPtr,
        _jni_env: JNIEnvPtr,
        _arg: *mut libc::c_void,
    ) {
        get_vm_mut().ctrl_svr.run();
    }

    pub extern "C" fn agent_profiler_run(
        _jvmti: JvmtiEnvPtr,
        _jni: JNIEnvPtr,
        _args: *mut libc::c_void,
    ) {
        let mut mask = MaybeUninit::<libc::sigset_t>::uninit();
        unsafe {
            let mask_ptr = mask.as_mut_ptr();
            libc::sigemptyset(mask_ptr);
            libc::sigaddset(mask_ptr, libc::SIGPROF);
            if libc::pthread_sigmask(libc::SIG_BLOCK, mask_ptr, ptr::null_mut()) != 0 {
                log_error!("ERROR: error block thread SIGPROF");
            }
        }
        get_vm_mut().profiler.run();
    }

    /// new java thread with thread name
    /// the thread name must be c str end with \0.
    pub fn new_java_thread(jni: JNIEnv, thr_name: *const i8) -> Option<jthread> {
        unsafe {
            (**jni.inner())
                .FindClass
                .map(|f| f(jni.inner(), c_str!("java/lang/Thread")))
        };

        let jthr_clz = match jni.find_class(c_str!("java/lang/Thread")) {
            None => {
                log_error!("ERROR: find thread class error");
                return None;
            }
            Some(c) => c,
        };

        let init_mid = match jni.get_method_id(jthr_clz, c_str!("<init>"), c_str!("()V")) {
            None => {
                log_error!("ERROR: get method id class error");
                return None;
            }
            Some(c) => c,
        };

        let jthr = match jni_method!(jni, NewObject, jthr_clz, init_mid) {
            None => return None,
            Some(obj) => obj,
        };

        if !thr_name.is_null() {
            let name = jni.new_string_utf(thr_name).unwrap();
            if let Some(set_name_mid) =
                jni.get_method_id(jthr_clz, c_str!("setName"), c_str!("(Ljava/lang/String;)V"))
            {
                jni_method!(jni, CallObjectMethod, jthr, set_name_mid, name);
            };
        }
        Some(jthr)
    }

    #[inline(always)]
    pub fn code_heap(&self) -> CodeHeap {
        self.vm_struct.code_heap()
    }

    #[inline(always)]
    pub unsafe fn update_heap_bounds(&mut self, start: *const i8, end: *const i8) {
        self.vm_struct.update_bounds(start, end);
    }

    #[inline(always)]
    pub fn eetop(&self) -> jfieldID {
        self.vm_struct.eetop()
    }

    #[inline(always)]
    pub fn tid(&self) -> jfieldID {
        self.vm_struct.tid()
    }

    #[inline(always)]
    pub fn thread_osthread_offset(&self) -> i32 {
        self.vm_struct.thread_osthread_offset()
    }

    #[inline(always)]
    pub fn osthread_id_offset(&self) -> i32 {
        self.vm_struct.osthread_id_offset()
    }

    #[inline(always)]
    pub fn has_native_thread_id(&self) -> bool {
        self.vm_struct.has_native_thread_id()
    }

    #[inline(always)]
    pub fn thread_env_offset(&self) -> i32 {
        self.vm_struct.thread_env_offset()
    }

    #[inline(always)]
    pub fn nmethod_name_offset(&self) -> i32 {
        self.vm_struct.nmethod_name_offset()
    }
}
