use super::*;
use crate::paging::table::{PagingStruct, PRESENT_FLAG, PS_FLAG, RW_FLAG};

use core::ptr::write_bytes;

use paging_common::physical::PageAllocator;

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
            write_bytes(ptr, 0u8, length - BOOT_PAGE_TABLE_COUNT * 0x1000);
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
        let mut page_table = (read_cr3() + PAGING_STRUCTURE_BASE) as *mut PagingStruct;
        let mut page_table = unsafe { &mut *page_table };
        for _ in 0..3usize {
            let idx = (virt_addr & mask) >> shift;
            mask >>= 9;
            shift -= 9;
            let entry = unsafe { (*page_table).get_entry_mut(idx) };
            page_table = if entry.get_flags(PRESENT_FLAG) == PRESENT_FLAG {
                unsafe { &mut *(entry.get_virt_addr() as *mut PagingStruct) }
            } else {
                // TODO: if remaining page count <=3, extend page table region.
                let new_table = self.new_table() as *mut PagingStruct;
                let new_table = unsafe { &mut *new_table };
                entry.set_addr(new_table.phys_addr());
                entry.set_flags(PRESENT_FLAG | RW_FLAG, true);
                new_table
            }
        }
        let idx = (virt_addr & MASK_20_12) >> 12;
        let pte = unsafe { (*page_table).get_entry_mut(idx) };
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
        let mut page_table = (read_cr3() + PAGING_STRUCTURE_BASE) as *mut PagingStruct;
        let mut page_table = unsafe { &mut *page_table };

        // Unconditionally traverse once from PML4 to PDPT.
        let idx = (virt_addr & mask) >> shift;
        let mut entry = page_table.get_entry(idx);
        page_table = unsafe { &mut *(entry.get_virt_addr() as *mut PagingStruct) };

        for _ in 0..3usize {
            mask >>= 9;
            shift -= 9;
            let idx = (virt_addr & mask) >> shift;
            entry = page_table.get_entry(idx);
            if entry.get_flags(PS_FLAG) != PS_FLAG {
                page_table = unsafe { &mut *(entry.get_virt_addr() as *mut PagingStruct) }
            } else {
                break;
            }
        }
        entry.get_addr() + (virt_addr & !(0xFFFF_FFFF_FFFF_FFFF - ((1 << shift) - 1)))
    }

    pub fn fork(&mut self, cr3: usize) -> usize {
        let mut src_pml4 = (cr3 + PAGING_STRUCTURE_BASE) as *mut PagingStruct;
        let src_pml4 = unsafe { &*src_pml4 };
        let dst_pml4 = self.new_table() as *mut PagingStruct;
        let dst_pml4 = unsafe { &mut *dst_pml4 };

        // Shallow copy kernel address space
        for i in 256..512usize {
            let entry = src_pml4.get_entry(i);
            if entry.get_flags(PRESENT_FLAG) != PRESENT_FLAG {
                continue;
            }
            *(dst_pml4.get_entry_mut(i)) = entry.clone();
        }
        for i in 0..256usize {
            let entry = src_pml4.get_entry(i);
            if entry.get_flags(PRESENT_FLAG) != PRESENT_FLAG {
                continue;
            }
            *(dst_pml4.get_entry_mut(i)) = entry.clone();
            let dst_table = self.new_table() as *mut PagingStruct;
            let dst_table = unsafe { &mut *dst_table };
            dst_pml4.get_entry_mut(i).set_addr(dst_table.phys_addr());
            let src_table = entry.get_virt_addr() as *mut PagingStruct;
            let src_table = unsafe { &mut *src_table };
            self.recursively_clone(src_table, dst_table, 3);
        }

        dst_pml4.phys_addr()
    }

    fn recursively_clone(
        &mut self,
        src: &mut PagingStruct,
        dst: &mut PagingStruct,
        level: usize, // 3: pdpt, 2: pdt, 1: pt
    ) {
        for i in 0..512usize {
            let src_entry = src.get_entry_mut(i);
            let dst_entry = dst.get_entry_mut(i);
            if level == 1 {
                // src_entry is a pte
                if src_entry.get_flags(PRESENT_FLAG) != PRESENT_FLAG {
                    continue;
                }
                *dst_entry = src_entry.clone();
                dst_entry.set_flags(RW_FLAG, false);
                src_entry.set_flags(RW_FLAG, false);
            } else {
                // src_entry is a pdpt or pdt
                if src_entry.get_flags(PRESENT_FLAG) != PRESENT_FLAG {
                    continue;
                }
                *dst_entry = src_entry.clone();
                let dst_table = self.new_table();
                let dst_table = dst_table as *mut PagingStruct;
                let dst_table = unsafe { &mut *dst_table };
                dst_entry.set_addr(dst_table.phys_addr());

                let src_table = src_entry.get_virt_addr();
                let src_table = src_table as *mut PagingStruct;
                let src_table = unsafe { &mut *src_table };
                self.recursively_clone(src_table, dst_table, level - 1);
            }
        }
    }

    fn new_table(&mut self) -> usize {
        let new_table = self.base + self.next * 0x1000;
        self.next += 1;
        let ptr = new_table as *mut u8;
        unsafe {
            write_bytes(ptr, 0u8, 0x1000);
        }
        new_table
    }
}
