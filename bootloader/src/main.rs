#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(alloc)]
extern crate alloc;
extern crate uefi;
extern crate uefi_services;
extern crate rlibc;

use crate::framebuffer::Framebuffer;
use alloc::vec::*;
use core::fmt::Write;

use log::info;
use uefi::prelude::*;
use uefi::table::boot::{EventType, SearchType, TimerTrigger, Tpl};

pub mod framebuffer;

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);

    // Initialize framebuffer
    {
        let fb = &mut Framebuffer::new(&system_table);
        fb.init().expect("failed to initialize framebuffer");
        system_table.boot_services().stall(1000000);
        fb.write_str("Hello, World!\nHello, World again!");
    }

    // Proceed to bootstrapping the kernel.
    loop {}
}
