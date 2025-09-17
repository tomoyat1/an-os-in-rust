use crate::kernel::sched::Scheduler;
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use crate::some_task;

use alloc::alloc::alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BinaryHeap};

use core::alloc::Layout;
use core::arch::asm;
use core::cmp::Ordering;
use core::fmt::{Formatter, Write};
use core::mem::{size_of, ManuallyDrop};
use core::{fmt, mem, ptr};
use core::ops::DerefMut;

const KERNEL_STACK_SIZE: usize = 0x2000;

extern "C" {
    #[link_name = "boot_stack_top"]
    static mut boot_stack: Task;

    fn _task_entry();
}

pub(crate) struct TaskList {
    next_task_id: usize,
    current: Option<usize>,
    tasks: BTreeMap<usize, Box<Task>>,
    schedulable: BinaryHeap<TaskInfo>,
}

impl TaskList {
    pub const fn new() -> Self {
        Self {
            next_task_id: 0,
            current: None,
            tasks: BTreeMap::new(),
            schedulable: BinaryHeap::new(),
        }
    }

    pub fn current_task(&self) -> Option<TaskHandle> {
        let id = self.current?;
        let _ = self.tasks.get(&id)?;
        Some(TaskHandle(id))
    }

    pub fn set_current_task(&mut self, id: usize, now: u64) {
        self.current = Some(id);
        let mut task = self.tasks.get_mut(&id).expect("The task set as current must exist");
        task.info.last_scheduled = now;
    }

    pub fn get(&self, id: usize) -> Option<TaskHandle> {
        let _ = self.tasks.get(&id)?;
        Some(TaskHandle(id))
    }

    pub fn get_ptr(&self, id: TaskHandle) -> *const Task {
        let task = self.tasks.get(&id.0).expect("Task must exist for handle!");
        task.as_ref()
    }

    pub fn update_runtime(&mut self, id: TaskHandle, timestamp: u64) {
        let task = self
            .tasks
            .get_mut(&id.0)
            .expect("Task must exist for handle!");
        let delta = timestamp - task.info.last_scheduled;
        task.info.score += delta;
        if task.info.flags.is_runnable() {
            self.schedulable.push(task.info);
        }
    }

    pub fn next(&mut self) -> Option<TaskHandle> {
        let next = self.schedulable.pop()?;
        Some(TaskHandle(next.task_id))
    }

    pub fn new_task(&mut self) -> TaskHandle {
        let current_task = self
            .tasks
            .get(&self.current.unwrap())
            .expect("New task creation attempted before boot task initialization.");
        let mut kernel_stack = unsafe {
            let layout = Layout::new::<Task>();
            let ptr = alloc(layout) as *mut Task;
            (*ptr).info.registers = Registers {
                stack_top: 0,

                // Support for separate address space to be added later.
                cr3: current_task.info.registers.cr3,
            };
            (*ptr).stack = [0; KERNEL_STACK_SIZE - size_of::<TaskInfo>()];

            (*ptr).stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>() - 8)
                ..(KERNEL_STACK_SIZE - size_of::<TaskInfo>())]
                .copy_from_slice(&(_task_entry as usize).to_le_bytes());
            Box::from_raw(ptr)
        };
        let id = self.create_task(kernel_stack);
        let kernel_stack = self
            .tasks
            .get_mut(&id)
            .expect("Newly created Task must exist!");
        unsafe {
            kernel_stack.stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 16
                ..(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8]
                .copy_from_slice(&id.to_le_bytes());
        }
        kernel_stack.info.registers.stack_top = &(kernel_stack.stack
            [(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8 - size_of::<usize>() * 6])
            as *const u8 as usize;
        self.schedulable.push(kernel_stack.info);
        TaskHandle(id)
    }

    pub fn init_idle_task(&mut self) {
        // We only read the address
        let mut idle_task_stack = unsafe { &raw mut boot_stack };

        // SAFETY: The idle_task_stack never goes out of scope, as long as the kernel is running.
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
        idle_task_stack.info.registers.cr3 = cr3;
        idle_task_stack.info.registers.stack_top = rsp;
        let id = self.create_task(idle_task_stack);

        // Hereafter, we are running as the idle task.
        self.current = Some(id);
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    fn create_task(&mut self, mut kernel_stack: Box<Task>) -> usize {
        let id = self.next_task_id;
        kernel_stack.info.task_id = self.next_task_id;
        self.tasks.insert(id, kernel_stack);
        self.next_task_id += 1;
        id
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct TaskInfo {
    pub(crate) task_id: usize,
    registers: Registers,
    last_scheduled: u64,
    pub(crate) flags: TaskFlags,
    score: u64,
}

impl PartialEq for TaskInfo {
    fn eq(&self, other: &Self) -> bool {
        self.score.eq(&other.score)
    }
}
impl PartialOrd for TaskInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.score.partial_cmp(&other.score).map(|o| o.reverse())
    }
}

impl Eq for TaskInfo {}

impl Ord for TaskInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.cmp(&other.score).reverse()
    }
}

#[derive(Copy, Clone)]
pub struct TaskHandle(usize);

impl fmt::Display for TaskHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TaskHandle> for usize {
    fn from(value: TaskHandle) -> Self {
        value.0
    }
}

/// Kernel context registers for saving when context switching.
#[repr(C)]
#[derive(Copy, Clone)]
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
    info: TaskInfo,
    stack: [u8; (KERNEL_STACK_SIZE - size_of::<TaskInfo>())],
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct TaskFlags(u32);

impl TaskFlags {
    fn is_runnable(&self) -> bool {
        (self.0 | 1) != 0
    }

    fn set_is_runnable(&mut self, is_runnable: bool) {
        match is_runnable {
            true => self.0 |= 1,
            false => self.0 &= !1,
        }
    }
}

#[no_mangle]
#[linkage = "external"]
unsafe fn task_entry(task_id: usize, scheduler: *mut ManuallyDrop<WithSpinLockGuard<Scheduler>>) {
    {
        let mut scheduler = unsafe { ptr::read(scheduler) };
        // Drop the scheduler RAII guard to release the lock.
        ManuallyDrop::drop(&mut scheduler);
    }

    // Actual code that the task starts running
    some_task();
}
