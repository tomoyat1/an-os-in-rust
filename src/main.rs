#![no_std]
#![no_main]
#![no_builtins]
#![feature(linkage)]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![feature(const_fn)]

extern crate alloc;
extern crate bootlib;
extern crate rlibc;

use core::panic::PanicInfo;

mod arch;
use arch::x86_64::interrupt;
use arch::x86_64::mm::{init_mm, KERNEL_BASE};
use arch::x86_64::pit;
use arch::x86_64::pm;

mod boot;
mod drivers;
use drivers::acpi;
use drivers::serial;

mod mm;

mod kernel;
use crate::kernel::clock::Clock;

mod locking;

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
    interrupt::init(madt);
    let mut clock = pit::start();
    serial::init();

    // let stack_top: *mut u8 = 0xffffffffcfffffff as *mut u8;
    // let stack_top = &mut *stack_top;
    // *stack_top = 0xde;

    // Wait for 1000ms
    clock.sleep(10000);

    serial::tmp_write_com1(b"Done");

    // Start scheduler

    // Scheduler should not return;
    // panic!("Scheduler has returned when it shouldn't have");
    loop {
        let mut f1 = 1;
        let mut f2 = 1;
        for _ in 0..5 {
            let t = f1 + f2;
            f1 = f2;
            f2 = t;
        }
    }
}

#[panic_handler]
/// panic() handles panics!()'s in the kernel. These are called "kernel panic"s.
fn panic(_info: &PanicInfo) -> ! {
    // Do nothing and loop for now.
    // TODO: Paint screen red.
    loop {}
}
