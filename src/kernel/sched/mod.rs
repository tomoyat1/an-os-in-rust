use crate::arch::x86_64::hpet;
use crate::kernel::clock;
use crate::kernel::sched::task::{TaskInfo, TaskList};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use crate::panic;

use alloc::boxed::Box;
use alloc::vec::Vec;

use core::arch::asm;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::sync::atomic::Ordering::Relaxed;
use core::{mem, ptr};

mod task;
pub(crate) use task::current_task;

const SCHED_LATENCY: u64 = 20_000_000; // 20 ms.

extern "C" {
    fn _do_switch(
        from: *const task::Task,
        to: *const task::Task,
        scheduler: *mut c_void,
    ) -> *mut c_void;
}

pub struct Scheduler {
    task_list: TaskList,
}

static SCHEDULER: WithSpinLock<Scheduler> = WithSpinLock::new(Scheduler {
    task_list: TaskList::new(),
});

impl<'a> WithSpinLockGuard<'a, Scheduler> {
    pub(crate) fn switch(mut self) {
        // Hardcode use of HPET for now.
        // TODO: abstract clocksources and get time from the trait object.
        let now = hpet::get_time();

        let mut switch_from = self.task_list.current_task().unwrap();

        self.task_list.update_runtime(switch_from, now);

        // If we have exhausted runnable tasks, pick the kernel idle task (task 0).
        let switch_to = self
            .task_list
            .next()
            .unwrap_or(self.task_list.get(0).expect("Kernel task 0 must exist."));
        self.task_list.set_current_task(switch_to.into(), now);
        self.task_list.set_run_until(switch_to, now);

        let switch_from = self.task_list.get_ptr(switch_from);
        let switch_to = self.task_list.get_ptr(switch_to);

        let mut scheduler = ManuallyDrop::new(self);
        unsafe {
            let mut scheduler =
                ptr::read(
                    _do_switch(switch_from, switch_to, &raw mut scheduler as *mut c_void)
                        as *mut ManuallyDrop<WithSpinLockGuard<Scheduler>>,
                );
            ManuallyDrop::drop(&mut scheduler);
        };
    }

    pub(crate) fn new_task(&mut self) -> task::TaskHandle {
        self.task_list.new_task()
    }

    pub(crate) fn current_task(&self) -> task::TaskHandle {
        let t = self
            .task_list
            .current_task()
            .expect("No tasks found. Maybe this is called before boot task initialization?");
        t
    }

    pub(crate) fn sleep(mut self, ms: u64) {
        let clock = unsafe {
            clock::CLOCK
                .get()
                .as_mut()
                .expect("null pointer in UnsafeCell")
        };
        if let Some(clock) = clock {
            let (start, until) = {
                let start = clock.get_tick();
                let until = start + ms * 1_000_000;
                (start, until)
            };
            let current_task = {
                let task = self
                    .task_list
                    .current_task()
                    .expect("Current task must exist");
                self.task_list.set_runnable(task, false);
                task
            };
            clock.callback_at(
                until,
                Box::new(move |x| {
                    SCHEDULER.lock().task_list.set_runnable(current_task, true);
                }),
            );

            self.switch()
        } else {
            panic!("system clock is uninitialized");
        }
    }
}

pub(crate) fn init() {
    let mut handle = SCHEDULER.lock();
    handle.task_list.init_idle_task();

    // Switch into the idle task.
    handle.switch()
}

pub(crate) fn lock() -> WithSpinLockGuard<'static, Scheduler> {
    SCHEDULER.lock()
}

#[unsafe(no_mangle)]
extern "C" fn check_runtime() {
    let now = hpet::get_time();
    let mut handle = SCHEDULER.lock();
    let current = handle.current_task();
    if handle.task_list.get_run_until(current) <= now {
        handle.switch()
    }
}
