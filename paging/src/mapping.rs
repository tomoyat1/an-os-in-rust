use super::*;
use core::ops::Add;

const BOOT_PAGE_TABLE_COUNT: usize = 7;

pub struct Mapper {
    base: usize,
    length: usize,
    // TODO: Bump style allocation will break with offset mapping.
    //       Use better allocation method
    next: usize,
}

impl Mapper {
    pub fn new(base: usize, length: usize, next: usize) -> Self {
        let ptr = base as *mut u8;
        for i in BOOT_PAGE_TABLE_COUNT * 0x1000..length {
            unsafe {
                ptr.add(i).write_volatile(0);
            }
        }
        Mapper { base, length, next }
    }
    pub fn map(&mut self, phys_addr: usize, virt_addr: usize, size: usize) {
        let mut mask = MASK_47_39;
        let mut shift = 39;
        let mut page_table = (read_cr3() + PAGING_STRUCTURE_BASE) as *mut PageEntry;
        for _ in 0..3usize {
            let idx = ((virt_addr & mask) >> shift) as isize;
            mask >>= 9;
            shift -= 9;
            let mut entry = unsafe {
                let entry = page_table.offset(idx);
                &mut *entry
            };
            page_table = if entry.get_flags(PRESENT_FLAG) == PRESENT_FLAG {
                (entry.get_addr() + PAGING_STRUCTURE_BASE) as *mut PageEntry
            } else {
                // TODO: if remaining page count <=3, extend page table region.
                let new_table = self.new_table();
                entry.set_addr(new_table - PAGING_STRUCTURE_BASE);
                entry.set_flags(PRESENT_FLAG | RW_FLAG, true);
                new_table as *mut PageEntry
            }
        }
        let idx = ((virt_addr & MASK_20_12) >> 12) as isize;
        let pte = unsafe {
            let pte = page_table.offset(idx);
            &mut *pte
        };
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
    }

    fn new_table(&mut self) -> usize {
        let new_table = self.base + self.next * 0x1000;
        self.next += 1;
        new_table
    }
}
