#![no_std]
#![no_main]
#![no_builtins]
#![feature(linkage)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]
#![allow(unused)]
#![allow(unused_unsafe)]
#![feature(drain_filter)]
#![feature(new_uninit)]

extern crate alloc;
extern crate bootlib;
extern crate rlibc;

use core::fmt::Write;
use core::panic::PanicInfo;

mod arch;
use arch::x86_64::interrupt;
use arch::x86_64::mm::{init_mm, KERNEL_BASE};
use arch::x86_64::pit;
use arch::x86_64::pm;

mod boot;
mod drivers;
use drivers::acpi;
use drivers::net::rtl8139;
use drivers::pci;
use drivers::serial;

mod mm;

mod kernel;
use crate::kernel::clock;
use crate::kernel::clock::Clock;
use kernel::sched::task;

mod locking;

mod net;

#[no_mangle]
#[linkage = "external"]
/// start() is the entry point for kernel code.
/// # Arguments
/// * `boot_data` - The address of the BootData struct provided from the bootloader.
pub unsafe extern "C" fn start(boot_data: *mut bootlib::types::BootData) {
    let boot_data = boot::BootData::relocate(boot_data, KERNEL_BASE);
    let madt = acpi::parse_madt(boot_data.acpi_rsdp).expect("failed to parse MADT");
    init_mm(boot_data.memory_map); // TODO: error handling
    let gdt = pm::init();
    let lapic_id = interrupt::init(&madt);
    let mut clock = pit::start();
    serial::init();

    // let stack_top: *mut u8 = 0xffffffffcfffffff as *mut u8;
    // let stack_top = &mut *stack_top;
    // *stack_top = 0xde;

    // Wait for 1000ms
    clock.sleep(1000);

    task::init_idle_task();
    task::debug_task(0);

    serial::tmp_write_com1(b"Done\n");

    // Initialize PCI devices
    pci::init(lapic_id);
    let nics = rtl8139::init(&madt.interrupt_mappings);
    if nics == 1 {
        serial::tmp_write_com1(b"RTL8139 FOUND\n");
    } else {
        serial::tmp_write_com1(b"NO NIC\n")
    }

    // Create another task to demonstrate switching.
    task::new_task();
    task::debug_task(1);

    // Placeholder code for kernel idle task.
    // TODO: this function starts the scheduler?
    loop {
        // clock.sleep(1000);
        let current = task::current_task();
        writeln!(serial::Handle, "Yo! from task {:}", current);
        task::switch_to((current + 1) % 2)
    }
}

#[no_mangle]
#[linkage = "external"]
/// another_task() is a placeholder for actual code that a newly created task would run.
pub unsafe extern "C" fn another_task() {
    loop {
        let current = task::current_task();
        writeln!(serial::Handle, "Yo! from task {:}", current);
        task::switch_to((current + 1) % 2)
    }
}

#[panic_handler]
/// panic() handles panics!()'s in the kernel. These are called "kernel panic"s.
fn panic(info: &PanicInfo) -> ! {
    fn do_panic(info: &PanicInfo) -> Option<()> {
        serial::init();
        let args = info.message()?;
        let location = info.location()?;
        writeln!(
            serial::Handle,
            "file: {}, line: {}, col: {}",
            location.file(),
            location.line(),
            location.column(),
        );
        core::fmt::write(&mut serial::Handle, *args);
        Some(())
    };
    let r = do_panic(info);
    if r == None {
        writeln!(serial::Handle, "Failed to get panic message");
    }

    // TODO: Paint screen red.
    loop {}
}
