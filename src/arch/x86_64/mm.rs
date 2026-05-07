use alloc::vec::Vec;
use core::arch::asm;
use core::ops;
use core::ptr;
use core::slice::from_raw_parts;

use uefi::table::boot::{MemoryDescriptor, MemoryType};

use crate::locking::spinlock::WithSpinLock;
use crate::mm::malloc;
use paging::{
    physical, read_cr3, Mapper, PageEntry, MASK_20_0, MASK_29_0, MASK_29_21, MASK_38_30,
    MASK_47_30, MASK_47_39, MASK_51_12, MASK_51_21, MASK_51_30, PAGING_STRUCTURE_BASE,
    PRESENT_FLAG, PS_FLAG, RW_FLAG,
};

extern "C" {
    #[link_name = "boot_pml4"]
    static mut KERNEL_PML4: [PageEntry; 512];

    #[link_name = "boot_pdpt"]
    static mut BOOT_PDPT: [PageEntry; 512];

    #[link_name = "boot_pdt"]
    static mut BOOT_PDT: [PageEntry; 512];

    #[link_name = "boot_paging_pdpt"]
    static mut BOOT_PAGING_PDPT: [PageEntry; 512];

    #[link_name = "boot_paging_pdt"]
    static mut BOOT_PAGING_PDT: [PageEntry; 512];
}

static PHYSICAL_PAGE_ALLOCATOR: WithSpinLock<physical::PageAllocator> =
    WithSpinLock::new(physical::PageAllocator::new());

pub const KERNEL_BASE: usize = 0xffff800000000000;

/// init_mm() (re)-initializes paging data structures for kernel execution.
/// It also sets up paging for the kernel heap located at KERNEL_BASE + 512MiB
pub fn init_mm(memory_map: &[MemoryDescriptor]) {
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

    // Map first 2MiB of paging structures to 0xffff_ff80_0020_0000.
    let pml4 = unsafe { &raw mut KERNEL_PML4 };
    let virt_addr = PAGING_STRUCTURE_BASE + read_cr3();
    let idx = (virt_addr & MASK_47_39) >> 39;
    unsafe {
        let pdpt = &raw mut BOOT_PAGING_PDPT;
        let pdpt = pdpt as usize - KERNEL_BASE;
        (*pml4)[idx] = PageEntry::new(0x3, pdpt)
    }

    let pdpt = unsafe { &raw mut BOOT_PAGING_PDPT };
    let idx = (virt_addr & MASK_38_30) >> 30;
    unsafe {
        let pdt = &raw mut BOOT_PAGING_PDT;
        let pdt = pdt as usize - KERNEL_BASE;
        (*pdpt)[idx] = PageEntry::new(PRESENT_FLAG | RW_FLAG, pdt)
    }

    let pdt = unsafe { &raw mut BOOT_PAGING_PDT };
    let idx = (virt_addr & MASK_29_21) >> 21;
    unsafe { (*pdt)[idx] = PageEntry::new(0x83, 0x200000) }

    flush_tlb();

    // TODO: initialize virtual memory mapper with PAGING_STRUCTURE_BASE, and initial offset of 0x7000
    let mut mapper = Mapper::new(virt_addr, 0x200000, 7);
    mapper.map(0x800000, 0xffff800000600000, 0x1000);
    flush_tlb();
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
        let pdpt = (KERNEL_BASE | pml4e.get_addr()) as *const PageEntry;
        let pdpt = from_raw_parts(pdpt, 512);
        let idx = (linear_addr as usize & MASK_38_30) >> 30;
        &pdpt[idx]
    };

    // If PS = 1
    if pdpte.get_flags(PS_FLAG) & PS_FLAG == PS_FLAG {
        (pdpte.get_addr() & MASK_51_30 | (linear_addr as usize) & MASK_29_0) as *const u8
    } else {
        let pdte = unsafe {
            let pdt = (KERNEL_BASE | pdpte.get_addr()) as *const PageEntry;
            let pdt = from_raw_parts(pdt, 512);
            let idx = (linear_addr as usize & MASK_29_21) >> 21;
            &pdt[idx]
        };
        if pdte.get_flags(0x80) == 0x80 {
            (pdte.get_addr() & MASK_51_21 | (linear_addr as usize) & MASK_20_0) as *const u8
        } else {
            // Mappings by PTs are currently unsupported.
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
