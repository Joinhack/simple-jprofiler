use crate::ctrl_svr::CtrlSvr;
use crate::jvmti::{
    JNIEnv, JNIEnvPtr, JavaVM, JvmtiEnv, JvmtiEnvPtr, JvmtiEventCallbacks
};
use crate::jvmti_native::{
    jint, jmethodID, jthread, JVMTI_ENABLE, JVMTI_EVENT_VM_INIT
};
use crate::profiler::Profiler;
use crate::{
    c_str, check_null, get_vm, get_vm_mut, jni_method, log_error, MaybeUninitTake
};
use std::mem::{self, MaybeUninit};
use std::ptr;

pub const JNI_VERSION_1_6: i32 = 0x00010006;
pub const JNI_EDETACHED: i32 = -2;
pub const JNI_EVERSION: i32 = -3;
pub const MAX_TRACE_DEEP: u32 = 128;
pub const DEFAUTLT_CTRL_PORT: u32 = 5000;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct JVMPICallFrame {
    pub lineno: jint,
    pub method_id: jmethodID,
}

impl Default for JVMPICallFrame {
    fn default() -> Self {
        Self { 
            lineno: 0, 
            method_id: ptr::null_mut() 
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct JVMPICallTrace {
    // JNIEnv of the thread from which we grabbed the trace
    pub env: JNIEnvPtr,
    // < 0 if the frame isn't walkable
    pub num_frames: jint,
    // The frames, callee first.
    pub frames: *mut JVMPICallFrame,
}

impl Default for JVMPICallTrace {
    fn default() -> Self {
        Self { 
            env: ptr::null_mut(), 
            num_frames: 0, 
            frames: ptr::null_mut() 
        }
    }
}

type AsgcType = unsafe extern "C" fn(*mut JVMPICallTrace, jint, *const libc::c_void);

pub struct VM {
    profiler: Profiler,
    jvmti: JvmtiEnv,
    jvm: JavaVM,
    asgc: AsgcType,
    ctrl_svr: CtrlSvr,
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
        }
    }

    pub fn initial(&mut self) {
        self.profiler.set_signal_action(Self::prof_signal_handle);
        let mut jvmti_callback: JvmtiEventCallbacks = unsafe { std::mem::zeroed() };
        jvmti_callback.VMInit = Some(Self::vm_init);
        self.jvmti
            .set_event_callbacks(
                &jvmti_callback,
                std::mem::size_of::<JvmtiEventCallbacks>() as _,
            )
            .unwrap();
        self.jvmti
            .set_event_notification_mode(JVMTI_ENABLE, JVMTI_EVENT_VM_INIT, ptr::null_mut())
            .unwrap();
    }

    extern "C" fn vm_init(_jvmti: JvmtiEnvPtr, jni: JNIEnvPtr, _jthr: jthread) {
        get_vm_mut().ctrl_svr.start(jni.into());
    }

    pub fn get_jni_env(&self) -> Option<JNIEnv> {
        let mut jni = MaybeUninit::<JNIEnvPtr>::uninit();
        let stat = self.jvm.get_env(&mut jni, JNI_VERSION_1_6);
        match stat {
            Some(JNI_EDETACHED | JNI_EVERSION) => None,
            _ => Some(jni.take().into()),
        }
    }

    pub extern "C" fn prof_signal_handle(
        _flags: libc::c_int,
        _info: *const libc::siginfo_t,
        ucontext: *mut libc::c_void,
    ) {
        let vm = get_vm_mut();
        let mut jvmti_trace = MaybeUninit::<JVMPICallTrace>::uninit();
        unsafe {
            (vm.asgc)(jvmti_trace.as_mut_ptr(), MAX_TRACE_DEEP as _, ucontext);
            vm.profiler.push_trace(&*jvmti_trace.as_ptr());
        }
    }

    fn asgc_symbol() -> AsgcType {
        unsafe {
            let asgc = libc::dlsym(libc::RTLD_DEFAULT, c_str!("AsyncGetCallTrace"));
            assert!(
                asgc != ptr::null_mut(),
                "AsyncGetCallTrace not found symbol"
            );
            mem::transmute::<*mut libc::c_void, AsgcType>(asgc)
        }
    }

    #[inline(always)]
    pub fn profiler_mut(&mut self) -> &mut Profiler {
        &mut self.profiler
    }

    #[inline(always)]
    pub fn profiler(&mut self) -> &Profiler {
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

    pub fn new_java_thread(jni: JNIEnv, thr_name: &str) -> Option<jthread> {
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

        if thr_name != "" {
            let thr_name = format!("{thr_name}\0");
            let name = jni.new_string_utf(thr_name.as_ptr() as _).unwrap();
            if let Some(set_name_mid) =
                jni.get_method_id(jthr_clz, c_str!("setName"), c_str!("(Ljava/lang/String;)V"))
            {
                jni_method!(jni, CallObjectMethod, jthr, set_name_mid, name);
            };
        }
        Some(jthr)
    }
}
