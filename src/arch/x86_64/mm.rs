use alloc::vec::Vec;
use core::arch::asm;
use core::ops;
use core::ptr;
use core::slice::from_raw_parts;

use uefi::table::boot::{MemoryDescriptor, MemoryType};
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use crate::mm::malloc;
use paging::{
    flush_tlb, physical, read_cr3, Mapper, PageEntry, MASK_20_0, MASK_29_0, MASK_29_21, MASK_38_30,
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

static MAPPER: WithSpinLock<Option<Mapper>> = WithSpinLock::new(None);

pub const KERNEL_BASE: usize = 0xffff_8000_0000_0000;
pub const MMIO_BASE: usize = 0xffff_ff00_0000_0000;

/// init_mm() (re)-initializes paging data structures for kernel execution.
pub fn init_mm(memory_map: &[MemoryDescriptor]) {
    let mut free_blocks = Vec::<(usize, usize)>::new();

    for mdesc in memory_map {
        match mdesc.ty {
            MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::CONVENTIONAL => {
                if let Some(block) = exclude_ranges(mdesc, &[0..0x800000]) {
                    free_blocks.push(block)
                }
            }
            _ => { /* noop */ }
        }
    }

    let mut allocator = physical::PageAllocator::new();
    allocator.init(&free_blocks);

    // Map first 2MiB of paging structures to 0xffff_ff80_0020_0000.
    let pml4 = unsafe { &raw mut KERNEL_PML4 };
    let page_structure_base = PAGING_STRUCTURE_BASE + read_cr3();
    let idx = (page_structure_base & MASK_47_39) >> 39;
    unsafe {
        let pdpt = &raw mut BOOT_PAGING_PDPT;
        let pdpt = pdpt as usize - KERNEL_BASE;
        (*pml4)[idx] = PageEntry::new(0x3, pdpt)
    }

    let pdpt = unsafe { &raw mut BOOT_PAGING_PDPT };
    let idx = (page_structure_base & MASK_38_30) >> 30;
    unsafe {
        let pdt = &raw mut BOOT_PAGING_PDT;
        let pdt = pdt as usize - KERNEL_BASE;
        (*pdpt)[idx] = PageEntry::new(PRESENT_FLAG | RW_FLAG, pdt)
    }

    let pdt = unsafe { &raw mut BOOT_PAGING_PDT };
    let idx = (page_structure_base & MASK_29_21) >> 21;
    unsafe { (*pdt)[idx] = PageEntry::new(0x83, 0x200000) }

    flush_tlb();

    let mut mapper = Mapper::new(page_structure_base, 0x200000, 7, allocator);
    {
        let mut m = MAPPER.lock();
        *m = Some(mapper)
    }
    MAPPER
        .lock()
        .as_mut()
        .unwrap()
        .alloc_page_at(0xffff800000600000);

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
            | MemoryType::LOADER_DATA
            | MemoryType::MMIO
            | MemoryType::MMIO_PORT_SPACE => {
                for p in 0..mdesc.page_count {
                    let phys_addr = (mdesc.phys_start + p * 0x1000) as usize;
                    let virt_addr = MMIO_BASE + (mdesc.phys_start + p * 0x1000) as usize;
                    MAPPER.lock().as_mut().unwrap().map(phys_addr, virt_addr);
                }
            }
            _ => { /* noop */ }
        }
    }

    let pml4 = unsafe { &raw mut KERNEL_PML4 };
    let page_structure_base = PAGING_STRUCTURE_BASE + read_cr3();
    unsafe {
        let pml4e = &mut (*pml4)[0];
        pml4e.set_flags(PRESENT_FLAG, false)
    }

    flush_tlb();
}

/// phys_addr returns the physical address for `linear_address`.
pub fn phys_addr(linear_addr: *const u8) -> *const u8 {
    MAPPER
        .lock()
        .as_mut()
        .unwrap()
        .phys_addr(linear_addr as usize) as *const u8
}

pub fn mapper() -> WithSpinLockGuard<'static, Option<Mapper>> {
    MAPPER.lock()
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
