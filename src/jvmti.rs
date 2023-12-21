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

    pub fn get_current_thread(
        &self,
        jthread: *mut jthread,
    ) -> Option<u32> {
        unsafe {
            (**self.0)
                .GetCurrentThread
                .map(|g| g(self.0, jthread as _))
        }
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
    pub fn get_class_object(
        &self,
        obj: jobject,
    ) -> Option<jclass> {
        unsafe { 
            check_null!((**self.0).GetObjectClass.map(|g| g(self.0, obj)))
         }
    }

    #[inline(always)]
    pub fn get_field_id(
        &self,
        obj: jclass,
        name: *const i8,
        sig: *const i8,
    ) -> Option<jfieldID> {
        unsafe { 
            check_null!((**self.0).GetFieldID.map(|g| g(self.0, obj, name, sig)))
         }
    }

    #[inline(always)]
    pub fn get_long_field(
        &self,
        obj: jobject,
        field: jfieldID,
    ) -> Option<jlong> {
        unsafe { 
            (**self.0).GetLongField.map(|g| g(self.0, obj, field))
         }
    }
}
