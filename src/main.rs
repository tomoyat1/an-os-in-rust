#![no_std]
#![no_main]
#![feature(linkage)]
#![feature(asm)]

extern crate rlibc;

use core::panic::PanicInfo;

mod arch;

use arch::x86_64::mm::init_mm;

#[no_mangle]
#[linkage = "external"]
/// start() is the entry point for kernel code.
/// # Arguments
/// * `gop_buf` - The address of the framebuffer passed into the kernel by the bootloader.
///               The framebuffer is obtained through using the UEFI GOP protocol.
pub unsafe extern "C" fn start(gop_buf: *const [u8]) {
    init_mm();

    let stack_top: *mut u8 = 0xFFFFFFFFCFFFFFFF as *mut u8;
    unsafe {
        let stack_top = &mut *stack_top;
        *stack_top = 0xde;
    }

    // Start scheduler

    // Scheduler should not return;
    // panic!("Scheduler has returned when it shouldn't have");
    loop{}
}

#[panic_handler]
/// panic() handles panics!()'s in the kernel. These are called "kernel panic"s.
fn panic(_info: &PanicInfo) -> ! {
    // Do nothing and loop for now.
    // TODO: Paint screen red.
    loop{}
}
