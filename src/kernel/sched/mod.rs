use crate::kernel::sched::task::{Task, TaskList};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use alloc::vec::Vec;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::{mem, ptr};

mod task;

extern "C" {
    fn _do_switch(
        from: *const task::KernelStack,
        to: *const task::KernelStack,
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
        let from = self.scheduler.task_list.current_task().unwrap();
        let to = (from.task_id + 1) % self.scheduler.task_list.len();
        let to = self.scheduler.task_list.get(to).unwrap();
        self.scheduler.task_list.set_current_task(to.task_id);

        let mut scheduler = ManuallyDrop::new(self.scheduler);
        unsafe {
            let mut scheduler = ptr::read(_do_switch(
                from.kernel_stack,
                to.kernel_stack,
                &raw mut scheduler as *mut c_void,
            )
                as *mut ManuallyDrop<WithSpinLockGuard<Scheduler>>);
            ManuallyDrop::drop(&mut scheduler);
        };
    }

    pub(crate) fn new_task(&mut self) -> usize {
        self.scheduler.task_list.new_task()
    }

    pub(crate) fn current_task(&self) -> usize {
        let t = self
            .scheduler
            .task_list
            .current_task()
            .expect("No tasks found. Maybe this is called before boot task initialization?");
        t.task_id
    }
}

pub(crate) fn init() {
    let mut scheduler = unsafe { SCHEDULER.lock() };
    scheduler.task_list.init_idle_task()
}
