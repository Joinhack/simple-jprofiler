#![allow(unused)]
use std::{fs::{OpenOptions, ReadDir, self}, io::Read, os::unix::ffi::OsStrExt};

use super::ThreadState;


pub struct OSImpl;

impl OSImpl {
    pub fn thread_id() -> u32 {
        unsafe { libc::syscall(libc::SYS_gettid) as _ }
    }

    fn process_id() -> i32 {
        unsafe {
            libc::getpid()
        }
    }

    pub fn send_thread_alarm(tid: u32, alarm: u32) -> bool {
        unsafe {
            libc::syscall(libc::SYS_tgkill, Self::process_id(), tid, alarm) == 0
        }
    }

    pub unsafe fn thread_state(tid: u32) -> ThreadState {
        let stat_path = format!("/proc/self/task/{tid}/stat");
        let state_file = OpenOptions::new()
            .read(true)
            .open(stat_path);
        let mut state_file = match state_file {
            Ok(f) => f,
            Err(_) => return ThreadState::Invalid,
        };
        let mut value = String::new();
        if let Ok(n) = state_file.read_to_string(&mut value) {
            let bs = value.as_bytes();
            if let Some(n) = bs.iter().position(|b| *b == b')') {
                return match bs[n+2] {
                    b'R'|b'D' => ThreadState::Running,
                    _ => ThreadState::Sleeping,
                };
            } 
        }
        ThreadState::Invalid
    }
}

const TASK_PATH: &str = "/proc/self/task";

pub struct OSThreadListImpl {
    iter: ReadDir
}

impl OSThreadListImpl {
    pub fn new() -> Self {
        let iter = fs::read_dir(TASK_PATH)
            .expect("open task dir error.");
        Self {
            iter
        }
    }

    /// read the self of process info and parse thread number.
    fn thread_count(&self) -> u32 {
        let mut state_file = OpenOptions::new()
            .read(true)
            .open("/proc/self/stat")
            .expect("open stat file fail.");
        let mut value = String::new();
        if let Ok(n) = state_file.read_to_string(&mut value) {
            let bs = value[0..n].as_bytes();
            if let Some(n) = bs.iter().position(|b| *b == b')') {
                let mut space_num = 0;
                //index the 18th space after ')'
                let mut s_idx = 0;
                //index the 19th space after ')'
                let mut e_idx = 0;
                //the 18th is the value
                for i in n..bs.len() {
                    e_idx = i;
                    if bs[e_idx] == b' ' {
                        space_num += 1;
                        if space_num > 18 {
                            break;
                        }
                        s_idx = e_idx;
                    }
                }
                let thr_id = unsafe {
                    std::str::from_utf8_unchecked(&bs[s_idx+1..e_idx])
                };
                return match u32::from_str_radix(thr_id, 10) {
                    Ok(n) => n,
                    Err(_) => 0,
                };
            }
        }
        0
    }

    pub fn rewind(&mut self) {
        self.iter = fs::read_dir(TASK_PATH)
            .expect("open task dir error.");
    }

    pub fn next(&mut self) -> Option<u32> {
        while let Some(entry) = self.iter.next() {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                let bs = file_name.as_bytes();
                if bs[0] == b'.' {
                    continue;
                }
                let tid = unsafe {
                    std::str::from_utf8_unchecked(bs)
                };
                return u32::from_str_radix(tid, 10).ok();
            }
        }
        None
    }

    pub fn size(&mut self) -> u32 {
        self.thread_count()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_thread_count() {
        let thr_list = OSThreadListImpl::new();
        assert_ne!(thr_list.thread_count(), 0);
    }

    #[test]
    fn test_thread_state() {
        let mut thr_list = OSThreadListImpl::new();
        let mut running = false;
        while let Some(tid) = thr_list.next() {
            let tid_state = unsafe {
                OSImpl::thread_state(tid)
            };
            if let ThreadState::Running = tid_state {
                running = true;
            }
        }
        assert!(running);
    }
}