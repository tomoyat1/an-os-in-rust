use super::*;

#[test]
fn test_map() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let phys_addr = 0xdeadb000usize;
    // The following is not UserlandTest::PAGING_STRUCTURE_BASE on purpose.
    // Would like to test the mapping on something that is not an identity map.
    let virt_addr = phys_addr + PAGING_STRUCTURE_BASE;
    mapper.map(phys_addr, virt_addr);

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct;
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
        assert_eq!(
            pte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PTE should be present"
        );
        assert_eq!(
            page_phys_addr,
            phys_addr & MASK_47_12,
            "Physical page address should match"
        );

        unsafe { alloc::alloc::dealloc(base, layout) };
    }
}

#[test]
fn test_map_userland() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let phys_addr = 0xdeadb000usize;
    let virt_addr = 0x1000usize;
    mapper.map(phys_addr, virt_addr);

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct;
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
        assert_eq!(
            pte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PTE should be present"
        );
        assert_eq!(
            page_phys_addr,
            phys_addr & MASK_47_12,
            "Physical page address should match"
        );

        let mp = mapper
            .mapped_pages
            .get(&page_phys_addr)
            .expect("MappedPage for address should exist");

        assert_eq!(
            mp.refs.load(SeqCst),
            1,
            "Mapped page ref count should be 1 after allocation"
        );
        assert_eq!(
            mp.aliasing_paging_structures.len(),
            1,
            "Should have one aliasing paging structure"
        );
        let alias = mp
            .aliasing_paging_structures
            .first()
            .expect("There should be exactly one aliasing paging structure");
        assert_eq!(
            *alias, pml4 as usize,
            "pml4 should be the only aliasing paging structure"
        );

        unsafe { alloc::alloc::dealloc(base, layout) };
    }
}
