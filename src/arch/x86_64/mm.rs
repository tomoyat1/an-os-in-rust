extern "C" {
    #[link_name = "boot_pml4"]
    static mut KERNEL_PML4: [u64; 512];

    #[link_name = "boot_pdpt"]
    static mut BOOT_PDPT: [u64;  512];
}

/// init_mm() (re)-initializes paging data structures for kernel execution.
/// This also maps memory required for UEFI runtime services so that memory layout matches
/// what the bootloader set with SetVirtualAddressMap().
pub fn init_mm() {
    let mut kernel_pml4 = unsafe {&KERNEL_PML4};
    let mut boot_pdpt = unsafe {&BOOT_PDPT};

    // Map first 2 GiB of physical memory to upper 2 GiB.
    // First GiB is already done, so do the latter 1 GiB.

    // Unmap identity mapping for lower half entrypoint.

    // Map UEFI runtime service memory to space below kernel.
}
