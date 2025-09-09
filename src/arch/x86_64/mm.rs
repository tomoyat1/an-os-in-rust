use core::arch::asm;
use core::ptr;
use core::slice::from_raw_parts;

use uefi::table::boot;

use crate::mm::malloc;

extern "C" {
    #[link_name = "boot_pml4"]
    static mut KERNEL_PML4: [u64; 512];

    #[link_name = "boot_pdpt"]
    static mut BOOT_PDPT: [u64; 512];
}

pub const KERNEL_BASE: usize = 0xffffffff80000000;

const MASK_51_12: usize = 0x000ffffffffff000;
const MASK_51_30: usize = 0x000ffffffc0000000;
const MASK_47_12: usize = 0x0000fffffffff000;
const MASK_47_39: usize = 0x0000ff8000000000;
const MASK_47_30: usize = 0x0000fffffc0000000;
const MASK_38_30: usize = 0x0000007fc0000000;
const MASK_29_0: usize = 0x000000003fffffff;

/// init_mm() (re)-initializes paging data structures for kernel execution.
/// This also maps memory required for UEFI runtime services so that memory layout matches
/// what the bootloader set with SetVirtualAddressMap().
pub fn init_mm(memory_map: &[boot::MemoryDescriptor]) {
    let kernel_pml4 = unsafe { &raw const KERNEL_PML4 };

    // Map first 2 GiB of physical memory to upper 2 GiB.
    // First GiB is already done, so do the latter 1 GiB.
    let pdpt_idx: usize = ((KERNEL_BASE + (1 << 30)) & MASK_38_30) >> 30;
    let pdpte = 1 << 30u64 & MASK_47_30 as u64 | 0x83;
    unsafe {
        let boot_pdpt = unsafe { &raw mut BOOT_PDPT[pdpt_idx] };
        ptr::write_volatile(boot_pdpt, pdpte);
    }

    // Map UEFI runtime service memory to space below kernel.
    // memory_map contains ALL mappings, including ones unnecessary after exit_boot_services().
    // It is our responsibility here to filter out unnecessary MemoryTypes.
    for mdesc in memory_map {
        let ty = mdesc.ty;
        let virt_start = mdesc.virt_start;
        let phys_start = mdesc.phys_start;
        let page_count = mdesc.page_count;

        // TODO: implement later when we use UEFI runtime services.
        // match mdesc.ty {
        //     boot::MemoryType::RUNTIME_SERVICES_CODE => {
        //     },
        //     boot::MemoryType::RUNTIME_SERVICES_DATA => {
        //
        //     },
        //     _ => { /* noop */ },
        // }
    }

    // Unmap identity mapping for lower half entrypoint.
    // If we tear this down here, APIC related code which depends on identity mapping does not work.

    // kernel_pdpt[0] = 0;

    flush_tlb();
}

/// flush_tlb() flushes the TLB.
fn flush_tlb() {
    unsafe {
        asm!(
             "mov {tmp}, cr3",
             "mov cr3, {tmp}",
             tmp = out(reg) _,
        )
    }
}

/// phys_addr returns the physical address for `linear_address`.
// TODO: decide u64 or usize or *const u8.
pub fn phys_addr(linear_addr: *const u8) -> *const u8 {
    let pml4e = {
        let idx = (linear_addr as usize & MASK_47_39) >> 39;
        let kernel_pml4 = unsafe { &raw const KERNEL_PML4[idx] };
        unsafe { ptr::read_volatile(kernel_pml4) }
    } as usize;

    let pdpte = unsafe {
        // We know the kernel base, so cheat by adding it.
        // TODO: Design data structure that will tell us the virtual address of paging structures from just the
        //       virtual address of PML4.
        let pdpt = (KERNEL_BASE | pml4e & MASK_51_12) as *const u64;
        let pdpt = from_raw_parts(pdpt, 512);
        let idx = ((linear_addr as usize & MASK_38_30) >> 30) as usize;
        pdpt[idx]
    } as usize;

    // If PS = 1
    if pdpte & 0x80 == 0x80 {
        (pdpte & MASK_51_30 | (linear_addr as usize) & MASK_29_0) as *const u8
    } else {
        0 as *const u8
    }
}
