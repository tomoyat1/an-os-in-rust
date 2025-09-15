use crate::arch::x86_64::pit;
use core::arch::asm;
use core::cell::SyncUnsafeCell;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

pub static CLOCK: SyncUnsafeCell<Clock> = SyncUnsafeCell::new(Clock::new());

pub struct Clock {
    // Ticks since the starting the clock in nanoseconds.
    ticks: AtomicU64,
}

impl Clock {
    const fn new() -> Self {
        Self {
            ticks: AtomicU64::new(0), // In nanoseconds
        }
    }

    pub fn tick(&mut self, increment: u64) {
        self.ticks.fetch_add(increment, Relaxed);
    }
}

pub fn tick_fn() -> fn(u64) {
    |x| unsafe {
        if let Some(clock) = CLOCK.get().as_mut() {
            clock.tick(x)
        }
    }
}

/// Temporary API to sleep in a busy-loop.
pub fn sleep(ms: u64) {
    let clock = CLOCK.get();
    let (start, until) = {
        let start = unsafe { (*clock).ticks.load(Relaxed) };
        let until = start + ms * 1_000_000;
        (start, until)
    };
    while unsafe { (*clock).ticks.load(Relaxed) } < until {
        unsafe { asm!("hlt") };
    }
}
