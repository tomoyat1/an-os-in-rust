use crate::kernel::sched;
use crate::kernel::sched::TaskHandle;
use crate::locking::spinlock::WithSpinLock;

use alloc::collections::VecDeque;
use core::sync::atomic::Ordering::{AcqRel, Acquire};
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Semaphore {
    count: AtomicUsize,
    max: usize,
    waiting: WithSpinLock<VecDeque<TaskHandle>>,
    releasing: WithSpinLock<VecDeque<TaskHandle>>,
}

impl Semaphore {
    pub const fn new(init: usize, max: usize) -> Semaphore {
        assert!(init <= max);
        Semaphore {
            count: AtomicUsize::new(init),
            max,
            waiting: WithSpinLock::new(VecDeque::new()),
            releasing: WithSpinLock::new(VecDeque::new()),
        }
    }

    pub fn wait(&self) {
        loop {
            let scheduler = sched::lock();
            let ok = self
                .count
                .try_update(AcqRel, Acquire, |u| if u > 0 { Some(u - 1) } else { None })
                .is_ok();
            if ok {
                break;
            }
            self.waiting.lock().push_back(sched::current_task());
            scheduler.block();
        }
        let releasing_task = self.releasing.lock().pop_front();
        if let Some(releasing_task) = releasing_task {
            sched::lock().wake(releasing_task);
        }
    }

    pub fn try_wait(&self) -> bool {
        if self
            .count
            .try_update(AcqRel, Acquire, |u| if u > 0 { Some(u - 1) } else { None })
            .is_err()
        {
            return false;
        }
        let releasing_task = self.releasing.lock().pop_front();
        if let Some(releasing_task) = releasing_task {
            sched::lock().wake(releasing_task);
        }
        true
    }

    pub fn signal(&self) {
        loop {
            let scheduler = sched::lock();
            let ok = self
                .count
                .try_update(AcqRel, Acquire, |u| {
                    if u < self.max {
                        Some(u + 1)
                    } else {
                        None
                    }
                })
                .is_ok();
            if ok {
                break;
            }
            self.releasing.lock().push_back(sched::current_task());
            scheduler.block();
        }
        let waiting_task = self.waiting.lock().pop_front();
        if let Some(waiting_task) = waiting_task {
            sched::lock().wake(waiting_task);
        }
    }

    pub fn try_signal(&self) -> bool {
        if self
            .count
            .try_update(AcqRel, Acquire, |u| {
                if u < self.max {
                    Some(u + 1)
                } else {
                    None
                }
            })
            .is_err()
        {
            return false;
        }
        let waiting_task = self.waiting.lock().pop_front();
        if let Some(waiting_task) = waiting_task {
            sched::lock().wake(waiting_task);
        }
        true
    }
}
