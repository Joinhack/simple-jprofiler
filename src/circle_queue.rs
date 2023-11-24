use std::{sync::atomic::{AtomicUsize, AtomicBool, Ordering}, time::Duration};

use crate::{vm::{JVMPICallTrace, JVMPICallFrame}};

const HOLDER_SIZE: usize = 1024;
const FRAME_SIZE: usize = 1024;

#[derive(Default)]
pub struct CallTraceHolder {
    pub trace: JVMPICallTrace,
    pub is_commit: AtomicBool,
}

impl CallTraceHolder {
    #[inline(always)]
    pub fn new(holder: JVMPICallTrace) -> Self {
        Self { 
            trace: holder, 
            is_commit: AtomicBool::new(false)
        }
    }
}

struct CircleQueue {
    i_idx: AtomicUsize,
    o_idx: AtomicUsize,
    holder_mem: Vec<CallTraceHolder>,
    frame_mem: Vec<Vec<JVMPICallFrame>>,
}

impl CircleQueue {
    pub fn new() -> Self {
        let i_idx = AtomicUsize::new(0);
        let o_idx = AtomicUsize::new(0);
        let holder_mem = (0..HOLDER_SIZE)
            .map(|_| CallTraceHolder::default())
            .collect::<Vec<_>>();
        let frame_mem = (0..HOLDER_SIZE)
            .map(|_| {
                (0..FRAME_SIZE)
                    .map(|_| JVMPICallFrame::default())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        Self {
            i_idx,
            o_idx,
            holder_mem,
            frame_mem,
        }
    }

    pub fn advice(i: usize) -> usize {
        return (i+1) % HOLDER_SIZE;
    }

    pub fn push(&mut self, trace: JVMPICallTrace) -> bool {
        let hodler = CallTraceHolder::new(trace);
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
        self.holder_mem[i_idx] = hodler;
        self.holder_mem[i_idx].is_commit.store(true, Ordering::Release);
        true
    }

    pub fn pop(&mut self) -> bool {
        let o_idx = self.o_idx.load(Ordering::Relaxed);
        let i_idx = self.i_idx.load(Ordering::Relaxed);
        if o_idx == i_idx {
            return false;
        }
        while !self.holder_mem[i_idx].is_commit.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_micros(1));
        }
        
        self.holder_mem[o_idx].is_commit.store(false, Ordering::Release);
        self.o_idx.store(Self::advice(o_idx), Ordering::Relaxed);

        true
    }
}