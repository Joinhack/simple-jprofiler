use std::{ptr, mem};

use libc::uintptr_t;

use crate::{get_vm, stack_frame::StackFrame};

const MAX_FRAME_SIZE: usize = 0x40000;
const MAX_WALK_SIZE: usize = 0x10000;
const MIN_VALID_PC: isize = 0x1000;
const FRAME_PC_SLOT: usize = 1;

pub struct StackContext {
    pub pc: *const (),
    pub sp: uintptr_t,
    pub fp: uintptr_t,
}

impl StackContext {
    pub fn new() -> Self {
        Self {
            pc: ptr::null(),
            sp: 0,
            fp: 0,
        }
    }

    pub fn set(&mut self, pc: *const (), sp: uintptr_t, fp: uintptr_t) {
        self.pc = pc;
        self.sp = sp;
        self.fp = fp;
    }
}

pub struct StackWalker;

impl StackWalker {
    pub unsafe fn walk_frame<'a>(
        ucontext: *const (), 
        call_chan:&'a mut [*const ()], 
        java_ctx: &mut StackContext) -> &'a [*const ()] {
        let sp = 0;
        let bottom = (&sp as *const _ as uintptr_t) + MAX_WALK_SIZE;
        let mut frame = StackFrame::new(ucontext as _);
        let mut pc = frame.pc() as _;
        let mut fp = frame.fp() as _;
        let sp = frame.sp() as _;
        let mut deep = 0;
        while deep < call_chan.len() {
            let vm = get_vm();
            let code_heap = vm.code_heap();
            if code_heap.code_contains(pc as _) {
                java_ctx.set(pc, sp, fp);
                break
            }
            call_chan[deep] = pc;
            deep += 1;
            
            if fp < sp || fp >= sp + MAX_FRAME_SIZE || fp >= bottom {
                break;
            }

            if fp & (mem::size_of::<uintptr_t>() - 1) != 0 {
                break;
            }

            pc = *(fp as *const *const ()).add(FRAME_PC_SLOT);
            if pc < MIN_VALID_PC as _ || pc > (-MIN_VALID_PC) as _ {
                break;
            }
            fp = *(fp as *const uintptr_t);
        }
        &call_chan[0..deep]
    }
}