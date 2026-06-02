use super::*;
use paging_common::physical::PageAllocator;

#[test]
fn test_phys_addr() {
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

    let got_phys_addr = mapper.phys_addr(virt_addr).expect("Mapping should exist");
    assert_eq!(
        got_phys_addr, phys_addr,
        "Physical address should match: {got_phys_addr:#x} != {phys_addr:#x}"
    );
    unsafe { alloc::alloc::dealloc(base, layout) };
}

#[test]
fn test_phys_addr_huge_page() {
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

    // 2 MiB aligned physical address
    let phys_addr = 0xdeadbeefusize;
    let virt_addr = phys_addr + PAGING_STRUCTURE_BASE;
    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;
    let pd_idx = (virt_addr & MASK_29_21) >> 21;

    let pml4 = base as *mut PagingStruct<UserlandTest>;
    let pdpt = mapper.new_table();
    let pd = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr((*pd).phys_addr());
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pde = (*pd).get_entry_mut(pd_idx);
        pde.set_addr(phys_addr & MASK_51_21); // 2MiB aligned
        pde.set_flags(PRESENT_FLAG | RW_FLAG | PS_FLAG, true);
    }

    let got_phys_addr = mapper.phys_addr(virt_addr).expect("Mapping should exist");
    assert_eq!(
        got_phys_addr, phys_addr,
        "Physical address should match: {got_phys_addr:#x} != {phys_addr:#x}"
    );
    unsafe { alloc::alloc::dealloc(base, layout) };
}

#[test]
fn test_phys_addr_gigantic_page() {
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

    // 1 GiB aligned physical address
    let phys_addr = 0xceadbeefusize;
    let virt_addr = phys_addr + PAGING_STRUCTURE_BASE;
    let pml4_idx = (virt_addr & MASK_47_39) >> 39;
    let pdpt_idx = (virt_addr & MASK_38_30) >> 30;

    let pml4 = base as *mut PagingStruct<UserlandTest>;
    let pdpt = mapper.new_table();
    let pd = mapper.new_table();

    unsafe {
        let pml4e = (*pml4).get_entry_mut(pml4_idx);
        pml4e.set_addr((*pdpt).phys_addr());
        pml4e.set_flags(PRESENT_FLAG | RW_FLAG, true);

        let pdpte = (*pdpt).get_entry_mut(pdpt_idx);
        pdpte.set_addr(phys_addr & MASK_51_30);
        pdpte.set_flags(PRESENT_FLAG | RW_FLAG | PS_FLAG, true);
    }

    let got_phys_addr = mapper.phys_addr(virt_addr).expect("Mapping should exist");
    assert_eq!(
        got_phys_addr, phys_addr,
        "Physical address should match: {got_phys_addr:#x} != {phys_addr:#x}"
    );
    unsafe { alloc::alloc::dealloc(base, layout) };
}
