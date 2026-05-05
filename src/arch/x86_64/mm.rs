use alloc::vec::Vec;
use core::arch::asm;
use core::ops;
use core::ptr;
use core::slice::from_raw_parts;

use uefi::table::boot::{MemoryDescriptor, MemoryType};

use crate::locking::spinlock::WithSpinLock;
use crate::mm::malloc;
use paging::physical;

extern "C" {
    #[link_name = "boot_pml4"]
    static mut KERNEL_PML4: [PageEntry; 512];

    #[link_name = "boot_pdpt"]
    static mut BOOT_PDPT: [PageEntry; 512];

    #[link_name = "boot_pdt"]
    static mut BOOT_PDT: [PageEntry; 512];
}

static PHYSICAL_PAGE_ALLOCATOR: WithSpinLock<physical::PageAllocator> =
    WithSpinLock::new(physical::PageAllocator::new());

pub const KERNEL_BASE: usize = 0xffff800000000000;

const MASK_51_12: usize = 0x000ffffffffff000;
const MASK_51_30: usize = 0x000ffffffc0000000;
const MASK_51_21: usize = 0x0000fffffffe00000;
const MASK_47_12: usize = 0x0000fffffffff000;
const MASK_47_39: usize = 0x0000ff8000000000;
const MASK_47_30: usize = 0x0000fffffc0000000;
const MASK_38_30: usize = 0x0000007fc0000000;
const MASK_29_21: usize = 0x000000003fe00000;
const MASK_29_0: usize = 0x000000003fffffff;
const MASK_20_0: usize = 0x00000000001fffff;

/// init_mm() (re)-initializes paging data structures for kernel execution.
/// It also sets up paging for the kernel heap located at KERNEL_BASE + 512MiB
pub fn init_mm(memory_map: &[MemoryDescriptor]) {
    let kernel_pml4 = unsafe { &raw const KERNEL_PML4 };

    /*
    TODO: remove this code; we do not map the next 2 GiB anymore.
    // Map the first 2 GiB of physical memory to the upper 2 GiB.
    // First GiB is already done, so do the latter 1 GiB.
    let pdpt_idx: usize = ((KERNEL_BASE + (1 << 30)) & MASK_38_30) >> 30;
    let pdpte = 1 << 30u64 & MASK_47_30 as u64 | 0x83;
    unsafe {
        let boot_pdpt = unsafe { &raw mut BOOT_PDPT[pdpt_idx] };
        ptr::write_volatile(boot_pdpt, pdpte);
    }
    */

    let mut free_blocks = Vec::<(usize, usize)>::new();

    for mdesc in memory_map {
        match mdesc.ty {
            MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::CONVENTIONAL => {
                // TODO: exclude range of boot page tables
                if let Some(block) = exclude_ranges(mdesc, &[0..0x800000]) {
                    free_blocks.push(block)
                }
            }
            _ => { /* noop */ }
        }
    }

    {
        PHYSICAL_PAGE_ALLOCATOR.lock().init(&free_blocks);
    }

    // TODO: map boot page tables to offset 0xffff_ff800_0000_0000

    for mdesc in memory_map {
        // TODO: Map with KERNEL_BASE offset the following
        //         - RUNTIME_SERVICES_CODE
        //         - RUNTIME_SERVICES_DATA
        //         - ACPI_RECLAIM
        //         - ACPI_NON_VOLATILE
        //         - MMIO
        //         - MMIO_PORT_SPACE
        //       Requires:
        //         - Physical page allocator
        //         - Virtual page allocator
        match mdesc.ty {
            MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA
            | MemoryType::ACPI_RECLAIM
            | MemoryType::ACPI_NON_VOLATILE
            | MemoryType::MMIO
            | MemoryType::MMIO_PORT_SPACE => {
                // TODO: Map [phys_start, phys_start + page_count * 4096) to
                //       [KERNEL_BASE + phys_start, ...) so the layout matches what was set
                //       via set_virtual_address_map(). Note that this may require obtaining free pages
                //       from the physical page allocator, initialized in the for loop above.
            }
            _ => { /* noop */ }
        }
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
pub fn phys_addr(linear_addr: *const u8) -> *const u8 {
    let pml4e = {
        let idx = (linear_addr as usize & MASK_47_39) >> 39;
        let pml4e = unsafe { &raw const KERNEL_PML4[idx] };
        unsafe { ptr::read(pml4e) }
    };

    let pdpte = unsafe {
        // We know the kernel base, so get the virtual address of the pdpte just adding it.
        // TODO: change this to paging structures base 0xffffff80000000000.
        let pdpt = (KERNEL_BASE | pml4e.bytes & MASK_51_12) as *const PageEntry;
        let pdpt = from_raw_parts(pdpt, 512);
        let idx = (linear_addr as usize & MASK_38_30) >> 30;
        &pdpt[idx]
    };

    // If PS = 1
    if pdpte.bytes & 0x80 == 0x80 {
        (pdpte.bytes & MASK_51_30 | (linear_addr as usize) & MASK_29_0) as *const u8
    } else {
        let pdte = unsafe {
            let pdt = (KERNEL_BASE | pdpte.bytes & MASK_51_12) as *const PageEntry;
            let pdt = from_raw_parts(pdt, 512);
            let idx = (linear_addr as usize & MASK_29_21) >> 21;
            &pdt[idx]
        };
        if pdte.get_flags(0x80) == 0x80 {
            (pdte.bytes & MASK_51_21 | (linear_addr as usize) & MASK_20_0) as *const u8
        } else {
            0 as *const u8
        }
    }
}

fn exclude_range(
    (start, length): (usize, usize),
    range: ops::Range<usize>,
) -> Option<(usize, usize)> {
    let result = match (range.contains(&start), range.contains(&(start + length))) {
        (true, true) => None,
        (true, false) => {
            let shift = range.end - start;
            let start = range.end;
            let length = length - shift;
            if length == 0 {
                None
            } else {
                Some((start, length))
            }
        }
        (false, true) => {
            let shift = (start + length) - range.start;
            let start = start;
            let length = length - shift;
            if length == 0 {
                None
            } else {
                Some((start, length))
            }
        }
        _ => Some((start, length)),
    };
    result
}

fn exclude_ranges(
    mdesc: &MemoryDescriptor,
    ranges: &[ops::Range<usize>],
) -> Option<(usize, usize)> {
    let mut block = Some((
        mdesc.phys_start as usize,
        (mdesc.page_count * 0x1000) as usize,
    ));

    for range in ranges {
        block = exclude_range(block?, range.clone())
    }

    block
}

/// Present; when 0, the entry is ignored
const PRESENT_FLAG: usize = 1;

/// Read/write; if 0, writes are not allowed to the region that the entry controls.
const RW_FLAG: usize = 1 << 1;

/// User/supervisor; if 0, user-mode accesses are not allowed to the region that the entry controls.
const US_FLAG: usize = 1 << 2;

/// Page-level write-through
const PWT_FLAG: usize = 1 << 3;

/// Page-level cache disable
const PCD_FLAG: usize = 1 << 4;

/// Accessed; when 1, the entry has been used for linear address translation.
const ACCESSED_FLAG: usize = 1 << 5;

/// Dirty; when 1, the region that the entry controls has been written to.
const DIRTY_FLAG: usize = 1 << 6;

/// Page size; when 1, the entry directly maps memory. When 0, the entry references a subordinate paging structure.
const PS_FLAG: usize = 1 << 7;

/// Global; determines whether the translation is global.
const GLOBAL_FLAG: usize = 1 << 8;

/// Page attribute table; determines the memory thpe used to access the region that the entry controls.
const PAT_FLAG: usize = 1 << 12;

/// A paging structure entry.
#[repr(C)]
struct PageEntry {
    bytes: usize,
}

impl PageEntry {
    const fn new(flags: usize, phys_addr: usize) -> Self {
        PageEntry {
            bytes: flags | (phys_addr & MASK_51_12),
        }
    }

    fn get_flags(&self, flag: usize) -> usize {
        self.bytes & flag
    }

    fn set_flags(&mut self, flag: usize, value: bool) {
        if value {
            self.bytes |= flag;
        } else {
            self.bytes &= !flag;
        }
    }
}
