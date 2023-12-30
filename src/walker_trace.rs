use std::{sync::atomic::{AtomicBool, Ordering}, time::Duration};

use crate::os::{OSThreadList, OS, ThreadState};

const THREAD_PER_TICKS: usize = 8;

const MIN_INTERVAL: u64 = 10_000_000;

pub struct WalkerTrace {
    running: AtomicBool,
    interval: u64,
}

impl WalkerTrace {
    pub fn new() ->  Self {
        Self {
            interval: MIN_INTERVAL,
            running: AtomicBool::new(false),
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
    }

    pub fn run(&mut self) {
        let mut thread_list = OSThreadList::new();
        let self_tid = OS::thread_id();
        self.running.store(true, Ordering::Relaxed);
        while self.running.load(Ordering::Acquire) {
            let mut count = 0;
            while count < THREAD_PER_TICKS {
                let tid = match thread_list.next() {
                    None => {
                        thread_list.rewind();
                        break;
                    }
                    Some(tid) => tid,
                };
                
                if tid == self_tid {
                    continue;
                }
                
                if let ThreadState::Running = OS::thread_state(tid) {
                    OS::send_thread_alarm(tid, libc::SIGALRM as _);
                    count += 1;
                }
            }
            let duration = Duration::from_nanos(self.interval);
            std::thread::sleep(duration);
        }
    }
}