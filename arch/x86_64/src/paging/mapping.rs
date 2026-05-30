use super::*;
use crate::paging::table::{PagingStruct, PRESENT_FLAG, PS_FLAG, RW_FLAG};

use core::ptr::write_bytes;

use interface::Environment;
use paging_common::physical::PageAllocator;

const BOOT_PAGE_TABLE_COUNT: usize = 7;

// TODO: make this a trait if we support architectures other than x86_64.
pub struct Mapper<E: Environment + Clone> {
    base: *mut PagingStruct<E>,
    length: usize,
    // TODO: Bump style allocation will break with offset mapping.
    //       Use better allocation method
    next: usize,

    page_allocator: PageAllocator,
    environment: E,
}

impl<E: Environment + Clone> Mapper<E> {
    pub fn new(
        base: *mut PagingStruct<E>,
        length: usize,
        next: usize,
        page_allocator: PageAllocator,
        environment: E,
    ) -> Self {
        let ptr = unsafe { base.add(7) } as *mut u8;
        unsafe {
            write_bytes(ptr, 0u8, length - BOOT_PAGE_TABLE_COUNT * 0x1000);
        }
        Mapper {
            base,
            length,
            next,
            page_allocator,
            environment,
        }
    }

    pub fn map(&mut self, phys_addr: usize, virt_addr: usize) {
        let mut mask = MASK_47_39;
        let mut shift = 39;
        let mut page_table = self.environment.paging_structure_base() as *mut PagingStruct<E>;
        // let mut page_table = unsafe { &mut *page_table };
        for _ in 0..3usize {
            let idx = (virt_addr & mask) >> shift;
            mask >>= 9;
            shift -= 9;

            let (present, next_virt) = unsafe {
                let entry = (*page_table).get_entry(idx);
                (
                    entry.get_flags(PRESENT_FLAG) == PRESENT_FLAG,
                    entry.get_virt_addr(),
                )
            };
            let next_ptr = if present {
                next_virt as *mut PagingStruct<E>
            } else {
                // TODO: if remaining page count <=3, extend page table region.
                let new_table_ptr = self.new_table();
                let new_phys = unsafe { (*new_table_ptr).phys_addr() };

                unsafe {
                    let entry = (*page_table).get_entry_mut(idx);
                    entry.set_addr(new_phys);
                    entry.set_flags(PRESENT_FLAG | RW_FLAG, true);
                }
                new_table_ptr
            };

            page_table = next_ptr;
        }
        let idx = (virt_addr & MASK_20_12) >> 12;
        unsafe {
            let pte = (*page_table).get_entry_mut(idx);
            pte.set_addr(phys_addr & MASK_51_12);
            pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
        }
        self.environment.flush_tlb();
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

        // Start with PML4.
        let mut page_table = unsafe {
            self.environment
                .paging_structure_base()
                .add(E::PAGING_STRUCTURE_BASE)
        } as *const PagingStruct<E>;

        // Initialize with PML4E.
        let idx = (virt_addr & mask) >> shift;
        let (mut entry_addr, mut entry_flags, mut entry_phys) = unsafe {
            let entry = (*page_table).get_entry(idx);
            (entry.get_addr(), entry.get_flags(!0), entry.get_addr())
        };

        // Traverse maximum 3 times; PML4 -> PDPT -> PD -> PT
        for _ in 0..3usize {
            if entry_flags & PS_FLAG != PS_FLAG {
                page_table = self.table_for_phys_addr(entry_phys);
            } else {
                break;
            }
            mask >>= 9;
            shift -= 9;
            let idx = (virt_addr & mask) >> shift;
            let (addr, flags, phys_addr) = unsafe {
                let entry = (*page_table).get_entry(idx);
                (entry.get_addr(), entry.get_flags(!0), entry.get_addr())
            };
            entry_addr = addr;
            entry_flags = flags;
            entry_phys = phys_addr;
        }
        let _ = entry_flags;
        entry_addr.wrapping_add(virt_addr & !(0xFFFF_FFFF_FFFF_FFFF - ((1 << shift) - 1)))
    }

    pub fn fork(&mut self, paging_struct_base: *mut PagingStruct<E>) -> usize {
        let src_pml4 = paging_struct_base;
        let src_pml4 = unsafe { &*src_pml4 };
        let dst_pml4 = self.new_table();
        let dst_pml4 = unsafe { &mut *(dst_pml4 as *mut PagingStruct<E>) };

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
            let dst_table = self.new_table();
            let dst_table = unsafe { &mut *(dst_table as *mut PagingStruct<E>) };
            dst_pml4.get_entry_mut(i).set_addr(dst_table.phys_addr());
            let src_table = entry.get_virt_addr() as *mut PagingStruct<E>;
            let src_table = unsafe { &mut *src_table };
            self.recursively_clone(src_table, dst_table, 3);
        }

        dst_pml4.phys_addr()
    }

    fn recursively_clone(
        &mut self,
        src: &mut PagingStruct<E>,
        dst: &mut PagingStruct<E>,
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
                let dst_table = dst_table;
                let dst_table = unsafe { &mut *(dst_table as *mut PagingStruct<E>) };
                dst_entry.set_addr(dst_table.phys_addr());

                let src_table = src_entry.get_virt_addr();
                let src_table = src_table as *mut PagingStruct<E>;
                let src_table = unsafe { &mut *src_table };
                self.recursively_clone(src_table, dst_table, level - 1);
            }
        }
    }

    fn new_table(&mut self) -> *mut PagingStruct<E> {
        let new_table = unsafe { self.base.add(self.next) };
        self.next += 1;
        new_table
    }

    fn table_for_phys_addr(&self, phys_addr: usize) -> *mut PagingStruct<E> {
        unsafe {
            let idx = (E::PAGING_STRUCTURE_BASE + phys_addr - self.base as usize)
                / size_of::<PagingStruct<E>>();
            self.base.add(idx)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use interface::Environment;

    #[derive(Copy, Clone)]
    struct UserlandTest(*mut u8);

    impl Environment for UserlandTest {
        const PAGING_STRUCTURE_BASE: usize = 0;
        fn paging_structure_base(&self) -> *mut u8 {
            self.0
        }
        fn flush_tlb(&self) {}
    }

    const PAGING_STRUCTURE_REGION_LEN: usize = 0x200000 / size_of::<PagingStruct<UserlandTest>>();

    #[test]
    fn test_new_table() {
        let allocator = PageAllocator::new();
        let layout =
            core::alloc::Layout::new::<[PagingStruct<UserlandTest>; PAGING_STRUCTURE_REGION_LEN]>();
        let base = unsafe { alloc::alloc::alloc_zeroed(layout) };
        let fake_native = UserlandTest(base);
        let mut mapper = Mapper::new(
            base as *mut PagingStruct<UserlandTest>,
            0x200000,
            0,
            allocator,
            fake_native,
        );
        let first_table = mapper.new_table();
        // assert_eq!(mapper.next, 1, "Next table should be offset 1");
        assert_eq!(
            first_table as *const PagingStruct<UserlandTest> as usize, base as usize,
            "First table should be at base"
        );

        let second_table = mapper.new_table();
        // assert_eq!(mapper.next, 2, "Next table should be offset 2");
        assert_eq!(
            second_table as *const PagingStruct<UserlandTest> as usize,
            base as usize + 0x1000,
            "Second table should be at base + 0x1000"
        );

        unsafe { alloc::alloc::dealloc(base, layout) };
    }

    #[test]
    fn test_map_phys_addr() {
        let allocator = PageAllocator::new();
        let layout =
            core::alloc::Layout::new::<[PagingStruct<UserlandTest>; PAGING_STRUCTURE_REGION_LEN]>();
        let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
        let fake_native = UserlandTest(base);
        let mut mapper = Mapper::new(
            base as *mut PagingStruct<UserlandTest>,
            0x200000,
            0,
            allocator,
            fake_native,
        );

        let phys_addr = 0xdeadbeefusize;
        // The following is not UserlandTest::PAGING_STRUCTURE_BASE on purpose.
        // Would like to test the mapping on something that is not an identity map.
        let virt_addr = phys_addr + PAGING_STRUCTURE_BASE;
        mapper.map(phys_addr, virt_addr);

        let got_phys_addr = mapper.phys_addr(virt_addr);
        assert_eq!(
            got_phys_addr, phys_addr,
            "Physical address should match: {got_phys_addr:#x} != {phys_addr:#x}"
        );

        unsafe { alloc::alloc::dealloc(base, layout) };
    }
}
