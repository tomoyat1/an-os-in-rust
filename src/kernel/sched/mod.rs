use crate::arch::x86_64::hpet;
use crate::kernel::clock;
use crate::kernel::sched::task::{TaskInfo, TaskList};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::sync::atomic::Ordering::Relaxed;
use core::{mem, ptr};

mod task;

extern "C" {
    fn _do_switch(
        from: *const task::Task,
        to: *const task::Task,
        scheduler: *mut c_void,
    ) -> *mut c_void;
}

struct Scheduler {
    task_list: TaskList,
}

static SCHEDULER: WithSpinLock<Scheduler> = WithSpinLock::new(Scheduler {
    task_list: TaskList::new(),
});

pub struct Handle<'a> {
    scheduler: WithSpinLockGuard<'a, Scheduler>,
}

impl<'a> Handle<'a> {
    pub(crate) fn new() -> Self {
        let scheduler = unsafe { SCHEDULER.lock() };
        Self { scheduler }
    }
    pub(crate) fn switch(mut self) {
        // Attempt to get the next task before we put the currently running one back in the queue.
        // If we have exhausted runnable tasks, pick the kernel idle task (task 0).
        let switch_to = self.scheduler.task_list.next().unwrap_or(
            self.scheduler
                .task_list
                .get(0)
                .expect("Kernel task 0 must exist."),
        );
        let mut switch_from = self.scheduler.task_list.current_task().unwrap();
        // Hardcode use of HPET for now.
        // TODO: abstract clocksources and get time from the trait object.
        let now = hpet::get_time();
        self.scheduler.task_list.update_runtime(switch_from, now);
        self.scheduler
            .task_list
            .set_current_task(usize::from(switch_to), now);
        let switch_from = self.scheduler.task_list.get_ptr(switch_from);
        let switch_to = self.scheduler.task_list.get_ptr(switch_to);

        let mut scheduler = ManuallyDrop::new(self.scheduler);
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
        self.scheduler.task_list.new_task()
    }

    pub(crate) fn current_task(&self) -> task::TaskHandle {
        let t = self
            .scheduler
            .task_list
            .current_task()
            .expect("No tasks found. Maybe this is called before boot task initialization?");
        t
    }

    pub(crate) fn sleep(mut self, ms: u64) {
        if let Some(clock) = unsafe { clock::CLOCK.get().as_mut() } {
            let (start, until) = {
                let start = clock.get_tick();
                let until = start + ms * 1_000_000;
                (start, until)
            };
            let current_task = {
                let task = self
                    .scheduler
                    .task_list
                    .current_task()
                    .expect("Current task must exist");
                self.scheduler.task_list.set_runnable(task, false);
                task
            };
            clock.callback_at(
                until,
                Box::new(move |x| {
                    // TODO: This callback is deadlocking when trying to obtain the lock on SCHEDULER.
                    SCHEDULER.lock().task_list.set_runnable(current_task, true);
                }),
            );

            self.switch()
        }
    }
}

pub(crate) fn init() {
    let mut scheduler = unsafe { SCHEDULER.lock() };
    scheduler.task_list.init_idle_task()
}
