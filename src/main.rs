#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(alloc)]
#![feature(asm)]
extern crate alloc;
extern crate uefi;
extern crate uefi_services;
extern crate rlibc;

use alloc::vec::*;
use core::fmt::Write;

use log::info;
use uefi::prelude::*;
use uefi::table::boot::{EventType, SearchType, TimerTrigger, Tpl};

pub mod framebuffer;
pub use crate::framebuffer::Framebuffer;

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);

    // Initialize framebuffer
    {
        let fb = &mut Framebuffer::new(&system_table);
        fb.init().expect("failed to initialize framebuffer");
        system_table.boot_services().stall(1000000);
        writeln!(fb, "Hello, World!");

        let mut cr0: u32;
        let mut cr3: u64;
        let mut cr4: u64;
        let mut efer: u64;
        unsafe {
            asm!(
                "mov r8, cr0",
                "mov r9, cr3",
                "mov r10, cr4",
                "mov rcx, 0xC0000080",
                "rdmsr",
                out("rcx") _,
                out("r8") cr0,
                out("r9") cr3,
                out("r10") cr4,
                out("rax") efer,
            );
        }
        writeln!(fb, "cr0: {:x}", cr0);
        writeln!(fb, "protected mode: {}", cr0 & 0x00000001);
        writeln!(fb, "paging: {}", (cr0 & (1 << 31)) >> 31 as u32);

        writeln!(fb, "cr3: {:x}", cr3);
        writeln!(fb, "page directory base: {:x}", (cr3 & 0xFFFFFFFFFFFFF000) >> 12);

        writeln!(fb, "PAE enabled: {:x}", (cr4 & (1 << 5)) >> 5);

        writeln!(fb, "efer: {:x}", efer);
        writeln!(fb, "long mode enabled: {:x}", (efer & (1 << 8)) >> 8);
    }

    // Proceed to bootstrapping the kernel.
    loop {}
}
