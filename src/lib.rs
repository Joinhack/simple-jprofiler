mod vm;
mod circle_queue;
mod code_cache;
mod ctrl_svr;
mod dwarf;
mod jvmti;
mod jvmti_native;
mod r#macro;
mod os;
mod profiler;
mod signal_prof;
mod spinlock;
mod stack_frame;
mod stack_walker;
mod symbol_parser;
mod frame_name;
mod vm_struct;
mod walker_trace;

use jvmti::{JavaVMPtr, JvmtiEnvPtr};
use std::{mem::MaybeUninit, sync::Once};

use crate::jvmti::{jint, JavaVM, JVMTI_VERSION};
use crate::vm::VM;

static AGENT_START: Once = Once::new();
static mut VM_INSTANCE: Option<VM> = None;

#[inline(always)]
pub fn get_vm_mut() -> &'static mut VM {
    unsafe { VM_INSTANCE.as_mut().unwrap() }
}

#[inline(always)]
pub fn get_vm() -> &'static VM {
    unsafe { VM_INSTANCE.as_ref().unwrap() }
}

pub fn set_vm(vm: VM) {
    unsafe {
        VM_INSTANCE = Some(vm);
    }
}

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "C" fn Agent_OnLoad(
    jvm: JavaVMPtr,
    option: *const libc::c_char,
    revert: *const libc::c_void,
) -> jint {
    let tid = os::OS::thread_id();
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
        let vm_inst = VM::new(jvm, unsafe { jvmti.assume_init() }.into());
        set_vm(vm_inst);
        get_vm_mut().initial(false);
    });
    return 0;
}
