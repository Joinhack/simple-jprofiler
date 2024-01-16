#![allow(unused)]

#[cfg(target_pointer_width = "64")]
mod stack_frame_x64;
#[cfg(target_pointer_width = "64")]
use stack_frame_x64::*;

use libc::uintptr_t;

pub struct SavedFrame<'a> {
    restore: bool,
    stack_frame: &'a mut StackFrame,
    pub pc: uintptr_t,
    pub fp: uintptr_t,
    pub sp: uintptr_t,
}

impl<'a> Drop for SavedFrame<'a> {
    fn drop(&mut self) {
        unsafe {
            if self.restore {
                self.stack_frame.restore(self.pc, self.fp, self.sp);
            }
        }
    }
}

pub struct StackFrame {
    ucontext: *const libc::ucontext_t,
    inner: StackFrameImpl,
}

impl StackFrame {
    #[inline(always)]
    pub fn new(ucontext: *const libc::ucontext_t) -> Self {
        let inner = StackFrameImpl::new(ucontext);
        Self { ucontext, inner }
    }

    pub unsafe fn save_frame(&mut self, restore: bool) -> SavedFrame {
        let mut pc = 0;
        let mut fp = 0;
        let mut sp = 0;
        if !self.ucontext.is_null() {
            pc = *self.pc();
            fp = *self.fp();
            sp = *self.sp();
        }
        SavedFrame {
            restore,
            pc,
            fp,
            sp,
            stack_frame: self,
        }   
    }

    /// restore sp, pc, fp
    pub unsafe fn restore(&mut self, pc: uintptr_t, fp: uintptr_t, sp: uintptr_t) {
        if !self.ucontext.is_null() {
            *self.pc() = pc;
            *self.fp() = fp;
            *self.sp() = sp;
        }
    }

    #[inline(always)]
    pub unsafe fn pc(&mut self) -> *mut uintptr_t {
        self.inner.pc()
    }

    #[inline(always)]
    pub unsafe fn sp(&mut self) -> *mut uintptr_t {
        self.inner.sp()
    }

    #[inline(always)]
    pub unsafe fn fp(&mut self) -> *mut uintptr_t {
        self.inner.fp()
    }

    #[inline(always)]
    pub unsafe fn retval(&mut self) -> *mut uintptr_t {
        self.inner.retval()
    }

    #[inline(always)]
    pub unsafe fn arg0(&mut self) -> *mut uintptr_t {
        self.inner.arg0()
    }

    #[inline(always)]
    pub unsafe fn arg1(&mut self) -> *mut uintptr_t {
        self.inner.arg1()
    }

    #[inline(always)]
    pub unsafe fn arg2(&mut self) -> *mut uintptr_t {
        self.inner.arg2()
    }

    #[inline(always)]
    pub unsafe fn arg3(&mut self) -> *mut uintptr_t {
        self.inner.arg3()
    }

    #[inline(always)]
    pub unsafe fn stack_at(&mut self, pos: isize) -> uintptr_t {
        self.inner.stack_at(pos)
    }
}
