#![no_std]
#![no_main]
#![no_builtins]
#![feature(linkage)]
#![feature(asm)]
#![feature(alloc_error_handler)]

extern crate rlibc;
extern crate alloc;
extern crate bootlib;

use core::panic::PanicInfo;

mod arch;
use arch::x86_64::mm::{init_mm, KERNEL_BASE};
use arch::x86_64::pm::init_pm;

mod boot;

mod mm;

#[no_mangle]
#[linkage = "external"]
/// start() is the entry point for kernel code.
/// # Arguments
/// * `boot_data` - The address of the BootData struct provided from the bootloader.
pub unsafe extern "C" fn start(boot_data: *mut bootlib::types::BootData) {
    let boot_data = boot::BootData::relocate(boot_data, KERNEL_BASE);
    init_mm(boot_data.memory_map); // TODO: error handling
    let gdt = init_pm();

    // let stack_top: *mut u8 = 0xffffffffcfffffff as *mut u8;
    // let stack_top = &mut *stack_top;
    // *stack_top = 0xde;

    // Start scheduler

    // Scheduler should not return;
    // panic!("Scheduler has returned when it shouldn't have");
    loop {}
}

#[panic_handler]
/// panic() handles panics!()'s in the kernel. These are called "kernel panic"s.
fn panic(_info: &PanicInfo) -> ! {
    // Do nothing and loop for now.
    // TODO: Paint screen red.
    loop {}
}
