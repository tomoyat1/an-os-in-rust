#![no_std]
#![no_main]
#![feature(abi_efiapi)]
extern crate alloc;
extern crate bootlib;
extern crate rlibc;
extern crate uefi;
extern crate uefi_services;

use crate::framebuffer::Framebuffer;
use alloc::vec::*;
use core::arch::asm;
use core::ffi::c_void;
use core::fmt::Write;
use core::ptr;

use log::info;
use uefi::prelude::*;
use uefi::proto::loaded_image::LoadedImage;
use uefi::table::boot;
use uefi::table::boot::{EventType, MemoryDescriptor, SearchType, TimerTrigger, Tpl};
use uefi::table::Runtime;

pub mod framebuffer;
pub mod loader;
use crate::loader::elf::load_elf;
use crate::loader::load_file;
use bootlib::types::BootData;
use core::mem;
use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use uefi::table::cfg::ACPI2_GUID;

static mut SYSTEM_TABLE: *const () = 0x0 as *const ();

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);
    let addr = (&system_table as *const SystemTable<Boot>) as *const ();
    unsafe {
        SYSTEM_TABLE = addr;
    }

    // Initialize framebuffer
    let fb = &mut Framebuffer::new(&system_table);
    fb.init().expect("failed to initialize framebuffer");
    system_table.boot_services().stall(1000000);
    let loaded_image = system_table
        .boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .expect("error when loading loaded image protocol");
    let loaded_image = unsafe { &*loaded_image.get() };
    let (base, size) = loaded_image.info();
    writeln!(fb, "Bootloader was loaded at {:x}", base);
    writeln!(fb, "Loading kernel...");

    // ACPI RSDP
    let mut acpi_rsdp: *const c_void = 0x0 as *const c_void;

    for t in system_table.config_table() {
        if t.guid == ACPI2_GUID {
            acpi_rsdp = t.address;
        }
    }

    // Proceed to bootstrapping the kernel.
    let file = match load_file(&system_table) {
        Ok(file) => {
            writeln!(fb, "kernel loaded: {}", file.len());
            file
        }
        Err(e) => {
            writeln!(fb, "kernel load failed: {:?}", e);
            return uefi::Status::ABORTED;
        }
    };

    writeln!(fb, "Booting kernel...");
    let raw_fb = fb.raw_framebuffer();

    let len = system_table.boot_services().memory_map_size().map_size;
    let len = len * 2;
    let mut mmap = Vec::<u8>::with_capacity(len);
    unsafe { mmap.set_len(len) }

    // Allocate new memory map before exit_boot_services(), since memory allocation will not be
    // available after this point.
    let mut virt_mmap =
        Vec::<MemoryDescriptor>::with_capacity(len / mem::size_of::<MemoryDescriptor>() + 1);
    let (system_table, mmap_iter) = system_table
        .exit_boot_services(handle, mmap.as_mut_slice())
        .expect("failed to exit boot services");

    // Pass virtual memory mappings to UEFI for relocation of runtime services.
    let mut head: u64 = 0xffffffff80000000;
    for entry in mmap_iter {
        let mut ve = MemoryDescriptor::default();
        ve.ty = entry.ty;
        ve.phys_start = entry.phys_start;
        ve.page_count = entry.page_count;
        ve.att = entry.att;
        head -= (ve.page_count * 0x1000);
        ve.virt_start = head;
        virt_mmap.push(ve);
    }

    unsafe {
        system_table
            .runtime_services()
            .set_virtual_address_map(&mut virt_mmap)
            .expect("error when setting virtual memory map");
    }

    let system_table = &system_table as *const SystemTable<Runtime>;
    let entry_point = match load_elf(&file) {
        Ok(ep) => ep,
        Err(e) => {
            return uefi::Status::ABORTED;
        }
    };

    let boot_data = 0x400000 as *mut bootlib::types::BootData;

    let boot_data = unsafe {
        ptr::write(
            boot_data,
            bootlib::types::BootData {
                memory_map_buf: virt_mmap.as_mut_ptr(),
                memory_map_len: virt_mmap.len(),
                framebuffer: raw_fb,
                system_table,
                acpi_rsdp,
            },
        );
        &mut *boot_data
    };
    unsafe { entry_point(boot_data) };
    loop {}
}
