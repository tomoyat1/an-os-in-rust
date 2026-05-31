use super::*;
use crate::paging::table::{PagingStruct, ALL_FLAGS, PRESENT_FLAG, PS_FLAG, RW_FLAG};

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
        let (mut entry_addr, mut entry_flags) = unsafe {
            let entry = (*page_table).get_entry(idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };

        // Traverse maximum 3 times; PML4 -> PDPT -> PD -> PT
        for _ in 0..3usize {
            if entry_flags & PS_FLAG != PS_FLAG {
                page_table = self.table_for_phys_addr(entry_addr);
            } else {
                break;
            }
            mask >>= 9;
            shift -= 9;
            let idx = (virt_addr & mask) >> shift;
            let (addr, flags) = unsafe {
                let entry = (*page_table).get_entry(idx);
                (entry.get_addr(), entry.get_flags(ALL_FLAGS))
            };
            entry_addr = addr;
            entry_flags = flags;
        }
        let _ = entry_flags;
        entry_addr.wrapping_add(virt_addr & !(0xFFFF_FFFF_FFFF_FFFF - ((1 << shift) - 1)))
    }

    pub fn fork(&mut self, paging_struct_base: *mut PagingStruct<E>) -> usize {
        let src_pml4 = paging_struct_base;
        let dst_pml4 = self.new_table();

        // Shallow copy kernel address space
        for i in 256..512usize {
            let (entry_addr, flags) = unsafe {
                let entry = (*src_pml4).get_entry(i);
                (entry.get_addr(), entry.get_flags(ALL_FLAGS))
            };
            if flags & PRESENT_FLAG != PRESENT_FLAG {
                continue;
            }
            unsafe {
                let dst_entry = (*dst_pml4).get_entry_mut(i);
                dst_entry.set_addr(entry_addr);
                dst_entry.set_flags(flags, true);
            }
        }
        for i in 0..256usize {
            let (entry_addr, flags) = unsafe {
                let entry = (*src_pml4).get_entry(i);
                (entry.get_addr(), entry.get_flags(ALL_FLAGS))
            };
            if flags & PRESENT_FLAG != PRESENT_FLAG {
                continue;
            }
            let dst_pdpt = self.new_table();
            unsafe {
                (*dst_pml4).get_entry_mut(i).set_flags(flags, true);
                let dst_table_addr = (*dst_pdpt).phys_addr();
                (*dst_pml4).get_entry_mut(i).set_addr(dst_table_addr)
            }

            let src_pdpt = unsafe {
                let entry = (*src_pml4).get_entry(i);
                let phys_addr = entry.get_addr();
                self.table_for_phys_addr(phys_addr)
            };
            self.recursively_clone(src_pdpt, dst_pdpt, 3);
        }

        unsafe { (*dst_pml4).phys_addr() }
    }

    fn recursively_clone(
        &mut self,
        src: *mut PagingStruct<E>,
        dst: *mut PagingStruct<E>,
        level: usize, // 2: pdpt -> pd, 1: pd -> pt
    ) {
        for i in 0..512usize {
            if level == 1 {
                // src is a pt
                let (src_addr, src_flags) = unsafe {
                    let src_entry = (*src).get_entry(i);
                    (src_entry.get_addr(), src_entry.get_flags(ALL_FLAGS))
                };
                if src_flags & PRESENT_FLAG != PRESENT_FLAG {
                    continue;
                }
                unsafe {
                    let dst_entry = (*dst).get_entry_mut(i);
                    dst_entry.set_addr(src_addr);
                    dst_entry.set_flags(src_flags, true);
                    dst_entry.set_flags(RW_FLAG, false)
                }
                unsafe {
                    let src_entry = (*src).get_entry_mut(i);
                    src_entry.set_flags(RW_FLAG, false);
                }
            } else {
                // src_entry is a pdpte or pde
                let (src_ent_addr, src_ent_flags) = unsafe {
                    let src_entry = (*src).get_entry(i);
                    (src_entry.get_addr(), src_entry.get_flags(ALL_FLAGS))
                };
                if src_ent_flags & PRESENT_FLAG != PRESENT_FLAG {
                    continue;
                }
                let dst_table = self.new_table();
                unsafe {
                    let dst_entry = (*dst).get_entry_mut(i);
                    let dst_table_addr = (*dst_table).phys_addr();
                    dst_entry.set_addr(dst_table_addr);
                    dst_entry.set_flags(src_ent_flags, true);
                    dst_entry.set_flags(RW_FLAG, false)
                }

                let src_table = unsafe {
                    let src_entry = (*src).get_entry(i);
                    let addr = src_entry.get_addr();
                    self.table_for_phys_addr(addr)
                };
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
            1,
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

    #[test]
    fn test_fork() {
        let allocator = PageAllocator::new();
        let layout =
            core::alloc::Layout::new::<[PagingStruct<UserlandTest>; PAGING_STRUCTURE_REGION_LEN]>();
        let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
        let fake_native = UserlandTest(base);
        // `next = 1` reserves index 0 for the source PML4 (located at `base`).
        let mut mapper = Mapper::new(
            base as *mut PagingStruct<UserlandTest>,
            0x200000,
            1,
            allocator,
            fake_native,
        );

        let phys_addr = 0x0000_beefusize;
        let virt_addr = 0x0000_0000_dead_beefusize;

        let pml4_idx = (virt_addr & MASK_47_39) >> 39;
        let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
        let pd_idx = (virt_addr & MASK_29_21) >> 21;
        let pt_idx = (virt_addr & MASK_20_12) >> 12;

        let src_pml4 = base as *mut PagingStruct<UserlandTest>;

        let src_pdpt = mapper.new_table();
        let src_pd = mapper.new_table();
        let src_pt = mapper.new_table();

        unsafe {
            let pml4e = (*src_pml4).get_entry_mut(pml4_idx);
            pml4e.set_addr((*src_pdpt).phys_addr());
            pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

            let pdpte = (*src_pdpt).get_entry_mut(pdpt_idx);
            pdpte.set_addr((*src_pd).phys_addr());
            pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

            let pde = (*src_pd).get_entry_mut(pd_idx);
            pde.set_addr((*src_pt).phys_addr());
            pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

            let pte = (*src_pt).get_entry_mut(pt_idx);
            pte.set_addr(phys_addr & MASK_51_12);
            pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
        }

        let new_table_base = mapper.fork(src_pml4);
        let new_pml4 = mapper.table_for_phys_addr(new_table_base);
        assert_ne!(
            new_pml4 as usize, src_pml4 as usize,
            "Cloned PML4 must be a different table from the source PML4"
        );

        let (dst_pdpt_addr, dst_pml4e_flags) = unsafe {
            let entry = (*new_pml4).get_entry(pml4_idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        assert_eq!(
            dst_pml4e_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "Dst PML4E must be present"
        );
        let src_pdpt_addr = unsafe { (*src_pml4).get_entry(pml4_idx).get_addr() };
        assert_ne!(
            dst_pdpt_addr, src_pdpt_addr,
            "Dst PDPT must be a fresh table, not aliasing src PDPT"
        );

        let dst_pdpt = mapper.table_for_phys_addr(dst_pdpt_addr);
        let (dst_pd_addr, dst_pdpte_flags) = unsafe {
            let entry = (*dst_pdpt).get_entry(pdpt_idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        assert_eq!(
            dst_pdpte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "Dst PDPTE must be present"
        );
        let src_pd_addr = unsafe { (*src_pdpt).get_entry(pdpt_idx).get_addr() };
        assert_ne!(
            dst_pd_addr, src_pd_addr,
            "Dst PD must be a fresh table, not aliasing src PD"
        );

        let dst_pd = mapper.table_for_phys_addr(dst_pd_addr);
        let (dst_pt_addr, dst_pde_flags) = unsafe {
            let entry = (*dst_pd).get_entry(pd_idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        assert_eq!(
            dst_pde_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "Dst PDE must be present"
        );
        let src_pt_addr = unsafe { (*src_pd).get_entry(pd_idx).get_addr() };
        assert_ne!(
            dst_pt_addr, src_pt_addr,
            "Dst PT must be a fresh table, not aliasing src PT"
        );

        let dst_pt = mapper.table_for_phys_addr(dst_pt_addr);

        let (dst_leaf_addr, dst_leaf_flags) = unsafe {
            let entry = (*dst_pt).get_entry(pt_idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        let (src_leaf_addr, src_leaf_flags) = unsafe {
            let entry = (*src_pt).get_entry(pt_idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        assert_eq!(
            dst_leaf_addr,
            phys_addr & MASK_51_12,
            "Dst leaf PTE must point at the same physical page"
        );
        assert_eq!(
            src_leaf_addr,
            phys_addr & MASK_51_12,
            "Src leaf PTE must still point at the same physical page"
        );
        assert_eq!(
            dst_leaf_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "Dst leaf PTE must be present"
        );
        assert_eq!(
            dst_leaf_flags & RW_FLAG,
            0,
            "Dst leaf PTE must have RW cleared (COW)"
        );
        assert_eq!(
            src_leaf_flags & RW_FLAG,
            0,
            "Src leaf PTE must have RW cleared (COW)"
        );

        unsafe { alloc::alloc::dealloc(base, layout) };
    }
}
