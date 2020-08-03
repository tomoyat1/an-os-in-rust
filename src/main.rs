#![no_std]
#![no_main]
#![feature(linkage)]

extern crate rlibc;

use core::panic::PanicInfo;

#[no_mangle]
#[linkage = "external"]
pub unsafe extern "C" fn start() {
    let mut sum = foo;
    for i in 0..3 {
        sum += i;
    }
    loop{}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop{}
}

#[link_section = ".bss"]
static foo: u64 = 0;

#[link_section = ".data"]
static bar: u64 = 42;
