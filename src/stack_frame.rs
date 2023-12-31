use std::usize;

#[cfg(target_pointer_width="64")]
mod stack_frame_x64;
#[cfg(target_pointer_width="64")]
use stack_frame_x64::*;

#[allow(non_camel_case_types)]
type uintptr_t = usize;

pub struct StackFrame {
    ucontext: *const libc::ucontext_t,
    inner: StackFrameImpl,
}

impl StackFrame {
    #[inline(always)]
    pub fn new(ucontext: *const libc::ucontext_t) -> Self {
        let inner = StackFrameImpl::new(ucontext);
        Self {
            ucontext,
            inner,
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