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
pub mod loader;
use crate::loader::load_file;
use crate::loader::elf::load_elf;

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);

    // Initialize framebuffer
    let fb = &mut Framebuffer::new(&system_table);
    fb.init().expect("failed to initialize framebuffer");
    system_table.boot_services().stall(1000000);
    writeln!(fb, "Hello, World!\nHello, World again!");

    // Proceed to bootstrapping the kernel.
    match load_file(&system_table) {
        Ok(file) => {
            writeln!(fb, "kernel loaded: {}", file.len());
            load_elf(&file, fb);
        }
        Err(e) => {
            writeln!(fb, "kernel load failed: {:?}", e);
        }
    };
    loop {}
}
