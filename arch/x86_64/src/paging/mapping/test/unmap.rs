use super::*;
use paging_common::physical::PageAllocator;

#[test]
fn test_unmap() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    // `next = 1` reserves index 0 for the source PML4 (located at `base`).
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
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

    let pml4 = base as *mut PagingStruct;

    let pdpt = mapper.new_table();
    let pd = mapper.new_table();
    let pt = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr((*pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*pt).get_entry_mut(pt_idx);
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
    }

    let mut aliases = BTreeSet::new();
    aliases.insert((pml4 as usize, virt_addr));
    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Normal,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures: aliases,
        },
    );

    let result = mapper.unmap(virt_addr);

    assert!(result.is_ok(), "Unmapping should succeed");

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

#[test]
fn test_unmap_userland_aliased() {
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

    let phys_addr = 0x0000_b000usize;
    let virt_addr = 0x1000usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let other_pml4 = mapper.new_table();
    let other_pdpt = mapper.new_table();
    let other_pd = mapper.new_table();
    let other_pt = mapper.new_table();

    // This is what cr3 points to when unmap() is called.
    // Set this to base, which is what UserlandTest returns.
    let pml4 = base as *mut PagingStruct;
    let pdpt = mapper.new_table();
    let pd = mapper.new_table();
    let pt = mapper.new_table();

    // Construct mapping for other_pml4
    unsafe {
        let pml4e = (*other_pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*other_pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*other_pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*other_pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*other_pd).get_entry_mut(pd_idx);
        pde.set_addr((*other_pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*other_pt).get_entry_mut(pt_idx);
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG, true);
    }
    let mut aliasing_paging_structures = BTreeSet::new();
    aliasing_paging_structures.insert((other_pml4 as usize, virt_addr));
    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Normal,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures,
        },
    );

    // Construct mapping for pml4
    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr((*pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*pt).get_entry_mut(pt_idx);
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG, true);
    }
    let mp = mapper
        .mapped_pages
        .get_mut(&phys_addr)
        .expect("Mapped page should exist in mapped_pages");
    mp.refs.fetch_add(1, Ordering::Relaxed);
    mp.aliasing_paging_structures
        .insert((pml4 as usize, virt_addr));

    let result = mapper.unmap(virt_addr);
    assert!(result.is_ok(), "Unmapping should succeed");

    // The current mapping is torn down.
    unsafe {
        let leaf = mapper.walk_to_leaf(pml4, virt_addr);
        let pte = (*pt).get_entry_mut(pt_idx);
        assert_eq!(
            pte.get_flags(ALL_FLAGS) & PRESENT_FLAG,
            0,
            "Unmapped PTE should not be present"
        );
        // walk_to_leaf still resolves because intermediate tables remain, but the
        // leaf is no longer present.
        let _ = leaf;
    }

    // The page survives because another alias still references it, and that
    // surviving alias is restored to read-write since it is no longer shared.
    let mp = mapper
        .mapped_pages
        .get(&phys_addr)
        .expect("Aliased page should remain in mapped_pages");
    assert_eq!(mp.refs.load(SeqCst), 1, "Ref count should drop to 1");
    assert_eq!(
        mp.aliasing_paging_structures.len(),
        1,
        "Only the surviving alias should remain"
    );
    let (alias, _) = mp
        .aliasing_paging_structures
        .first()
        .expect("There should be exactly one aliasing paging structure");
    assert_eq!(
        *alias, other_pml4 as usize,
        "other_pml4 should be the surviving alias"
    );

    unsafe {
        let other_pte = (*other_pt).get_entry_mut(pt_idx);
        assert_eq!(
            other_pte.get_flags(ALL_FLAGS) & PRESENT_FLAG,
            PRESENT_FLAG,
            "Surviving alias PTE should still be present"
        );
        assert_eq!(
            other_pte.get_flags(ALL_FLAGS) & RW_FLAG,
            RW_FLAG,
            "Surviving alias PTE should be restored to RW"
        );
    }

    unsafe { alloc::alloc::dealloc(base, layout) };
}

#[test]
fn test_unmap_misaligned() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    // `next = 1` reserves index 0 for the source PML4 (located at `base`).
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let phys_addr = 0x0000_b100usize;
    let virt_addr = 0x0000_0000_dead_b100usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct;

    let pdpt = mapper.new_table();
    let pd = mapper.new_table();
    let pt = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr((*pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*pt).get_entry_mut(pt_idx);
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
    }

    let mut aliases = BTreeSet::new();
    aliases.insert((pml4 as usize, virt_addr));
    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Normal,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures: aliases,
        },
    );

    let result = mapper.unmap(virt_addr);

    assert!(result.is_err(), "Unmapping should fail");
    assert_eq!(
        result.unwrap_err(),
        PagingError::MisalignedAddress(virt_addr, PageSize::Normal),
        "Expected misaligned address error"
    );

    unsafe {
        alloc::alloc::dealloc(base, layout);
    }
}

#[test]
fn test_unmap_huge_misaligned() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    // `next = 1` reserves index 0 for the source PML4 (located at `base`).
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let phys_addr = 0x0010_0000usize;
    let virt_addr = 0x0000_0000_de90_0000usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;

    let pml4 = base as *mut PagingStruct;

    let pdpt = mapper.new_table();
    let pd = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr(phys_addr & MASK_51_12);
        pde.set_flags(PRESENT_FLAG | RW_FLAG | PS_FLAG, true);
    }

    let mut aliases = BTreeSet::new();
    aliases.insert((pml4 as usize, virt_addr));
    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Huge,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures: aliases,
        },
    );

    let result = mapper.unmap(virt_addr);

    assert!(result.is_err(), "Unmapping should fail");
    assert_eq!(
        result.unwrap_err(),
        PagingError::MisalignedAddress(virt_addr, PageSize::Huge),
        "Expected misaligned address error"
    );

    unsafe {
        alloc::alloc::dealloc(base, layout);
    }
}

#[test]
fn test_unmap_gigantic_misaligned() {
    let allocator = PageAllocator::new();
    let layout = core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let fake_native = UserlandTest(base);
    // `next = 1` reserves index 0 for the source PML4 (located at `base`).
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let phys_addr = 0x4100_0000usize;
    let virt_addr = 0x0000_0000_4100_0000usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;

    let pml4 = base as *mut PagingStruct;

    let pdpt = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr(phys_addr);
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG | PS_FLAG, true);
    }

    let mut aliases = BTreeSet::new();
    aliases.insert((pml4 as usize, virt_addr));
    mapper.mapped_pages.insert(
        phys_addr,
        MappedPage {
            phys_addr,
            size: PageSize::Gigantic,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures: aliases,
        },
    );

    let result = mapper.unmap(virt_addr);

    assert!(result.is_err(), "Unmapping should fail");
    assert_eq!(
        result.unwrap_err(),
        PagingError::MisalignedAddress(virt_addr, PageSize::Gigantic),
        "Expected misaligned address error"
    );

    unsafe {
        alloc::alloc::dealloc(base, layout);
    }
}
