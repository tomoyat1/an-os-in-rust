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
    pub fn new(max: usize) -> Semaphore {
        Semaphore {
            count: AtomicUsize::new(0),
            max,
            waiting: WithSpinLock::new(VecDeque::new()),
            releasing: WithSpinLock::new(VecDeque::new()),
        }
    }

    pub fn wait(&self) {
        while self
            .count
            .try_update(AcqRel, Acquire, |u| if u > 0 { Some(u - 1) } else { None })
            .is_err()
        {
            self.waiting.lock().push_back(sched::current_task());
            sched::lock().block();
        }
        if let Some(releasing_task) = self.releasing.lock().pop_front() {
            sched::lock().wake(releasing_task);
        }
    }

    pub fn release(&self) {
        while !self.try_release() {
            self.releasing.lock().push_back(sched::current_task());
            sched::lock().block();
        }
    }

    /// Releases without blocking. Returns `false` if the semaphore is already at `max`.
    pub fn try_release(&self) -> bool {
        if self
            .count
            .try_update(AcqRel, Acquire, |u| if u < self.max { Some(u + 1) } else { None })
            .is_err()
        {
            return false;
        }
        if let Some(waiting_task) = self.waiting.lock().pop_front() {
            sched::lock().wake(waiting_task);
        }
        true
    }
}
