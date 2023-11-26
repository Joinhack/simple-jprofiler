use std::{
    sync::atomic::{
        AtomicUsize, AtomicBool, Ordering
    }, 
    time::Duration, 
    alloc::{self, Layout}
};

use crate::vm::{JVMPICallTrace, JVMPICallFrame};

const HOLDER_SIZE: usize = 1024;
const FRAME_SIZE: usize = 1024;

#[derive(Default)]
pub struct CallTraceHolder {
    pub trace: JVMPICallTrace,
    pub is_commit: AtomicBool,
}

impl CallTraceHolder {
    #[inline(always)]
    pub fn new(holder: &JVMPICallTrace) -> Self {
        Self { 
            trace: *holder, 
            is_commit: AtomicBool::new(false)
        }
    }
}

pub struct CircleQueue {
    i_idx: AtomicUsize,
    o_idx: AtomicUsize,
    holders: *mut CallTraceHolder,
    frames: *mut [JVMPICallFrame; FRAME_SIZE],
}

impl CircleQueue {
    pub fn new() -> Self {
        let i_idx = AtomicUsize::new(0);
        let o_idx = AtomicUsize::new(0);
        let holders: *mut CallTraceHolder = Self::array_ptr(HOLDER_SIZE);
        (0..HOLDER_SIZE).for_each(|i| {
            unsafe {
                *holders.add(i) = CallTraceHolder::default();
            }
        });
        let frames = Self::frames_ptr();
        Self {
            i_idx,
            o_idx,
            holders,
            frames,
        }
    }

    fn frames_ptr() -> *mut [JVMPICallFrame; FRAME_SIZE] {
        Self::array_ptr::<_>(HOLDER_SIZE)
    }

    fn array_ptr<T>(size: usize) -> *mut T {
        let layout = Layout::array::<T>(size).unwrap();
        unsafe {
            alloc::alloc(layout) as _
        }
    }

    #[inline(always)]
    pub fn advice(i: usize) -> usize {
        return (i+1) % HOLDER_SIZE;
    }

    #[inline(always)]
    fn holders(&self, i: usize) -> &CallTraceHolder {
        unsafe {
            &*self.holders.add(i)
        }
    }

    #[inline(always)]
    fn holders_mut(&mut self, i: usize) -> &mut CallTraceHolder {
        unsafe {
            &mut *self.holders.add(i)   
        }
    }

    #[inline(always)]
    fn frames(&self, i: usize) -> &[JVMPICallFrame; FRAME_SIZE] {
        unsafe {
            &*self.frames.add(i)   
        }
    }

    #[inline(always)]
    fn frames_mut(&self, i: usize) -> &mut [JVMPICallFrame; FRAME_SIZE] {
        unsafe {
            &mut *self.frames.add(i)   
        }
    }

    #[inline(always)]
    fn write_handle(&mut self, idx: usize, holder:CallTraceHolder) {
        let holder_mut = self.holders_mut(idx);
        *holder_mut = holder;
    }

    pub fn push(&mut self, trace: &JVMPICallTrace) -> bool {
        let holder = CallTraceHolder::new(&trace);
        let mut i_idx;
        let mut next_i_idx;
        let mut o_idx;
        loop {
            i_idx = self.i_idx.load(Ordering::Relaxed);
            o_idx = self.o_idx.load(Ordering::Relaxed);
            next_i_idx = Self::advice(i_idx);
            if o_idx == next_i_idx {
                return false;
            }
            if let Ok(_) = self.i_idx.compare_exchange_weak(
                i_idx, 
                next_i_idx, 
                Ordering::Relaxed, 
                Ordering::Relaxed
            ) {
                break;
            }
        }
        self.write_handle(i_idx, holder);
        self.holders_mut(i_idx).is_commit.store(true, Ordering::Release);
        true
    }

    pub fn pop(&mut self) -> bool {
        let o_idx = self.o_idx.load(Ordering::Relaxed);
        let i_idx = self.i_idx.load(Ordering::Relaxed);
        if o_idx == i_idx {
            return false;
        }
        while !self.holders(o_idx).is_commit.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_micros(1));
        }
        self.holders(o_idx).is_commit.store(false, Ordering::Release);
        self.o_idx.store(Self::advice(o_idx), Ordering::Relaxed);
        true
    }
}