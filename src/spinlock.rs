use std::sync::atomic::{AtomicBool, Ordering};


pub struct SpinLock(AtomicBool);

impl SpinLock {
    pub fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub fn try_lock(&self) -> bool {
        match self.0.compare_exchange_weak(false, true, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn try_lock_with_guard(&self) -> Option<LockGuard> {
        if self.try_lock() {
            Some(LockGuard::new(self))
        } else {
            None
        }
    }

    pub fn unlock(&self) {
        while let Ok(_) = self.0.compare_exchange_weak(true, false, Ordering::Release, Ordering::Relaxed) {
        }
    }
}

unsafe impl Sync for SpinLock {}

unsafe impl Send for SpinLock {}

pub struct LockGuard<'a>(&'a SpinLock);

impl<'a> LockGuard<'a> {
    fn new(l: &'a SpinLock) -> Self {
        Self(l)
    }
}

impl<'a> Drop for LockGuard<'a> {
    fn drop(&mut self) {
        self.0.unlock();
    }
}
