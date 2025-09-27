use crate::arch::x86_64::hpet;
use crate::kernel::sched::{Scheduler, SCHED_LATENCY};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use crate::some_task;

use alloc::alloc::alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BinaryHeap};
use alloc::format;
use alloc::sync::Arc;
use core::alloc::Layout;
use core::arch::asm;
use core::cell::Cell;
use core::cmp::Ordering;
use core::ffi::c_void;
use core::fmt::{Formatter, Write};
use core::mem::{size_of, ManuallyDrop};
use core::ops::DerefMut;
use core::sync::atomic::Ordering::SeqCst;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};
use core::{fmt, mem, ptr};

pub const KERNEL_STACK_SIZE: usize = 0x2000;
const TASK_STRUCT_MASK: usize = (KERNEL_STACK_SIZE - 1) ^ 0xffff_ffff_ffff_ffff;

extern "C" {
    #[link_name = "boot_stack_top"]
    static mut boot_stack: *const c_void;

    fn _task_entry();
}

pub(crate) struct TaskList {
    next_task_id: usize,
    tasks: BTreeMap<usize, Arc<Task>>,

    // TODO: ensure at type level that this only contains runnable tasks.
    schedulable: BinaryHeap<Arc<Task>>,
}

impl TaskList {
    pub const fn new() -> Self {
        Self {
            next_task_id: 0,
            tasks: BTreeMap::new(),
            schedulable: BinaryHeap::new(),
        }
    }

    // Remove when the scheduler itself is able to wait for runnable Tasks.
    #[deprecated]
    pub fn get(&self, id: usize) -> Option<Arc<Task>> {
        let task = self.tasks.get(&id)?;
        Some(task.clone())
    }

    // TODO: ensure at the type level that this is only called on a runnable task, and not on a
    //       blocked or running one
    pub fn begin_timeslice(&mut self, task: &Arc<Task>, begin_at: u64) {
        task.info.lock().last_scheduled = begin_at;
        let len = self.tasks.len() as u64;
        task.info.lock().run_until = (begin_at + SCHED_LATENCY / len);
    }

    // TODO: ensure at type level that this is only called on a running task, and not on a runnable
    //       or blocked one.
    pub fn end_timeslice(&mut self, task: &Arc<Task>, timestamp: u64) {
        let task_count = self.tasks.len();
        let delta = timestamp - task.info.lock().last_scheduled;
        let delta = delta * task_count as u64;
        task.info.lock().total_runtime += delta;

        // We do not need to first remove the task from the schedulable queue, because update_runtime
        // is called on a running task, which by definition is not in the schedulable queue.
        if (&task.info.lock().flags).is_runnable() {
            self.schedulable.push(task.clone());
        }
    }

    pub fn set_runnable(&mut self, task: &Arc<Task>, runnable: bool) {
        // O(n) complexity over number of tasks in the queue.
        self.schedulable
            .retain(|x| x.info.lock().task_id != task.info.lock().task_id);

        if runnable {
            task.info.lock().flags.0 = 0x1;
            self.schedulable.push(task.clone())
        } else {
            task.info.lock().flags.0 = 0x0;
        }
    }

    pub fn next(&mut self) -> Option<Arc<Task>> {
        let next = self.schedulable.pop()?;
        Some(next.clone())
    }

    pub fn new_task(&mut self) -> Arc<Task> {
        let current_task = self.current_task();
        let mut kernel_stack = unsafe {
            let layout = Layout::new::<Task>();
            let ptr = alloc(layout) as *mut Task;
            (*ptr).info.lock().registers = Registers {
                stack_top: 0,

                // Support for separate address space to be added later.
                cr3: current_task.info.lock().registers.cr3,
            };
            (*ptr).info.lock().flags = TaskFlags(0x1);
            (*ptr).info.lock().last_scheduled = 0;
            (*ptr).info.lock().total_runtime = 0;
            (*ptr).stack = [0; KERNEL_STACK_SIZE - size_of::<TaskInfo>()];

            (*ptr).stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>() - 8)
                ..(KERNEL_STACK_SIZE - size_of::<TaskInfo>())]
                .copy_from_slice(&(_task_entry as usize).to_le_bytes());
            Box::from_raw(ptr)
        };
        let id = self.insert_task(kernel_stack);
        let kernel_stack = self
            .tasks
            .get_mut(&id)
            .expect("Newly created Task must exist!");
        self.schedulable.push(kernel_stack.clone());
        kernel_stack.clone()
    }

    pub fn init_idle_task(&mut self) {
        // We only read the address
        let mut idle_task_stack = unsafe { &raw mut boot_stack as *mut Task };

        // SAFETY: The idle_task_stack never goes out of scope as long as the kernel is running.
        //         Therefore, it is safe to pretend that the memory for the stack was allocated
        //         by the GlobalAllocator, even if it wasn't.
        let mut idle_task_stack = unsafe { Box::from_raw(idle_task_stack) };
        let mut cr3: usize;
        let mut rsp: usize;
        unsafe {
            asm!(
            "mov r8, cr3",
            "mov r9, rsp",
            out("r8") cr3,
            out("r9") rsp,
            );
        }

        // TODO: abstract clocksources and get time from the trait object.
        let now = hpet::get_time();

        idle_task_stack.info = WithSpinLock::new(TaskInfo {
            task_id: 0,
            registers: Registers {
                stack_top: rsp.into(),
                cr3,
            },
            last_scheduled: now.into(),
            run_until: 0,
            total_runtime: 0,
            flags: TaskFlags(0x1),
        });
        let id = self.insert_task(idle_task_stack);
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    fn insert_task(&mut self, mut kernel_stack: Box<Task>) -> usize {
        let id = self.next_task_id;
        kernel_stack.info.lock().task_id = self.next_task_id;
        unsafe {
            kernel_stack.stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 16
                ..(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8]
                .copy_from_slice(&id.to_le_bytes());
        }
        kernel_stack.info.lock().registers.stack_top = &(kernel_stack.stack
            [(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8 - size_of::<usize>() * 6])
            as *const u8 as usize;
        self.tasks.insert(id, Arc::from(kernel_stack));
        self.next_task_id += 1;
        id
    }

    pub(crate) fn current_task(&self) -> Arc<Task> {
        // SAFETY: When a Task is created, its Task struct is placed at the bottom of the 8192 byte
        //         kernel stack, or the top of the 8192 contiguous bytes of memory allocated.
        //         Once created it is never moved or deallocated until the Task ends. Therefore, it is
        //         safe to mask %rsp to get the top of the 8192 byte region of memory that it points to
        //         and use that as the address of the Task struct.
        let task = unsafe {
            let rsp: usize;
            asm!("mov {}, rsp", out(reg) rsp);
            let task = rsp & TASK_STRUCT_MASK;
            let task = task as *const Task;
            &*task
        };
        let id = task.info.lock().task_id;
        self.tasks
            .get(&id)
            .expect("Current task must exist")
            .clone()
    }
}
#[repr(C)]
pub(crate) struct TaskInfo {
    pub(crate) task_id: usize,
    registers: Registers,
    last_scheduled: u64,
    run_until: u64,
    total_runtime: u64,
    pub(crate) flags: TaskFlags,
}

/// Kernel context registers for saving when context switching.
#[repr(C)]
struct Registers {
    // TODO: move the following to architecture specific type and make Task generic
    /// `stack_top` is the value of the stack pointer register.
    stack_top: usize,

    /// `cr3` is the address of the highest level page table (PML4 in x86-64)
    cr3: usize,
}

// TODO: put this data structure behind a lock
// TODO: make this generic over the Registers type.
#[repr(C, align(0x2000))]
pub(crate) struct Task {
    info: WithSpinLock<TaskInfo>,
    stack: [u8; (KERNEL_STACK_SIZE - size_of::<TaskInfo>())],
}

impl Task {
    pub fn get_run_until(&self) -> u64 {
        self.info.lock().run_until
    }

    pub unsafe fn get_info_ptr(&self) -> *const TaskInfo {
        &*self.info.lock() as *const TaskInfo
    }
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.info
            .lock()
            .total_runtime
            .eq(&other.info.lock().total_runtime)
    }
}
impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.info
            .lock()
            .total_runtime
            .partial_cmp(&other.info.lock().total_runtime)
            .map(|o| o.reverse())
    }
}

impl Eq for Task {}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.info
            .lock()
            .total_runtime
            .cmp(&other.info.lock().total_runtime)
            .reverse()
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.info.lock().task_id)
    }
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct TaskFlags(u32);

impl TaskFlags {
    fn is_runnable(&self) -> bool {
        (self.0 & 1) != 0
    }

    fn set_is_runnable(&mut self, is_runnable: bool) {
        match is_runnable {
            true => self.0 |= 1,
            false => self.0 &= !1,
        };
    }
}

#[no_mangle]
unsafe fn task_entry(task_id: usize, scheduler: *mut ManuallyDrop<WithSpinLockGuard<Scheduler>>) {
    {
        let mut scheduler = unsafe { ptr::read(scheduler) };
        // Drop the scheduler RAII guard to release the lock.
        ManuallyDrop::drop(&mut scheduler);
    }

    // Actual code that the task starts running
    some_task();
}
