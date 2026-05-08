use super::*;
use crate::physical::PageAllocator;

const BOOT_PAGE_TABLE_COUNT: usize = 7;

// TODO: make this a trait if we support architectures other than x86_64.
pub struct Mapper {
    base: usize,
    length: usize,
    // TODO: Bump style allocation will break with offset mapping.
    //       Use better allocation method
    next: usize,

    page_allocator: PageAllocator,
}

impl Mapper {
    pub fn new(base: usize, length: usize, next: usize, page_allocator: PageAllocator) -> Self {
        let ptr = (base + BOOT_PAGE_TABLE_COUNT * 0x1000) as *mut u8;
        unsafe {
            core::ptr::write_bytes(ptr, 0u8, length - BOOT_PAGE_TABLE_COUNT * 0x1000);
        }
        Mapper {
            base,
            length,
            next,
            page_allocator,
        }
    }

    pub fn map(&mut self, phys_addr: usize, virt_addr: usize) {
        let mut mask = MASK_47_39;
        let mut shift = 39;
        let mut page_table = (read_cr3() + PAGING_STRUCTURE_BASE) as *mut [PageEntry; 512];
        for _ in 0..3usize {
            let idx = ((virt_addr & mask) >> shift);
            mask >>= 9;
            shift -= 9;
            let entry = unsafe { &mut (*page_table)[idx] };
            page_table = if entry.get_flags(PRESENT_FLAG) == PRESENT_FLAG {
                (entry.get_addr() + PAGING_STRUCTURE_BASE) as *mut [PageEntry; 512]
            } else {
                // TODO: if remaining page count <=3, extend page table region.
                let new_table = self.new_table();
                entry.set_addr(new_table - PAGING_STRUCTURE_BASE);
                entry.set_flags(PRESENT_FLAG | RW_FLAG, true);
                new_table as *mut [PageEntry; 512]
            }
        }
        let idx = ((virt_addr & MASK_20_12) >> 12);
        let pte = unsafe { &mut (*page_table)[idx] };
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
        flush_tlb();
    }
    pub fn alloc_page_at(&mut self, virt_addr: usize) {
        let phys_addr = self.page_allocator.allocate(12);
        match phys_addr {
            Some(phys_addr) => self.map(phys_addr.get_addr(), virt_addr),
            None => {
                panic!("No available physical pages!")
            }
        }
    }

    pub fn phys_addr(&self, virt_addr: usize) -> usize {
        let mut mask = MASK_47_39;
        let mut shift = 39;
        let mut page_table = (read_cr3() + PAGING_STRUCTURE_BASE) as *mut [PageEntry; 512];

        // Unconditionally traverse once from PML4 to PDPT.
        let idx = ((virt_addr & mask) >> shift);
        let mut entry = unsafe { &mut (*page_table)[idx] };
        page_table = (entry.get_addr() + PAGING_STRUCTURE_BASE) as *mut [PageEntry; 512];

        for _ in 0..3usize {
            mask >>= 9;
            shift -= 9;
            let idx = (virt_addr & mask) >> shift;
            entry = unsafe { &mut (*page_table)[idx] };
            if entry.get_flags(PS_FLAG) != PS_FLAG {
                page_table = (entry.get_addr() + PAGING_STRUCTURE_BASE) as *mut [PageEntry; 512]
            } else {
                break;
            }
        }
        entry.get_addr() + (virt_addr & !(0xFFFF_FFFF_FFFF_FFFF - ((1 << shift) - 1)))
    }

    fn new_table(&mut self) -> usize {
        let new_table = self.base + self.next * 0x1000;
        self.next += 1;
        new_table
    }
}
