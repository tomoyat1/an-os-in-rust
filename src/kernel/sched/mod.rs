use alloc::vec::Vec;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::{mem, ptr};

use crate::arch::x86_64::hpet;
use crate::kernel::sched::task::{TaskInfo, TaskList};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};

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
        // For now, assume that the list of schedulable tasks is not empty.
        let switch_to = self
            .scheduler
            .task_list
            .next()
            .expect("Schedulable tasks exhausted.");
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
}

pub(crate) fn init() {
    let mut scheduler = unsafe { SCHEDULER.lock() };
    scheduler.task_list.init_idle_task()
}
