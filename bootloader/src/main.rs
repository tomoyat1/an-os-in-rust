#![no_std]
#![no_main]
extern crate alloc;
extern crate bootlib;
extern crate rlibc;
extern crate uefi;

use crate::framebuffer::Framebuffer;
use alloc::vec::*;
use core::ffi::c_void;
use core::fmt::Write;
use core::ptr;

use uefi::prelude::*;
use uefi::proto::loaded_image::LoadedImage;
use uefi::table::boot::{MemoryDescriptor, MemoryType};
use uefi::table::Runtime;

pub mod framebuffer;
pub mod loader;
use crate::loader::elf::load_elf;
use crate::loader::load_file;
use bootlib::types::BootData;
use uefi::table::cfg::ACPI2_GUID;

static mut SYSTEM_TABLE: *const () = 0x0 as *const ();

#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi::helpers::init(&mut system_table).expect("failed to initialize uefi-rs library.");
    let addr = (&system_table as *const SystemTable<Boot>) as *const ();
    unsafe {
        SYSTEM_TABLE = addr;
    }

    // Initialize framebuffer
    let mut fb = Framebuffer::new(&system_table);
    fb.init().expect("failed to initialize framebuffer");
    system_table.boot_services().stall(1000000);
    let handle = system_table.boot_services().image_handle();
    let loaded_image = system_table
        .boot_services()
        .open_protocol_exclusive::<LoadedImage>(handle)
        .expect("error when loading loaded image protocol");
    let (base, _size) = loaded_image.info();
    writeln!(fb, "Bootloader was loaded at {:p}", base);
    writeln!(fb, "Loading kernel...");
    drop(loaded_image);

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
            writeln!(fb, "kernel read failed: {:?}", e);
            return Status::ABORTED;
        }
    };

    writeln!(fb, "Booting kernel...");
    let raw_fb = fb.raw_framebuffer();

    let len = {
        let mmap = system_table
            .boot_services()
            .memory_map(MemoryType::LOADER_DATA)
            .unwrap();
        mmap.entries().len() + 2 // A couple of extra entries.
    };

    // Allocate new memory map before exit_boot_services(), since memory allocation will not be
    // available after this point.
    let mut virt_mmap = Vec::<MemoryDescriptor>::with_capacity(len);
    let (system_table, mut mmap_iter) =
        unsafe { system_table.exit_boot_services(MemoryType::LOADER_DATA) };

    // Pass virtual memory mappings to UEFI for relocation of runtime services.
    let mut head: u64 = 0xffffffff80000000;
    mmap_iter.sort();
    for entry in mmap_iter.entries() {
        let mut ve = MemoryDescriptor::default();
        ve.ty = entry.ty;
        ve.phys_start = entry.phys_start;
        ve.page_count = entry.page_count;
        ve.att = entry.att;
        head -= ve.page_count * 0x1000;
        ve.virt_start = head;
        virt_mmap.push(ve);
    }

    // The kernel expects the system table to be identity-mapped when it starts, so pass
    // virt_mmap.as_prt() as u64 as the `new_system_table_virtual_addr`.
    let system_table_virt_addr = virt_mmap.as_ptr() as u64;
    let system_table = unsafe {
        system_table
            .set_virtual_address_map(&mut virt_mmap, system_table_virt_addr)
            .expect("error when setting virtual memory map")
    };

    let system_table = &system_table as *const SystemTable<Runtime>;
    let entry_point = match load_elf(&file) {
        Ok(ep) => ep,
        Err(_e) => {
            return Status::ABORTED;
        }
    };

    let boot_data = 0x400000 as *mut BootData;

    let boot_data = unsafe {
        ptr::write(
            boot_data,
            BootData {
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
