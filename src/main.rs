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
use uefi::table::boot::{EventType, MemoryType, SearchType, TimerTrigger, Tpl};

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

        let bt = system_table.boot_services();
        let mem_map_size = 2 * bt.memory_map_size();
        let mut buf: Vec<u8> = Vec::with_capacity(mem_map_size);
        unsafe {
            buf.set_len(mem_map_size);
        }
        let (_k, mut desc_iter) = bt
            .memory_map(&mut buf)
            .expect("Failed to retrieve UEFI memory map")
            .expect("warnings in retriving UEFI memory map");
        writeln!(fb, "efi: usable memory ranges ({} total)", desc_iter.len());
        const EFI_PAGE_SIZE: u64 = 0x1000;
        for (j, descriptor) in desc_iter.enumerate() {
            let size = descriptor.page_count * EFI_PAGE_SIZE;
            let end_address = descriptor.phys_start + size;
            writeln!(fb, "{:?}: {:#x} - {:#x} ({} KiB)", descriptor.ty, descriptor.phys_start, end_address, size);
        }

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
        writeln!(fb, "LA57 enabled: {:x}", (cr4 & (1 << 12)) >> 12);

        writeln!(fb, "efer: {:x}", efer);
        writeln!(fb, "long mode enabled: {:x}", (efer & (1 << 8)) >> 8);
    }

    // Proceed to bootstrapping the kernel.
    loop {}
}
