use super::uintptr_t;


macro_rules! regm {
    ($s:ident, $m: tt) => {
        (*(*$s.ucontext).uc_mcontext).__ss.$m as *mut uintptr_t
    };
}
macro_rules! regl {
    ($s:ident, $r: expr) => {
        (*$s.ucontext).uc_mcontext.gregs[$r as usize] as *mut uintptr_t
    };
}

#[cfg(target_os = "macos")]
macro_rules! reg {
    ($s:ident, pc) => {
        regm!($s, __rip)
    };
    ($s:ident, sp) => {
        regm!($s, __rsp)
    };
    ($s:ident, bp) => {
        regm!($s, __rbp)
    };
    ($s:ident, ax) => {
        regm!($s, __rax)
    };
    ($s:ident, di) => {
        regm!($s, __rdi)
    };
    ($s:ident, si) => {
        regm!($s, __rsi)
    };
    ($s:ident, dx) => {
        regm!($s, __rdx)
    };
    ($s:ident, cx) => {
        regm!($s, __rcx)
    };
}

#[cfg(target_os = "linux")]
macro_rules! reg {
    ($s:ident, pc) => {
        regl!($s, libc::REG_RIP)
    };
    ($s:ident, sp) => {
        regl!($s, libc::REG_RSP)
    };
    ($s:ident, bp) => {
        regl!($s, libc::REG_RBP)
    };
    ($s:ident, ax) => {
        regl!($s, libc::REG_RAX)
    };
    ($s:ident, di) => {
        regl!($s, libc::REG_RDI)
    };
    ($s:ident, si) => {
        regl!($s, libc::REG_RSI)
    };
    ($s:ident, dx) => {
        regl!($s, libc::REG_RDX)
    };
    ($s:ident, cx) => {
        regl!($s, libc::REG_RCX)
    };
}

pub(crate) struct StackFrameImpl {
    ucontext: *const libc::ucontext_t,
}

impl StackFrameImpl {
    #[inline(always)]
    pub fn new(ucontext: *const libc::ucontext_t) -> Self {
        Self {
            ucontext
        }
    }

    #[inline(always)]
    pub unsafe fn pc(&mut self) -> *mut uintptr_t {
        reg!(self, pc)
    }

    #[inline(always)]
    pub unsafe fn sp(&mut self) -> *mut uintptr_t {
        reg!(self, sp)
    }

    #[inline(always)]
    pub unsafe fn bp(&mut self) -> *mut uintptr_t {
        reg!(self, bp)
    }

    #[inline(always)]
    pub unsafe fn retval(&mut self) -> *mut uintptr_t {
        reg!(self, ax)
    }

    #[inline(always)]
    pub unsafe fn arg0(&mut self) -> *mut uintptr_t {
        reg!(self, di)
    }

    #[inline(always)]
    pub unsafe fn arg1(&mut self) -> *mut uintptr_t {
        reg!(self, si)
    }

    #[inline(always)]
    pub unsafe fn arg2(&mut self) -> *mut uintptr_t {
        reg!(self, dx)
    }

    #[inline(always)]
    pub unsafe fn arg3(&mut self) -> *mut uintptr_t {
        reg!(self, cx)
    }

    #[inline(always)]
    pub unsafe fn stack_at(&mut self, pos: isize) -> uintptr_t {
        *self.sp().offset(pos)
    }
}