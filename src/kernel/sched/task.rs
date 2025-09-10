use crate::another_task;
use crate::drivers::serial;
use crate::kernel::sched::Scheduler;
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use alloc::alloc::alloc;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::arch::asm;
use core::cell::RefCell;
use core::fmt::Write;
use core::iter::Take;
use core::mem::{size_of, ManuallyDrop};
use core::{mem, ptr};
use spin::MutexGuard;

const KERNEL_STACK_SIZE: usize = 0x2000;

extern "C" {
    #[link_name = "boot_stack_top"]
    static mut boot_stack: KernelStack;

    fn _task_entry();
}

pub(crate) struct TaskList {
    next_task_id: usize,
    current: Option<usize>,
    tasks: Vec<Task>,
}

impl TaskList {
    pub const fn new() -> Self {
        Self {
            next_task_id: 0,
            current: None,
            tasks: Vec::new(),
        }
    }

    pub fn current_task(&self) -> Option<TaskHandle> {
        let id = self.current?;
        match self.current {
            Some(i) => Some(TaskHandle {
                task_id: id,
                kernel_stack: self.tasks[i].kernel_stack.as_ref(),
            }),
            None => None,
        }
    }

    pub fn set_current_task(&mut self, id: usize) {
        self.current = Some(id);
    }

    pub fn get(&self, id: usize) -> Option<TaskHandle> {
        let task = self.tasks.get(id)?;
        Some(TaskHandle {
            task_id: id,
            kernel_stack: task.kernel_stack.as_ref(),
        })
    }

    pub fn new_task(&mut self) -> usize {
        let current_task = self
            .tasks
            .get(self.current.unwrap())
            .expect("New task creation attempted before boot task initialization.");
        let mut kernel_stack = unsafe {
            let layout = Layout::new::<KernelStack>();
            let ptr = alloc(layout) as *mut KernelStack;
            (*ptr).regs = Registers {
                stack_top: 0,

                // Support for separate address space to be added later.
                cr3: current_task.kernel_stack.regs.cr3,
            };
            (*ptr).stack = [0; KERNEL_STACK_SIZE - size_of::<Registers>()];

            (*ptr).stack[KERNEL_STACK_SIZE - size_of::<Registers>() - 8
                ..KERNEL_STACK_SIZE - size_of::<Registers>()]
                .copy_from_slice(&(_task_entry as usize).to_le_bytes());
            Box::from_raw(ptr)
        };
        let id = self.create_task(kernel_stack);
        let mut task = &mut self.tasks[id];
        unsafe {
            task.kernel_stack.stack[KERNEL_STACK_SIZE - size_of::<Registers>() - 16
                ..KERNEL_STACK_SIZE - size_of::<Registers>() - 8]
                .copy_from_slice(&id.to_le_bytes());
        }
        task.kernel_stack.regs.stack_top = &(task.kernel_stack.stack
            [KERNEL_STACK_SIZE - size_of::<Registers>() - 8 - size_of::<usize>() * 6])
            as *const u8 as usize;
        id
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
        idle_task_stack.regs.cr3 = cr3;
        idle_task_stack.regs.stack_top = rsp;
        let id = self.create_task(idle_task_stack);

        // Hereafter, we are running as the idle task.
        self.current = Some(id);
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    fn create_task(&mut self, kernel_stack: Box<KernelStack>) -> usize {
        let mut task = Task {
            task_id: 0,
            kernel_stack,
        };
        self.tasks.push(task);
        let id = self.tasks.len() - 1;
        self.tasks[id].task_id = id;
        id
    }
}
#[repr(C)]
pub(crate) struct Task {
    pub(crate) task_id: usize,
    kernel_stack: Box<KernelStack>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct TaskHandle {
    pub task_id: usize,
    pub kernel_stack: *const KernelStack,
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
pub(crate) struct KernelStack {
    regs: Registers,
    stack: [u8; (KERNEL_STACK_SIZE - size_of::<Registers>())],
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
    another_task();
}
