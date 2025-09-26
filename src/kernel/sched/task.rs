use crate::arch::x86_64::hpet;
use crate::kernel::sched::{Scheduler, SCHED_LATENCY};
use crate::locking::spinlock::WithSpinLockGuard;
use crate::some_task;

use alloc::alloc::alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BinaryHeap};
use alloc::sync::Arc;
use core::alloc::Layout;
use core::arch::asm;
use core::cell::Cell;
use core::cmp::Ordering;
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
    static mut boot_stack: Task;

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

    pub fn set_current_task(&mut self, id: usize, now: u64) {
        let mut task = self
            .tasks
            .get_mut(&id)
            .expect("The task set as current must exist");
        task.info.last_scheduled.store(now, SeqCst);
    }

    pub fn get(&self, id: usize) -> Option<TaskHandle> {
        let _ = self.tasks.get(&id)?;
        Some(TaskHandle(id))
    }

    pub fn get_mut(&mut self, id: TaskHandle) -> &mut Arc<Task> {
        let task = self.tasks.get_mut(&id.0).unwrap();
        task
    }

    pub fn get_ptr(&self, id: TaskHandle) -> *const Task {
        let task = self.tasks.get(&id.0).expect("Task must exist for handle!");
        task.as_ref()
    }

    // TODO: ensure at type level that this is only called on a running task, and not on a runnable
    //       or blocked one.
    pub fn update_runtime(&mut self, id: TaskHandle, timestamp: u64) {
        let task_count = self.tasks.len();
        let task = self
            .tasks
            .get_mut(&id.0)
            .expect("Task must exist for handle!");
        let delta = timestamp - task.info.last_scheduled.load(SeqCst);
        let delta = delta * task_count as u64;
        task.info.total_runtime.fetch_add(delta, SeqCst);

        // We do not need to first remove the task from the schedulable queue, because update_runtime
        // is called on a running task, which by definition is not in the schedulable queue.
        if (&task.info.flags).is_runnable() {
            self.schedulable.push(task.clone());
        }
    }

    pub fn get_run_until(&self, id: TaskHandle) -> u64 {
        let task = self
            .tasks
            .get(&usize::from(id))
            .expect("Task must exist for handle");
        task.info.run_until.load(SeqCst)
    }

    pub fn set_run_until(&mut self, id: TaskHandle, begin_at: u64) {
        let len = self.tasks.len() as u64;
        let task = self
            .tasks
            .get_mut(&usize::from(id))
            .expect("Task must exist for handle");
        task.info
            .run_until
            .store((begin_at + SCHED_LATENCY / len), SeqCst);
    }

    pub fn set_runnable(&mut self, id: TaskHandle, runnable: bool) {
        // O(n) complexity over number of tasks in the queue.
        self.schedulable.retain(|x| x.info.task_id != id.into());

        if runnable {
            let task = self
                .tasks
                .get_mut(&usize::from(id))
                .expect("Task with issued handle must exist");

            task.info.flags.0.store(0x1, SeqCst);
            self.schedulable.push(task.clone())
        } else {
            let mut task = self
                .tasks
                .get_mut(&usize::from(id))
                .expect("Task with issued handle must exist");
            task.info.flags.0.store(0x0, SeqCst);
        }
    }

    pub fn next(&mut self) -> Option<TaskHandle> {
        let next = self.schedulable.pop()?;
        Some(TaskHandle(next.info.task_id))
    }

    pub fn new_task(&mut self) -> TaskHandle {
        let current_task = self
            .tasks
            .get(&current_task().0)
            .expect("New task creation attempted before boot task initialization.");
        let mut kernel_stack = unsafe {
            let layout = Layout::new::<Task>();
            let ptr = alloc(layout) as *mut Task;
            (*ptr).info.registers = Registers {
                stack_top: 0.into(),

                // Support for separate address space to be added later.
                cr3: current_task.info.registers.cr3,
            };
            (*ptr).info.flags = TaskFlags(0x1.into());
            (*ptr).info.last_scheduled = 0.into();
            (*ptr).info.total_runtime = 0.into();
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
        self.schedulable.push(kernel_stack.clone());
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

        // TODO: abstract clocksources and get time from the trait object.
        let now = hpet::get_time();

        idle_task_stack.info = TaskInfo {
            task_id: 0,
            registers: Registers {
                stack_top: rsp.into(),
                cr3,
            },
            last_scheduled: now.into(),
            run_until: 0.into(),
            total_runtime: 0.into(),
            flags: TaskFlags(0x1.into()),
        };
        let id = self.create_task(idle_task_stack);
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    fn create_task(&mut self, mut kernel_stack: Box<Task>) -> usize {
        let id = self.next_task_id;
        kernel_stack.info.task_id = self.next_task_id;
        unsafe {
            kernel_stack.stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 16
                ..(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8]
                .copy_from_slice(&id.to_le_bytes());
        }
        kernel_stack.info.registers.stack_top.store(
            (&(kernel_stack.stack
                [(KERNEL_STACK_SIZE - size_of::<TaskInfo>()) - 8 - size_of::<usize>() * 6])
                as *const u8 as usize),
            SeqCst,
        );
        self.tasks.insert(id, Arc::from(kernel_stack));
        self.next_task_id += 1;
        id
    }
}
#[repr(C)]
pub(crate) struct TaskInfo {
    pub(crate) task_id: usize,
    registers: Registers,
    last_scheduled: AtomicU64,
    run_until: AtomicU64,
    total_runtime: AtomicU64,
    pub(crate) flags: TaskFlags,
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
struct Registers {
    // TODO: move the following to architecture specific type and make Task generic
    /// `stack_top` is the value of the stack pointer register.
    stack_top: AtomicUsize,

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

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.info
            .total_runtime
            .load(SeqCst)
            .eq(&other.info.total_runtime.load(SeqCst))
    }
}
impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.info
            .total_runtime
            .load(SeqCst)
            .partial_cmp(&other.info.total_runtime.load(SeqCst))
            .map(|o| o.reverse())
    }
}

impl Eq for Task {}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.info
            .total_runtime
            .load(SeqCst)
            .cmp(&other.info.total_runtime.load(SeqCst))
            .reverse()
    }
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct TaskFlags(AtomicU32);

impl TaskFlags {
    fn is_runnable(&self) -> bool {
        (self.0.load(SeqCst) & 1) != 0
    }

    fn set_is_runnable(&mut self, is_runnable: bool) {
        match is_runnable {
            true => self.0.fetch_or(1, SeqCst),
            false => self.0.fetch_and(!1, SeqCst),
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

pub(crate) fn current_task() -> TaskHandle {
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
    TaskHandle(task.info.task_id)
}
