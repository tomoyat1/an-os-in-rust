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

    sched::init();

    let lapic_id = interrupt::init(&madt);

    serial::init();
    serial::tmp_write_com1(b"[OK]\tSerial console initialized\n");

    let hpet = acpi::parse_hpet(boot_data.acpi_rsdp);
    let clock = match hpet {
        Ok(hpet) => {
            let hpet = hpet::init(hpet);
            hpet::register_tick(clock::tick_fn());
            clock::init(hpet);
            hpet
        }
        Err(_) => {
            // pit::start();
            // pit::register_tick(clock::tick_fn());
            panic!("No supported clocksource found!")
        }
    };

    // Initialize PCI devices
    pci::init(lapic_id);
    let nics = rtl8139::init(&madt.interrupt_mappings);
    if nics == 1 {
        serial::tmp_write_com1(b"[OK]\tRTL8139 NIC initialized\n");
    } else {
        serial::tmp_write_com1(b"[OK]\tNo NICs found\n")
    }

    // Create several tasks to demonstrate switching.
    {
        let mut scheduler = sched::lock();
        scheduler.new_task();
    }
    {
        let mut scheduler = sched::lock();
        scheduler.new_task();
    }
    {
        let mut scheduler = sched::lock();
        scheduler.new_task();
    }

    // Start kernel main loop, where we handle queued data from interrupts.
    loop {
        sleep(1000);
        let current = sched::current_task();
        writeln!(serial::Handle::new(), "Yo! from kernel main loop",);

        // Were done handling all unprocessed inputs/outputs. Switch to another task.
        let scheduler = sched::lock();
        scheduler.switch()
    }
}

#[no_mangle]
#[linkage = "external"]
/// This is a placeholder for actual code that a newly created task would run.
pub unsafe extern "C" fn some_task() {
    loop {
        let current = sched::current_task();
        writeln!(serial::Handle::new(), "Yo! from some task: {:}", current);
        let mut scheduler = sched::lock();
        scheduler.sleep(1000)
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
