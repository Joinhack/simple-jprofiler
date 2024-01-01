use std::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock(AtomicBool);

impl SpinLock {
    pub fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    #[inline(always)]
    pub fn try_lock(&self) -> bool {
        self.0
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline(always)]
    pub fn lock(&self) -> Option<LockGuard> {
        while !self.try_lock() {}
        Some(LockGuard::new(self))
    }

    #[inline(always)]
    pub fn try_lock_with_guard(&self) -> Option<LockGuard> {
        if self.try_lock() {
            Some(LockGuard::new(self))
        } else {
            None
        }
    }

    pub fn unlock(&self) {
        self.0.store(false, Ordering::Release);
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
