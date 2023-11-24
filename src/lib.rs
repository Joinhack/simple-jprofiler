mod jvmti_native;
mod jvmti;
mod signal_prof;
mod r#macro;
mod ctrl_svr;
mod profiler;
mod vm;

use std::{sync::Once, mem::MaybeUninit};
use jvmti::{JvmtiEnvPtr, JavaVMPtr};

use crate::vm::VM;
use crate::jvmti::{
    JavaVM, JVMTI_VERSION, jint
};

static AGENT_START: Once = Once::new();
static mut VM_INSTANCE: Option<VM> = None;

pub trait MaybeUninitTake<T> {
    fn take(self) -> T;
}

impl<T: Clone + Copy> MaybeUninitTake<T> for MaybeUninit<T> {
    fn take(self) -> T {
        unsafe {
            *self.as_ptr()
        }
    }
}

pub fn get_vm_mut() -> &'static mut VM {
    unsafe {
        VM_INSTANCE.as_mut().unwrap()
    }
}

pub fn get_vm() -> &'static VM {
    unsafe {
        VM_INSTANCE.as_ref().unwrap()
    }
}

pub fn set_vm(vm: VM) {
    unsafe {
        VM_INSTANCE = Some(vm);
    }
}

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "C" fn Agent_OnLoad (
    jvm: JavaVMPtr,
    option: *const libc::c_char,
    revert: *const libc::c_void,
) -> jint {
    AGENT_START.call_once(|| {
        let jvm: JavaVM = jvm.into();
        let mut jvmti = MaybeUninit::<JvmtiEnvPtr>::uninit();
        if !match jvm.get_env(&mut jvmti, JVMTI_VERSION) {
            Some(r) if r == 0 => true,
            Some(_) => false,
            None => false,
        } {
            log_error!("ERROR: get the jvmti fail");
        }
        let vm_inst = VM::new(jvm, jvmti.take().into());
        set_vm(vm_inst);
        get_vm_mut().initial();
    });
    return 0;
}