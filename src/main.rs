#![no_std]
#![no_main]
#![no_builtins]
#![feature(linkage)]
#![feature(alloc_error_handler)]
#![allow(unused)]
#![allow(unused_unsafe)]
#![feature(extract_if)]
#![feature(sync_unsafe_cell)]
extern crate alloc;
extern crate bootlib;
extern crate rlibc;

use core::fmt::{Debug, Write};
use core::panic::PanicInfo;

mod arch;
use arch::x86_64::interrupt;
use arch::x86_64::mm::{init_mm, KERNEL_BASE};
use arch::x86_64::pm;
use arch::x86_64::{hpet, pit};

mod boot;
mod drivers;
use drivers::acpi;
use drivers::net::rtl8139;
use drivers::pci;
use drivers::serial;

mod mm;

mod kernel;
use crate::kernel::clock;
use crate::kernel::clock::{sleep, Clock};
use crate::kernel::sched;

mod locking;

mod net;

#[no_mangle]
#[linkage = "external"]
/// start() is the entry point for kernel code.
/// # Arguments
/// * `boot_data` - The address of the BootData struct provided from the bootloader.
pub unsafe extern "C" fn start(boot_data: *mut bootlib::types::BootData) {
    let boot_data = boot::BootData::relocate(boot_data, KERNEL_BASE);
    let madt = acpi::parse_madt(boot_data.acpi_rsdp).expect("failed to parse ACPI tables");
    init_mm(boot_data.memory_map); // TODO: error handling
    let gdt = pm::init();
    let lapic_id = interrupt::init(&madt);

    let hpet = acpi::parse_hpet(boot_data.acpi_rsdp);
    match hpet {
        Ok(hpet) => {
            hpet::init(hpet);
            hpet::register_tick(clock::tick_fn());
        }
        Err(_) => {
            pit::start();
            pit::register_tick(clock::tick_fn());
        }
    }

    serial::init();

    // Wait for 1000ms
    sleep(1000);

    sched::init();

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
    {
        let mut scheduler = sched::Handle::new();
        scheduler.new_task();
    }

    // Placeholder code for kernel idle task.
    // TODO: this function starts the scheduler?
    loop {
        sleep(1000);
        let scheduler = sched::Handle::new();
        let current = scheduler.current_task();
        writeln!(serial::Handle::new(), "Yo! from task {:}", current);
        scheduler.switch()
    }
}

#[no_mangle]
#[linkage = "external"]
/// another_task() is a placeholder for actual code that a newly created task would run.
pub unsafe extern "C" fn another_task() {
    loop {
        sleep(1000);
        let scheduler = sched::Handle::new();
        let current = scheduler.current_task();
        writeln!(serial::Handle::new(), "Another yo! from task {:}", current);
        scheduler.switch()
    }
}

#[panic_handler]
/// panic() handles panics!()'s in the kernel. These are called "kernel panic"s.
fn panic(info: &PanicInfo) -> ! {
    fn do_panic(info: &PanicInfo) -> Option<()> {
        serial::init();
        let args = info.message();
        let location = info.location()?;
        writeln!(
            serial::Handle::new(),
            "file: {}, line: {}, col: {}",
            location.file(),
            location.line(),
            location.column(),
        );
        write!(&mut serial::Handle::new(), "{}", info.message());
        Some(())
    };
    let r = do_panic(info);
    if r == None {
        writeln!(serial::Handle::new(), "Failed to get panic message");
    }

    // TODO: Paint screen red.
    loop {}
}
