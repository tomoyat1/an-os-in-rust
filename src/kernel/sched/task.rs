use crate::arch::x86_64::hpet;
use crate::arch::x86_64::mm::mapper;
use crate::kernel::sched::{Scheduler, SCHED_LATENCY};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use crate::some_task;

use alloc::alloc::alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BinaryHeap};

use core::alloc::Layout;
use core::arch::asm;
use core::cmp::Ordering;
use core::fmt::Formatter;
use core::mem::{size_of, ManuallyDrop};
use core::{fmt, ptr};
use interface::Environment;
use x86_64::paging::table::PagingStruct;
use x86_64_bare_metal::X86_64BareMetal;

pub const KERNEL_STACK_SIZE: usize = 0x2000;
const TASK_STRUCT_MASK: usize = (KERNEL_STACK_SIZE - 1) ^ 0xffff_ffff_ffff_ffff;

extern "C" {
    #[link_name = "boot_stack_top"]
    static mut boot_stack: Task;

    fn _task_entry();
}

pub(crate) struct TaskList {
    next_task_id: usize,
    current: Option<usize>,
    tasks: BTreeMap<usize, Box<Task>>,

    // TODO: ensure at type level that this only contains runnable tasks.
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

    pub fn set_current_task(&mut self, id: TaskHandle, now: u64) {
        self.current = Some(id.into());
        let mut task = self
            .tasks
            .get_mut(&id.into())
            .expect("The task set as current must exist");
        task.info.last_scheduled = now;
    }

    pub fn get(&self, id: usize) -> Option<TaskHandle> {
        let _ = self.tasks.get(&id)?;
        Some(TaskHandle(id))
    }

    pub fn get_ptr(&self, id: TaskHandle) -> *const Task {
        let task = self
            .tasks
            .get(&id.into())
            .expect("Task must exist for handle!");
        task.as_ref()
    }

    // TODO: ensure at type level that this is only called on a running task, and not on a runnable
    //       or blocked one.
    pub fn update_runtime(&mut self, id: TaskHandle, timestamp: u64) {
        let task_count = self.tasks.len();
        let task = self
            .tasks
            .get_mut(&id.into())
            .expect("Task must exist for handle!");
        let delta = timestamp - task.info.last_scheduled;
        let delta = delta * task_count as u64;
        task.info.total_runtime += delta;

        // We do not need to first remove the task from the schedulable queue, because update_runtime
        // is called on a running task, which by definition is not in the schedulable queue.
        if task.info.flags.is_runnable() {
            self.schedulable.push(task.info);
        }
    }

    pub fn get_run_until(&self, id: TaskHandle) -> u64 {
        let task = self
            .tasks
            .get(&id.into())
            .expect("Task must exist for handle");
        task.info.run_until
    }

    pub fn set_run_until(&mut self, id: TaskHandle, begin_at: u64) {
        let len = self.tasks.len() as u64;
        let task = self
            .tasks
            .get_mut(&id.into())
            .expect("Task must exist for handle");
        task.info.run_until = begin_at + SCHED_LATENCY / len;
    }

    pub fn set_runnable(&mut self, id: TaskHandle, runnable: bool) {
        // O(n) complexity over number of tasks in the queue.
        self.schedulable.retain(|x| x.task_id != id.into());

        if runnable {
            let task = self
                .tasks
                .get_mut(&id.into())
                .expect("Task with issued handle must exist");
            task.info.flags = TaskFlags(0x1);
            self.schedulable.push(task.info)
        } else {
            let mut task = self
                .tasks
                .get_mut(&id.into())
                .expect("Task with issued handle must exist");
            task.info.flags = TaskFlags(0x0)
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

            // TODO: create separate address space by going through the Mapper.
            let cr3 = mapper().as_mut().unwrap().fork(
                (current_task.info.registers.cr3 + X86_64BareMetal::PAGING_STRUCTURE_BASE)
                    as *mut PagingStruct,
            );
            (*ptr).info.registers = Registers {
                stack_top: 0,

                // Support for separate address space to be added later.
                cr3,
            };
            (*ptr).info.flags = TaskFlags(0x1);
            (*ptr).info.last_scheduled = 0;
            (*ptr).info.total_runtime = 0;
            (*ptr).stack = [0; KERNEL_STACK_SIZE - size_of::<TaskInfo>()];

            (&mut (*ptr)).stack[(KERNEL_STACK_SIZE - size_of::<TaskInfo>() - 8)
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

        idle_task_stack.info = TaskInfo {
            task_id: 0,
            registers: Registers {
                stack_top: rsp,
                cr3,
            },
            last_scheduled: now,
            run_until: 0,
            total_runtime: 0,
            flags: TaskFlags(0x1),
        };
        let id = self.create_task(idle_task_stack);

        // Hereafter, we are running as the idle task.
        self.current = Some(id);
    }

    fn create_task(&mut self, mut kernel_stack: Box<Task>) -> usize {
        let id = self.next_task_id;
        kernel_stack.info.task_id = self.next_task_id;
        self.tasks.insert(id, kernel_stack);
        self.next_task_id += 1;
        id
    }
}

// TODO: make this generic over the Registers type.
#[repr(C, align(0x2000))]
pub(crate) struct Task {
    info: TaskInfo,
    stack: [u8; (KERNEL_STACK_SIZE - size_of::<TaskInfo>())],
}

impl Task {
    pub(crate) fn get_handle(&self) -> TaskHandle {
        TaskHandle(self.info.task_id)
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct TaskInfo {
    pub(crate) task_id: usize,
    registers: Registers,
    last_scheduled: u64,
    run_until: u64,
    total_runtime: u64,
    pub(crate) flags: TaskFlags,
}

impl PartialEq for TaskInfo {
    fn eq(&self, other: &Self) -> bool {
        self.total_runtime.eq(&other.total_runtime)
    }
}
impl PartialOrd for TaskInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.total_runtime
            .partial_cmp(&other.total_runtime)
            .map(|o| o.reverse())
    }
}

impl Eq for TaskInfo {}

impl Ord for TaskInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.total_runtime.cmp(&other.total_runtime).reverse()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub(crate) struct TaskFlags(u32);

impl TaskFlags {
    fn is_runnable(&self) -> bool {
        (self.0 & 1) != 0
    }

    fn set_is_runnable(&mut self, is_runnable: bool) {
        match is_runnable {
            true => self.0 |= 1,
            false => self.0 &= !1,
        }
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

#[no_mangle]
unsafe fn task_entry(task_id: usize, scheduler: *mut ManuallyDrop<WithSpinLockGuard<Scheduler>>) {
    {
        let mut scheduler = unsafe { ptr::read(scheduler) };
        // Drop the scheduler RAII guard to release the lock.
        ManuallyDrop::drop(&mut scheduler);
    }

    // Ensure we enable interrupts after dropping the lock on the scheduler.
    // The new task does not have an `iret` in the kernel exit path, so
    // interrupts will remain disabled. The following is a special exception to ensure
    // interrupts are enabled.
    asm!("sti");

    // Actual code that the task starts running
    some_task();
}

pub(crate) fn current_task() -> TaskHandle {
    // SAFETY: When a Task is created, its Task struct is placed at the bottom of the 8192-byte
    //         kernel stack, or the top of the 8192 contiguous bytes of memory allocated.
    //         Once created, it is never moved or deallocated until the Task ends. Therefore, it is
    //         safe to mask %rsp to get the top of the 8192-byte region of memory that it points to
    //         and read the task identity stored there.
    unsafe {
        let rsp: usize;
        asm!("mov {}, rsp", out(reg) rsp);
        let task = (rsp & TASK_STRUCT_MASK) as *const Task;
        (*task).get_handle()
    }
}
