use crate::{
    check_null,
    jvmti_native::{self, *},
};
use std::{ffi::c_char, mem::MaybeUninit};

pub use jvmti_native::{
    jclass, jint, jthread, jvmtiStartFunction, JVMTI_THREAD_NORM_PRIORITY, JVMTI_VERSION,
};

pub type JavaVMPtr = *mut jvmti_native::JavaVM;

pub type JvmtiEnvPtr = *mut jvmti_native::jvmtiEnv;

pub type JNIEnvPtr = *mut jvmti_native::JNIEnv;

pub type JvmtiEventCallbacks = jvmtiEventCallbacks;

impl From<JavaVMPtr> for JavaVM {
    #[inline(always)]
    fn from(value: JavaVMPtr) -> Self {
        Self(value)
    }
}

impl From<JvmtiEnvPtr> for JvmtiEnv {
    #[inline(always)]
    fn from(value: JvmtiEnvPtr) -> Self {
        Self(value)
    }
}

impl From<JNIEnvPtr> for JNIEnv {
    #[inline(always)]
    fn from(value: JNIEnvPtr) -> Self {
        Self(value)
    }
}

pub struct JavaVM(JavaVMPtr);

impl JavaVM {
    #[inline(always)]
    pub fn get_env<T>(&self, penv: &mut MaybeUninit<T>, version: i32) -> Option<i32> {
        unsafe {
            let jvm = self.0;
            (**jvm)
                .GetEnv
                .map(|get_env| get_env(jvm, penv.as_mut_ptr() as _, version))
        }
    }
}

#[derive(Debug)]
pub struct JvmtiEnv(JvmtiEnvPtr);

impl JvmtiEnv {
    #[inline(always)]
    pub fn run_agent_thread(
        &self,
        jthr: jthread,
        thr_cb: jvmtiStartFunction,
        args: *const libc::c_void,
        priority: i32,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .RunAgentThread
                .map(|r| r(self.0, jthr, thr_cb, args as _, priority))
        }
    }

    pub fn set_event_callbacks(
        &self,
        callbacks: &JvmtiEventCallbacks,
        size_of_callbacks: i32,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .SetEventCallbacks
                .map(|c| c(self.0, callbacks, size_of_callbacks))
        }
    }

    pub fn add_capabilities(
        &self,
        caps: &jvmtiCapabilities,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .AddCapabilities
                .map(|c| c(self.0, caps as _))
        }
    }

    pub fn generate_events(
        &self,
        event: jvmtiEvent,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GenerateEvents
                .map(|c| c(self.0, event))
        }
    }

    pub fn get_method_name(
        &self,
        method_id: jmethodID,
        name_ptr: *mut *mut libc::c_char,
        sign_ptr: *mut *mut libc::c_char,
        gen_ptr: *mut *mut libc::c_char,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetMethodName
                .map(|c| c(self.0, method_id, name_ptr, sign_ptr, gen_ptr))
        }
    }

    pub fn get_method_declaring_class(
        &self,
        method_id: jmethodID,
        declaring_class_ptr: *mut jclass,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetMethodDeclaringClass
                .map(|c| c(self.0, method_id, declaring_class_ptr))
        }
    }

    pub fn get_class_signature(
        &self,
        class: jclass,
        name_ptr: *mut *mut i8,
        generic_ptr: *mut *mut i8,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetClassSignature
                .map(|c| c(self.0, class, name_ptr, generic_ptr))
        }
    }

    pub fn deallocate(&self, p: *const i8) -> u32 {
        unsafe { (**self.0).Deallocate.map(|d| d(self.0, p as _)).unwrap() }
    }

    pub fn get_current_thread(&self, jthread: *mut jthread) -> Option<u32> {
        unsafe { (**self.0).GetCurrentThread.map(|g| g(self.0, jthread as _)) }
    }

    pub fn set_event_notification_mode(
        &self,
        mode: u32,
        event_type: u32,
        event_thread: jthread,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .SetEventNotificationMode
                .map(|s| s(self.0, mode, event_type, event_thread))
        }
    }

    pub fn get_system_property(
        &self,
        name: *const i8,
        prop: *mut *mut i8,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetSystemProperty
                .map(|s| s(self.0, name, prop))
        }
    }


    pub fn get_loaded_classes(
        &self,
        count: *mut jint,
        class: *mut *mut jclass,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetLoadedClasses
                .map(|s| s(self.0, count, class))
        }
    }

    pub fn get_class_methods(
        &self,
        class: jclass,
        count: *mut jint,
        method: *mut *mut jmethodID,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetClassMethods
                .map(|s| s(self.0, class, count, method))
        }
    }

    pub fn get_thread_info(&self, thr: jthread, thread_info: *mut jvmtiThreadInfo) -> i32 {
        unsafe {
            match (**self.0)
                .GetThreadInfo
                .map(|g| g(self.0, thr, thread_info))
            {
                Some(o) => o as _,
                _ => -1,
            }
        }
    }
}

#[derive(Debug)]
pub struct JNIEnv(JNIEnvPtr);

impl JNIEnv {
    #[inline(always)]
    pub(crate) fn inner(&self) -> JNIEnvPtr {
        self.0
    }

    #[inline(always)]
    pub fn find_class(&self, clz: *const c_char) -> Option<jclass> {
        unsafe { check_null!((**self.0).FindClass.map(|f| f(self.0, clz))) }
    }

    pub fn new_string_utf(&self, name: *const c_char) -> Option<jstring> {
        unsafe { check_null!((**self.0).NewStringUTF.map(|f| f(self.0, name))) }
    }

    #[inline(always)]
    pub fn get_method_id(
        &self,
        clz: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> Option<jmethodID> {
        unsafe { check_null!((**self.0).GetMethodID.map(|f| f(self.0, clz, name, sig))) }
    }

    #[inline(always)]
    pub fn get_class_object(&self, obj: jobject) -> Option<jclass> {
        unsafe { check_null!((**self.0).GetObjectClass.map(|g| g(self.0, obj))) }
    }

    #[inline(always)]
    pub fn get_field_id(&self, obj: jclass, name: *const i8, sig: *const i8) -> Option<jfieldID> {
        unsafe { check_null!((**self.0).GetFieldID.map(|g| g(self.0, obj, name, sig))) }
    }

    #[inline(always)]
    pub fn get_long_field(&self, obj: jobject, field: jfieldID) -> Option<jlong> {
        unsafe { (**self.0).GetLongField.map(|g| g(self.0, obj, field)) }
    }
}
