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
        core::alloc::Layout::new::<[PagingStruct<UserlandTest>; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(paging_struct_layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct<UserlandTest>,
        0x200000,
        1,
        allocator,
        fake_native,
        core::ptr::null_mut(),
    );

    let new_page = mapper.cow_tmp_map();

    let virt_addr = mapper.cow_dest.0 as usize;
    let phys_addr = new_page.get_addr();
    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct<UserlandTest>;
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
        core::alloc::Layout::new::<[PagingStruct<UserlandTest>; PAGING_STRUCTURE_REGION_LEN]>();
    let base: *mut u8 = unsafe { alloc::alloc::alloc_zeroed(paging_struct_layout) };
    let fake_native = UserlandTest(base);
    let mut mapper = Mapper::new(
        base as *mut PagingStruct<UserlandTest>,
        0x200000,
        1,
        allocator,
        fake_native,
        fake_dest_page,
    );

    // Map twice to get a reference count of 2.
    mapper.map(fake_src_page as usize, fake_src_page as usize);
    mapper.map(fake_src_page as usize, fake_src_page as usize);

    mapper.cow(fake_src_page);

    // Check if mapping is correct
    let virt_addr = fake_src_page as usize;
    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;
    let pt_idx = (virt_addr & MASK_20_12) >> 12;

    let pml4 = base as *mut PagingStruct<UserlandTest>;
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
            fake_dest_page as usize & MASK_47_12,
            "Physical page address should match"
        );
    }

    let got_refs = mapper
        .mapped_pages
        .get(&(fake_src_page as usize))
        .expect("Mapped page should be in mapoed_pages")
        .refs
        .load(SeqCst);
    assert_eq!(got_refs, 1, "Reference count should be 1 after cow");

    // Check if data in src page was copied to dest page.
    unsafe {
        let ptr = core::slice::from_raw_parts_mut(fake_dest_page, data.len());
        assert_eq!(ptr, data);
    }

    unsafe { alloc::alloc::dealloc(base, paging_struct_layout) }
    unsafe { alloc::alloc::dealloc(fake_src_page, fake_src_page_layout) }
    unsafe { alloc::alloc::dealloc(fake_dest_page, fake_dest_page_layout) }
}
