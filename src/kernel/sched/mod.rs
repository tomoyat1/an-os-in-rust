use crate::arch::x86_64::hpet;
use crate::arch::x86_64::interrupt::{disable_interrupts, enable_interrupts, interrupts_enabled};
use crate::kernel::clock;
use crate::kernel::sched::task::TaskList;

use alloc::boxed::Box;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr;

mod task;
pub(crate) use crate::some_task;
pub(crate) use task::current_task;
pub(crate) use task::TaskHandle;

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

static SCHEDULER: spin::Mutex<Scheduler> = spin::Mutex::new(Scheduler {
    task_list: TaskList::new(),
});

/// Guard for the scheduler lock with the ability to restore the interrupt flag based on the
/// state that is stored in the Task.
pub(crate) struct SchedulerGuard<'a> {
    inner: spin::MutexGuard<'a, Scheduler>,
}

impl Deref for SchedulerGuard<'_> {
    type Target = Scheduler;

    fn deref(&self) -> &Scheduler {
        &self.inner
    }
}

impl DerefMut for SchedulerGuard<'_> {
    fn deref_mut(&mut self) -> &mut Scheduler {
        &mut self.inner
    }
}

impl Drop for SchedulerGuard<'_> {
    fn drop(&mut self) {
        let was_enabled = task::resume_interrupts();
        unsafe {
            // Release the lock before restoring interrupts.
            ptr::drop_in_place(&mut self.inner);
            if was_enabled {
                enable_interrupts()
            }
        }
    }
}

impl<'a> SchedulerGuard<'a> {
    pub(crate) fn switch(mut self) {
        // Hardcode use of HPET for now.
        // TODO: abstract clocksources and get time from the trait object.
        let now = hpet::get_time();

        let switch_from = current_task();

        self.task_list.update_runtime(switch_from, now);

        // If we have exhausted runnable tasks, pick the kernel idle task (task 0).
        let switch_to = self
            .task_list
            .next()
            .unwrap_or_else(|| self.task_list.get(0).expect("Kernel task 0 must exist."));
        self.task_list.set_current_task(switch_to, now);
        self.task_list.set_run_until(switch_to, now);

        let switch_from = self.task_list.get_ptr(switch_from);
        let switch_to = self.task_list.get_ptr(switch_to);

        let mut scheduler = ManuallyDrop::new(self);
        unsafe {
            let mut scheduler = ptr::read(_do_switch(
                switch_from,
                switch_to,
                &raw mut scheduler as *mut c_void,
            ) as *mut ManuallyDrop<SchedulerGuard>);
            ManuallyDrop::drop(&mut scheduler);
        };
    }

    pub(crate) fn new_task(&mut self, entry: fn()) -> TaskHandle {
        self.task_list.new_task(entry)
    }

    pub(crate) fn sleep(mut self, ns: u64) {
        let clock = unsafe {
            clock::CLOCK
                .get()
                .as_mut()
                .expect("null pointer in UnsafeCell")
        };
        if let Some(clock) = clock {
            let until = clock.get_tick() + ns;
            let current_task = {
                let task = current_task();
                self.task_list.set_runnable(task, false);
                task
            };
            clock.callback_at(
                until,
                Box::new(move |_| {
                    SCHEDULER.lock().task_list.set_runnable(current_task, true);
                }),
            );

            self.switch()
        } else {
            panic!("system clock is uninitialized");
        }
    }

    /// Wakes the specified Task.
    pub(crate) fn wake(&mut self, task: TaskHandle) {
        self.task_list.set_runnable(task, true);
    }

    /// Blocks the currently running Task.
    pub(crate) fn block(mut self) {
        self.task_list.set_runnable(current_task(), false);
        self.switch()
    }
}

pub(crate) fn init() {
    let mut handle = lock();
    handle.task_list.init_idle_task();

    // Switch into the idle task.
    handle.switch()
}

pub(crate) fn lock() -> SchedulerGuard<'static> {
    let was_enabled = interrupts_enabled();
    disable_interrupts();
    task::set_resume_interrupts(was_enabled);
    SchedulerGuard {
        inner: SCHEDULER.lock(),
    }
}

#[unsafe(no_mangle)]
extern "C" fn check_runtime() {
    let now = hpet::get_time();
    let scheduler = lock();
    if scheduler.task_list.get_run_until(current_task()) <= now {
        scheduler.switch()
    }
}
