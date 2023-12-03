use crate::log_error;
use std::mem::MaybeUninit;
use std::ptr;

pub type SigactionFn = extern "C" fn(libc::c_int, *const libc::siginfo_t, *mut libc::c_void);

const MAX_SIGNAL_SIZE: usize = 1024;

extern "C" {
    pub fn setitimer(
        which: libc::c_int,
        new_value: *const libc::itimerval,
        old_value: *mut libc::itimerval,
    ) -> libc::c_int;
}

pub(crate) struct SignalProf {
    intervals: Vec<u32>,
    curr_interval_idx: u32,
}

impl SignalProf {
    pub fn new(min: u32, max: u32) -> Self {
        let avarage = max - min + 1;
        let intervals: Vec<u32> = (0..MAX_SIGNAL_SIZE)
            .map(|_| min + Self::random() % avarage)
            .collect();
        Self {
            intervals,
            curr_interval_idx: 0,
        }
    }

    pub fn set_action(&mut self, sfn: SigactionFn) -> bool {
        let mut sa_uninit = MaybeUninit::<libc::sigaction>::zeroed();
        let mut old_sa_uninit = MaybeUninit::<libc::sigaction>::uninit();
        let sa = unsafe { sa_uninit.assume_init_mut() };
        sa.sa_flags = (libc::SA_RESTART | libc::SA_SIGINFO) as _;
        sa.sa_sigaction = sfn as _;
        //sa.sa_mask set zero by init.
        unsafe { libc::sigaction(libc::SIGPROF, sa, old_sa_uninit.as_mut_ptr()) == 0 }
    }

    pub fn update_interval(&mut self) -> bool {
        let idx = self.curr_interval_idx as usize;
        let rs = self.update_interval_by_val(self.intervals[idx]);
        self.curr_interval_idx %= self.curr_interval_idx + 1;
        return rs;
    }

    pub(crate) fn update_interval_by_val(&mut self, interval: u32) -> bool {
        let tv_sec = (interval as i64) / 1000_000_000;
        let tv_usec = ((interval % 1000_000) as i32 % 1000) as _;
        let it_interval = libc::timeval { tv_sec, tv_usec};
        let it_value = it_interval;
        let time: libc::itimerval = libc::itimerval {
            it_interval,
            it_value,
        };
        unsafe {
            if setitimer(libc::ITIMER_PROF, &time, ptr::null_mut()) < 0 {
                log_error!("ERROR: setitimer error");
                return false;
            }
        }
        return true;
    }

    #[inline(always)]
    fn random() -> u32 {
        unsafe { libc::rand() as _ }
    }
}
