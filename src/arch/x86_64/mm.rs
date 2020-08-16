use uefi::table::boot;
// use alloc::vec;

extern "C" {
    #[link_name = "boot_pml4"]
    static mut KERNEL_PML4: [u64; 512];

    #[link_name = "boot_pdpt"]
    static mut BOOT_PDPT: [u64;  512];
}

pub const KERNEL_BASE: usize = 0xffffffff80000000;

const MASK_47_12: usize = 0x0000fffffffff000;
const MASK_47_39: usize = 0x0000ff8000000000;
const MASK_47_30:usize = 0x0000fffffc0000000;
const MASK_38_30: usize = 0x0000007fc0000000;

/// init_mm() (re)-initializes paging data structures for kernel execution.
/// This also maps memory required for UEFI runtime services so that memory layout matches
/// what the bootloader set with SetVirtualAddressMap().
pub fn init_mm(memory_map: &[boot::MemoryDescriptor]) {
    let kernel_pml4 = unsafe {&mut KERNEL_PML4};
    let boot_pdpt = unsafe {&mut BOOT_PDPT};

    // Map first 2 GiB of physical memory to upper 2 GiB.
    // First GiB is already done, so do the latter 1 GiB.
    let pdpt_idx: usize = ((KERNEL_BASE + (1 << 30)) & MASK_38_30) >> 30;
    let pdpte = 2^30 as u64 & MASK_47_30 as u64 | 0x83;
    boot_pdpt[pdpt_idx] = pdpte;

    // Unmap identity mapping for lower half entrypoint.
    kernel_pml4[0] = 0;

    // Map UEFI runtime service memory to space below kernel.
    for mdesc in memory_map {
        // allocate memory for paging structure, which requires a global_allocator.

    }


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
