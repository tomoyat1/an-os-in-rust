#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(alloc)]
#![feature(asm)]
extern crate alloc;
extern crate rlibc;
extern crate uefi;
extern crate uefi_services;

use crate::framebuffer::Framebuffer;
use alloc::vec::*;
use core::fmt::Write;
use core::ptr;

use log::info;
use uefi::prelude::*;
use uefi::table::{Runtime};
use uefi::table::boot;
use uefi::table::boot::{EventType, SearchType, TimerTrigger, Tpl, MemoryDescriptor};
use uefi::proto::loaded_image::{LoadedImage};

pub mod framebuffer;
pub mod loader;
pub mod boot_types;
use crate::loader::elf::load_elf;
use crate::loader::load_file;
use core::mem;
use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use crate::boot_types::BootData;

static mut SYSTEM_TABLE: *const () = 0x0 as *const ();

#[repr(C)]
struct Gdtr {
    limit: u16,
    base: usize,
}

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
    let loaded_image = system_table.boot_services().handle_protocol::<LoadedImage>(handle)
        .expect("error when loading loaded image protocol")
        .expect("warnings when loading loaded image protocol");
    let loaded_image = unsafe {&*loaded_image.get()};
    let (base, size) = loaded_image.info();
    writeln!(fb, "Bootloader was loaded at {:x}", base);
    writeln!(fb, "Loading kernel...");

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

    // system_table.boot_services().stall(10_000_000);
    // writeln!(fb, "Booting kernel...");
    let raw_fb = fb.raw_framebuffer();

    let len = system_table.boot_services().memory_map_size();
    let mut mmap = Vec::<u8>::with_capacity(len * 2);
    unsafe {
        mmap.set_len(len * 2)
    }

    // allocate new memory map before exit_boot_services(), since memory allocation will not be
    // available after this point.
    let mut virt_mmap = Vec::<MemoryDescriptor>::with_capacity(len * 2 / mem::size_of::<MemoryDescriptor>() + 1);
    // TODO: exit_boot_services() depends on having 2 implementations of Framebuffer for boot and
    //       runtime, because system_table will be consumed in this call.
    //       We cannot use the old fb beyond this point.
    let (system_table, mmap_iter) = system_table.exit_boot_services(handle, mmap.as_mut_slice())
        .expect("failed to exit boot services")
        .expect("warnings when exiting boot services");

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
        system_table.runtime_services().set_virtual_address_map(&mut virt_mmap)
            .expect("error when setting virtual memory map");
    }

    // And throw the above work into the trash can ;)
    // let mmap_ptr = 0xdeadbeef as *mut MemoryDescriptor;
    // let mmap_ptr = unsafe {&mut *mmap_ptr};
    // let mut mmap = slice_from_raw_parts_mut(mmap_ptr, 2);
    // let mut mmap = unsafe {&mut *mmap};

    let system_table = &system_table as *const SystemTable<Runtime>;
    let entry_point = match load_elf(&file) {
        Ok(ep) => ep,
        Err(e) => {
            return uefi::Status::ABORTED;
        }
    };

    // set_entrypoint_page_executable(entry_point as usize, fb);

    // // probe segment registers
    // let mut new_gdt: Vec<u64> = Vec::with_capacity(32);
    // unsafe { new_gdt.set_len(32) }
    //
    // let mut gdtr = Gdtr { base: (&new_gdt as *const Vec<u64>) as usize, limit: 32 };
    // let mut gdtr2 = Gdtr { base: 0, limit: 0 };
    // let mut cs: usize = 0;
    // new_gdt[1] = 0x00209a00 << 32; // second 4 byte word; first word is 0 in x64
    // new_gdt[2] = 0x00009200 << 32; // second 4 byte word; first word is 0 in x64
    // unsafe {
    //     asm!(
    //         ".code64",
    //         // ".section .text",
    //         // "lgdt [{gdtr_addr}]",
    //         // "jmp fword ptr [cs_contents]",
    //         // "reload_cs:",
    //         // "mov {r},  0x10",
    //         // "mov ds, {r}",
    //         // "mov es, {r}",
    //         // "mov fs, {r}",
    //         "sgdt [{gdtr_addr2}]",
    //         "mov {cs}, cs",
    //         // ".section .data",
    //         // "cs_contents:",
    //         //  ".quad reload_cs",
    //         //  ".word 0x10",
    //         // gdtr_addr = in(reg) &gdtr as *const Gdtr,
    //         gdtr_addr2 = in(reg) &gdtr2 as *const Gdtr,
    //         // r = out(reg) _,
    //         cs = out(reg) cs,
    //     )
    // }
    // writeln!(fb, "GDT base: {:x}, limit: {:x}", gdtr2.base, gdtr2.limit);
    // writeln!(fb, "CS: {:x}", cs);
    // let gdt = slice_from_raw_parts(gdtr2.base as *const u64, (gdtr2.limit / 0x8) as usize);
    // let mut gdt = unsafe { &*gdt };
    // for i in 0..(gdtr2.limit / 8) {
    //     writeln!(fb, "segment desc. {}: {:x}", i, gdt[(i / 8) as usize]);
    // }

    // set up the most basic IDT here
    // let mut idt = Vec::<u128>::with_capacity(16);
    // unsafe {
    //     idt.set_len(16);
    // }
    // let off015 = entry_point as u128 & 0xffff;
    // let off1631 = entry_point as u128 & 0xffff0000 << 32;
    // let off3263 = entry_point as u128 & 0xffffffff00000000 << 64;
    // let segment_sel: u128 = 1 << 16;
    // let flags: u128 = 0x0
    //

    let boot_data = 0x400000 as *mut boot_types::BootData;

    let boot_data = unsafe {
        ptr::write(boot_data, boot_types::BootData{
            memory_map_buf: virt_mmap.as_mut_ptr(),
            memory_map_len: virt_mmap.len(),
            framebuffer: raw_fb,
            system_table,
        });
        &mut *boot_data
    };
    unsafe { entry_point(boot_data) };
    loop{}
}

fn set_entrypoint_page_executable(entrypoint_addr: usize, fb: &mut Framebuffer) {
    // Assume 4-level paging for now
    // TODO: probe CR4.LA57
    let mut cr3: usize = 0x00000000deadbeef;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) cr3,
        );
    }
    let pml4t = (cr3 & 0x000ffffffffff000) as *const u64;
    let pml4t = unsafe { &*slice_from_raw_parts::<u64>(pml4t, 512) };
    let pml4e = {
        let offset = (entrypoint_addr & 0x0000ff8000000000) >> 9; //Bits 11:3 from 47:39
        pml4t[offset]
    };
    writeln!(fb, "pml4e XD: {}", (pml4e & 0x8000000000000000) >> 63);

    let pdpt = (pml4e & 0x000ffffffffff000) as *const u64;
    let pdpt = unsafe { &*slice_from_raw_parts::<u64>(pdpt, 512) };

    let pdpte = {
        let offset = (entrypoint_addr & 0x000003ffc0000000) >> 27; // Bits 11:3 from 38:30
        pdpt[offset]
    };
    // should check contents of pdpte for page size
    writeln!(fb, "pdpte P: {}", pdpte & 0x1);
    writeln!(fb, "pdpte PS: {}", (pdpte & 0x80) >> 7);
    writeln!(fb, "pdpte XD: {}", (pdpte & 0x8000000000000000) >> 63);
    let pdt = (pdpte & 0x000ffffffffff000) as *const u64;
    let pdt = unsafe { &*slice_from_raw_parts::<u64>(pdt, 512) };

    let pde = {
        let offset = (entrypoint_addr & 0x000000003fe00000) >> 18;
        pdt[offset]
    };
    writeln!(fb, "pde P: {}", pde & 0x1);
    writeln!(fb, "pde PS: {}", (pde & 0x80) >> 7);
    writeln!(fb, "pde XD: {}", (pde & 0x8000000000000000) >> 63);
    writeln!(
        fb,
        "pde addr: {:x}",
        (pde & 0x0000ffffffe00000) | (entrypoint_addr as u64 & 0x1fffff)
    );
}
