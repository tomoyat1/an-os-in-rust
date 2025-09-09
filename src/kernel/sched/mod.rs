use crate::kernel::sched::task::{Task, TaskList};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use alloc::vec::Vec;
use core::mem;
use core::ops::Deref;

mod task;

struct Scheduler {}

static mut SCHEDULER: WithSpinLock<Scheduler> = WithSpinLock::new(Scheduler {});

pub struct SchedulerHandle<'a> {
    // scheduler: WithSpinLockGuard<'a, Scheduler>,
    task_list: WithSpinLockGuard<'a, TaskList>,
}

impl<'a> SchedulerHandle<'a> {
    pub(crate) fn switch(self) {
        let from = self.task_list.current_task().unwrap();
        let to = (from.task_id + 1) % self.task_list.len();
        let to = self.task_list.get(to).unwrap();

        from.switch_to(to, self.task_list)
    }

    pub(crate) fn new_task(&mut self) -> usize {
        self.task_list.new_task()
    }

    pub(crate) fn current_task(&self) -> usize {
        let t = self
            .task_list
            .current_task()
            .expect("No tasks found. Maybe this is called before boot task initialization?");
        t.task_id
    }
}

pub(crate) fn handle<'a>() -> SchedulerHandle<'a> {
    // let scheduler = unsafe { SCHEDULER.lock() };
    let task_list = unsafe { task::TASK_LIST.lock() };
    SchedulerHandle {
        // scheduler,
        task_list,
    }
}

pub(crate) fn init() {
    let mut task_list = unsafe { task::TASK_LIST.lock() };
    task_list.init_idle_task()
}
