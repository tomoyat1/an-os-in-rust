use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::cell::SyncUnsafeCell;
use core::mem::MaybeUninit;
use core::ops::Bound::Included;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

use crate::arch::x86_64::pit;
use crate::kernel::clocksource::ClockSource;

pub static CLOCK: SyncUnsafeCell<Option<Clock>> = SyncUnsafeCell::new(None);

pub struct Clock {
    // Ticks since the starting the clock in nanoseconds.
    ticks: AtomicU64,

    // (timestamp, callback) pairs to be run.
    callbacks: BTreeMap<u64, Box<dyn FnOnce(u64) + Sync>>,

    clocksource: &'static (dyn ClockSource + Send + Sync),
}

impl Clock {
    fn new(cs: &'static (dyn ClockSource + Send + Sync)) -> Self {
        Self {
            ticks: AtomicU64::new(0), // In nanoseconds
            callbacks: BTreeMap::new(),
            clocksource: cs,
        }
    }

    pub fn tick(&mut self, increment: u64) {
        self.ticks.fetch_add(increment, Relaxed);
        let now = self.ticks.load(Relaxed);
        let to_run: Vec<u64> = self
            .callbacks
            .keys()
            .copied()
            .filter(|&k| k <= now)
            .collect();
        for &k in to_run.iter() {
            if let Some(f) = self.callbacks.remove(&k) {
                f(now)
            }
        }
    }

    pub fn get_tick(&self) -> u64 {
        let now = self.clocksource.get_tick();
        self.ticks.store(now, Relaxed);
        now
    }

    /// Schedule a FnOnce to run at or after the given time, in nanoseconds.
    pub fn callback_at(&mut self, ns: u64, f: Box<dyn FnOnce(u64) + Sync>) {
        let existing = self.callbacks.get(&ns);
        self.callbacks.insert(ns, f);
    }
}

pub fn init(cs: &'static (dyn ClockSource + Send + Sync)) {
    let clock = Clock::new(cs);
    unsafe {
        CLOCK.get().as_mut().unwrap().replace(clock);
    }
}

pub fn tick_fn() -> fn(u64) {
    |x| unsafe {
        if let Some(clock) = CLOCK.get().as_mut().and_then(Option::as_mut) {
            clock.tick(x)
        }
    }
}

/// Temporary API to sleep in a busy-loop.
pub fn sleep(ms: u64) {
    let clock = unsafe { CLOCK.get().as_mut().expect("null pointer in UnsafeCell") };
    if let Some(clock) = clock {
        let (start, until) = {
            let start = unsafe { clock.ticks.load(Relaxed) };
            let until = start + ms * 1_000_000;
            (start, until)
        };
        while unsafe { clock.ticks.load(Relaxed) } < until {
            unsafe { asm!("hlt") };
        }
    } else {
        panic!("system clock is uninitialized")
    }
}
