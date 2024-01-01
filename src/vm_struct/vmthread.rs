use crate::{get_vm, jvmti::JNIEnv, jvmti_native::jthread};

pub struct VMThread {
    inner: *const i8,
    osthread_id_offset: i32,
    thread_osthread_offset: i32,
}

impl VMThread {
    #[inline(always)]
    pub fn from_java_thread(jni: &JNIEnv, thread: jthread) -> Option<Self> {
        let vm = get_vm();
        let eetop = vm.eetop();
        match jni.get_long_field(thread, eetop) {
            Some(p) => Some(Self {
                inner: p as _,
                osthread_id_offset: vm.osthread_id_offset(),
                thread_osthread_offset: vm.thread_osthread_offset(),
            }),
            None => None,
        }
    }

    #[inline(always)]
    pub fn inner(&self) -> *const i8 {
        self.inner
    }

    #[inline(always)]
    pub fn jthread_id(jni: &JNIEnv, thread: jthread) -> u64 {
        jni.get_long_field(thread, get_vm().tid()).unwrap() as _
    }

    #[inline(always)]
    pub unsafe fn os_thread_id(&self) -> u32 {
        let osthread = *(self.inner.offset(self.thread_osthread_offset as _) as *const *const i8);
        *(osthread.offset(self.osthread_id_offset as _) as *const u32)
    }

    #[inline(always)]
    pub unsafe fn native_thread_id(jni: &JNIEnv, jthread: jthread) -> Option<u32> {
        let vm = get_vm();
        if vm.has_native_thread_id() {
            Self::from_java_thread(jni, jthread).map(|thr| thr.os_thread_id() as _)
        } else {
            None
        }
    }

    pub unsafe fn from_jni_env(jni: &JNIEnv) -> Self {
        let vm = get_vm();
        let inner = (jni.inner() as *const i8).offset(vm.thread_env_offset() as _) as _;
        Self {
            inner,
            osthread_id_offset: vm.osthread_id_offset(),
            thread_osthread_offset: vm.thread_osthread_offset(),
        }
    }
}
