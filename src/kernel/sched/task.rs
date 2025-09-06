use crate::another_task;
use crate::drivers::serial;
use crate::locking::spinlock::WithSpinLock;
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
use core::mem::size_of;
use core::{mem, ptr};

const KERNEL_STACK_SIZE: usize = 0x2000;

extern "C" {
    #[link_name = "boot_stack_top"]
    static mut boot_stack: KernelStack;

    fn _do_switch(from: *const KernelStack, to: *const KernelStack);

    fn _task_entry();
}

pub(crate) static mut TASK_LIST: WithSpinLock<TaskList> = WithSpinLock::new(TaskList::new());

pub(crate) struct TaskList {
    next_task_id: usize,
    current: Option<usize>,
    tasks: Vec<Task>,
}

impl TaskList {
    const fn new() -> Self {
        Self {
            next_task_id: 0,
            current: None,
            tasks: Vec::new(),
        }
    }

    fn current_task(&self) -> Option<&Task> {
        match self.current {
            Some(i) => Some(&self.tasks[i]),
            None => None,
        }
    }

    fn new_task(&mut self, kernel_stack: Box<KernelStack>) -> usize {
        let mut task = Task {
            task_id: 0,
            kernel_stack,
        };
        self.tasks.push(task);
        let task_id = self.tasks.len() - 1;
        self.tasks[task_id].task_id = task_id;
        task_id
    }
}
#[repr(C)]
pub(crate) struct Task {
    pub(crate) task_id: usize,
    kernel_stack: Box<KernelStack>,
}

impl Task {
    fn switch_to(&self, to: &Task) {
        // SAFETY: ???
        unsafe {
            asm!("cli");
            _do_switch(
                self.kernel_stack.as_ref() as *const KernelStack,
                to.kernel_stack.as_ref() as *const KernelStack,
            );
            asm!("sti");
        }

        unsafe { TASK_LIST.lock().current = Some(to.task_id) };
        // TODO: Switch paging table.
    }
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
pub(crate) struct KernelStack {
    regs: Registers,
    stack: [u8; (KERNEL_STACK_SIZE - size_of::<Registers>())],
}

pub(crate) fn init_idle_task() {
    // We only read the address
    let mut idle_task_stack = unsafe { &mut boot_stack as *mut KernelStack };

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

    unsafe {
        let mut task_list = TASK_LIST.lock();
        let task_id = task_list.new_task(idle_task_stack);
        task_list.current = Some(task_id);
    }
}

pub(crate) fn new_task() -> usize {
    let mut task_list = unsafe { TASK_LIST.lock() };
    let current_task = task_list
        .current_task()
        .expect("New task creation attempted before boot task initialization.");
    let mut stack = unsafe {
        let layout = Layout::new::<KernelStack>();
        let ptr = alloc(layout) as *mut KernelStack;
        (*ptr).regs = Registers {
            stack_top: 0,

            // Support for separate address space to be added later.
            cr3: current_task.kernel_stack.regs.cr3,
        };
        (*ptr).stack = [0; KERNEL_STACK_SIZE - size_of::<Registers>()];

        // I hope there's a cleaner way to do this...
        let entry_addr = mem::transmute::<&u8, *mut usize>(
            &(*ptr).stack[KERNEL_STACK_SIZE - size_of::<Registers>() - 8],
        );
        ptr::write(entry_addr, _task_entry as usize);
        Box::from_raw(ptr)
    };
    let id = task_list.new_task(stack);
    let mut task = &mut task_list.tasks[id];
    unsafe {
        let task_id = mem::transmute(
            &task.kernel_stack.stack[KERNEL_STACK_SIZE - size_of::<Registers>() - 16],
        );
        ptr::write(task_id, id)
    }
    task.kernel_stack.regs.stack_top = &(task.kernel_stack.stack
        [KERNEL_STACK_SIZE - size_of::<Registers>() - 8 - size_of::<usize>() * 6])
        as *const u8 as usize;
    id
}

#[no_mangle]
#[linkage = "external"]
unsafe fn task_entry(task_id: usize) {
    // Iff we are running this function, then we are running as a newly initialized task.
    // Set the current task as such.
    unsafe {
        let mut task_list = TASK_LIST.lock();
        task_list.current = Some(task_id);
    }

    // Actual code that the task starts running
    another_task();
}

pub(crate) fn current_task() -> usize {
    let task_list = unsafe { TASK_LIST.lock() };
    let t = task_list
        .current_task()
        .expect("No tasks found. Maybe this is called before boot task initialization?");
    t.task_id
}

pub(crate) fn switch_to(to_id: usize) {
    let (from, to) = {
        let task_list = unsafe { TASK_LIST.lock() };
        let from = task_list.current_task().unwrap() as *const Task;
        let to = &task_list.tasks[to_id] as *const Task;
        (from, to)
    };
    unsafe {
        from.as_ref()
            .expect("Unexpected null pointer to previous task struct")
            .switch_to(
                to.as_ref()
                    .expect("Unexpected null pointer to next task struct."),
            )
    };

    unsafe {
        let mut task_list = TASK_LIST.lock();
        task_list.current = Some(to_id);
    }
}

pub(crate) fn debug_task(i: usize) {
    unsafe {
        let task_list = TASK_LIST.lock();
        let kernel_stack =
            (task_list.tasks[i].kernel_stack).as_ref() as *const KernelStack as usize;
        writeln!(
            serial::Handle,
            "task thread {:} at address: 0x{:x}",
            i,
            kernel_stack,
        );
    }
}
