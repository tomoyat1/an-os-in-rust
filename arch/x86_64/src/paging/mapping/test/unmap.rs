use super::*;
use paging_common::physical::PageAllocator;

#[test]
fn test_unmap() {
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
        core::ptr::null_mut(),
    );

    let phys_addr = 0x0000_b000usize;
    let virt_addr = 0x0000_0000_dead_b000usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct<UserlandTest>;

    let pdpt = mapper.new_table();
    let pd = mapper.new_table();
    let pt = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr((*pt).phys_addr());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*pt).get_entry_mut(pt_idx);
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
    }

    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Normal,
            refs: AtomicUsize::new(1),
        },
    );

    mapper.unmap(virt_addr);

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        let (pdpt_phys_addr, pml4e_flags) = (pml4e.get_addr(), pml4e.get_flags(ALL_FLAGS));
        assert_eq!(
            pml4e_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PML4E should be present"
        );
        let pdpt = mapper.table_for_phys_addr(pdpt_phys_addr);
        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        let (pd_phys_addr, pdpte_flags) = (pdpte.get_addr(), pdpte.get_flags(ALL_FLAGS));
        assert_eq!(
            pdpte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDPTE should be present"
        );
        let pd = mapper.table_for_phys_addr(pd_phys_addr);
        let pde = (*pd).get_entry_mut(pd_idx);
        let (pt_phys_addr, pde_flags) = (pde.get_addr(), pde.get_flags(ALL_FLAGS));
        assert_eq!(
            pde_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDE should be present"
        );
        let pt = mapper.table_for_phys_addr(pt_phys_addr);
        let pte = (*pt).get_entry_mut(pt_idx);
        let (page_phys_addr, pte_flags) = (pte.get_addr(), pte.get_flags(ALL_FLAGS));
        assert_eq!(pte_flags & PRESENT_FLAG, 0, "PTE should not be present");
        assert_eq!(
            page_phys_addr, 0,
            "Physical page address should be 0, got {page_phys_addr:#x}"
        );
    }

    assert!(
        mapper.mapped_pages.get(&phys_addr).is_none(),
        "Unmapped page should not be in mapped_pages"
    );

    unsafe { alloc::alloc::dealloc(base, layout) };
}

// TODO: add tests for huge and gigantic pages
