use super::*;

#[repr(C, align(0x1000))]
struct FakePage {
    bytes: [u8; 0x1000],
}

#[test]
fn test_cow_tmp_map() {
    let mut allocator = PageAllocator::new();
    let fake_page_layout = core::alloc::Layout::new::<FakePage>();
    let fake_page: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(fake_page_layout) };
    allocator.init(&[(fake_page as usize, 0x1000)]);

    let paging_struct_layout =
        core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(paging_struct_layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
    );

    let new_page = mapper.cow_tmp_map(core::ptr::null_mut());

    let virt_addr = core::ptr::null_mut::<u8>() as usize;
    let phys_addr = new_page.get_addr();
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
    }

    unsafe { alloc::alloc::dealloc(base, paging_struct_layout) }
    unsafe { alloc::alloc::dealloc(fake_page, fake_page_layout) }
}

#[test]
fn test_cow() {
    let mut allocator = PageAllocator::new();
    let fake_dest_page_layout = core::alloc::Layout::new::<FakePage>();
    let fake_dest_page: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(fake_dest_page_layout) };
    allocator.init(&[(fake_dest_page as usize, 0x1000)]);

    let fake_src_page_layout = core::alloc::Layout::new::<FakePage>();
    let fake_src_page: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(fake_src_page_layout) };

    let data = [0xdeu8, 0xadu8, 0xbeu8, 0xefu8];
    unsafe {
        let ptr = core::slice::from_raw_parts_mut(fake_src_page, data.len());
        ptr.copy_from_slice(&data);
    }

    let paging_struct_layout =
        core::alloc::Layout::new::<[PagingStruct; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(paging_struct_layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct,
        0x200000,
        1,
        allocator,
        fake_native,
    );

    let phys_addr = fake_src_page as usize;
    let virt_addr = fake_src_page as usize;

    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let src_pml4 = mapper.new_table();
    let src_pdpt = mapper.new_table();
    let src_pd = mapper.new_table();
    let src_pt = mapper.new_table();

    // This is what cr3 points to when cow() is called.
    // Set this to base, which is what UserlandTest returns.
    let dest_pml4 = base as *mut PagingStruct;
    let dest_pdpt = mapper.new_table();
    let dest_pd = mapper.new_table();
    let dest_pt = mapper.new_table();

    // Construct mapping for src_pml4
    unsafe {
        let pml4e = (*src_pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*src_pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*src_pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*src_pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*src_pd).get_entry_mut(pd_idx);
        pde.set_addr((*src_pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*src_pt).get_entry_mut(pt_idx);
        pte.set_addr(fake_src_page as usize);
        pte.set_flags(PRESENT_FLAG, true);
    }
    let mut aliasing_paging_structures = BTreeSet::new();
    aliasing_paging_structures.insert((src_pml4 as usize, fake_src_page as usize));
    mapper.mapped_pages.insert(
        fake_src_page as usize,
        MappedPage {
            phys_addr,
            size: PageSize::Normal,
            refs: AtomicUsize::new(1),
            aliasing_paging_structures,
        },
    );

    // Construct mapping for dest_pml4
    unsafe {
        let pml4e = (*dest_pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*dest_pdpt).phys_addr::<UserlandTest>());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*dest_pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*dest_pd).phys_addr::<UserlandTest>());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*dest_pd).get_entry_mut(pd_idx);
        pde.set_addr((*dest_pt).phys_addr::<UserlandTest>());
        pde.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pte = (*dest_pt).get_entry_mut(pt_idx);
        pte.set_addr(fake_src_page as usize);
        pte.set_flags(PRESENT_FLAG, true);
    }
    let mp = mapper
        .mapped_pages
        .get_mut(&(fake_src_page as usize))
        .expect("Mapped page should exist in mapped_pages");
    mp.refs.fetch_add(1, Ordering::Relaxed);
    mp.aliasing_paging_structures
        .insert((dest_pml4 as usize, fake_src_page as usize));

    mapper.cow(fake_src_page, fake_dest_page);

    // Check if mapping is correct
    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    unsafe {
        let pml4e = (*src_pml4).get_entry_mut(pml4_idx);
        let (_, pml4e_flags) = (pml4e.get_addr(), pml4e.get_flags(ALL_FLAGS));
        assert_eq!(
            pml4e_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PML4E should be present"
        );

        let pdpte = (*src_pdpt).get_entry_mut(pdpt_idx);
        let (_, pdpte_flags) = (pdpte.get_addr(), pdpte.get_flags(ALL_FLAGS));
        assert_eq!(
            pdpte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDPTE should be present"
        );

        let pde = (*src_pd).get_entry_mut(pd_idx);
        let (_, pde_flags) = (pde.get_addr(), pde.get_flags(ALL_FLAGS));
        assert_eq!(
            pde_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDE should be present"
        );

        let pte = (*src_pt).get_entry_mut(pt_idx);
        let (page_phys_addr, pte_flags) = (pte.get_addr(), pte.get_flags(ALL_FLAGS));
        assert_eq!(
            pte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PTE should be present"
        );
        assert_eq!(pte_flags & RW_FLAG, RW_FLAG, "PTE should be RW");
        assert_eq!(
            page_phys_addr,
            fake_src_page as usize & MASK_47_12,
            "Physical page address should match"
        );
    }

    unsafe {
        let pml4e = (*dest_pml4).get_entry_mut(pml4_idx);
        let (_, pml4e_flags) = (pml4e.get_addr(), pml4e.get_flags(ALL_FLAGS));
        assert_eq!(
            pml4e_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PML4E should be present"
        );

        let pdpte = (*dest_pdpt).get_entry_mut(pdpt_idx);
        let (_, pdpte_flags) = (pdpte.get_addr(), pdpte.get_flags(ALL_FLAGS));
        assert_eq!(
            pdpte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDPTE should be present"
        );

        let pde = (*dest_pd).get_entry_mut(pd_idx);
        let (_, pde_flags) = (pde.get_addr(), pde.get_flags(ALL_FLAGS));
        assert_eq!(
            pde_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PDE should be present"
        );

        let pte = (*dest_pt).get_entry_mut(pt_idx);
        let (page_phys_addr, pte_flags) = (pte.get_addr(), pte.get_flags(ALL_FLAGS));
        assert_eq!(
            pte_flags & PRESENT_FLAG,
            PRESENT_FLAG,
            "PTE should be present"
        );
        assert_eq!(pte_flags & RW_FLAG, RW_FLAG, "PTE should be RW");
        assert_eq!(
            page_phys_addr,
            fake_dest_page as usize & MASK_47_12,
            "Physical page address should match"
        );
    }

    let src_mp = mapper
        .mapped_pages
        .get(&(fake_src_page as usize))
        .expect("Mapped page should be in mapoed_pages");
    let got_refs = src_mp.refs.load(SeqCst);
    assert_eq!(got_refs, 1, "Reference count should be 1 after cow");
    assert_eq!(
        src_mp.aliasing_paging_structures.len(),
        1,
        "Should have one aliasing paging structure"
    );
    let (alias, _) = src_mp
        .aliasing_paging_structures
        .first()
        .expect("There should be exactly one aliasing paging structure");
    assert_eq!(
        *alias, src_pml4 as usize,
        "src_pml4 should be the only aliasing paging structure"
    );

    let dest_mp = mapper
        .mapped_pages
        .get(&(fake_dest_page as usize))
        .expect("Mapped page should be in mapoed_pages");
    let got_refs = dest_mp.refs.load(SeqCst);
    assert_eq!(got_refs, 1, "Reference count should be 1 after cow");
    assert_eq!(
        dest_mp.aliasing_paging_structures.len(),
        1,
        "There should be exactly one aliasing paging structure"
    );
    let (alias, _) = dest_mp
        .aliasing_paging_structures
        .first()
        .expect("There should be exactly one aliasing paging structure");
    assert_eq!(
        *alias, dest_pml4 as usize,
        "dest_pml4 should be the only aliasing paging structure"
    );

    // Check if data in src page was copied to dest page.
    unsafe {
        let ptr = core::slice::from_raw_parts_mut(fake_dest_page, data.len());
        assert_eq!(ptr, data);
    }

    unsafe { alloc::alloc::dealloc(base, paging_struct_layout) }
    unsafe { alloc::alloc::dealloc(fake_src_page, fake_src_page_layout) }
    unsafe { alloc::alloc::dealloc(fake_dest_page, fake_dest_page_layout) }
}
