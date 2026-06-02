use super::*;
use paging_common::physical::PageAllocator;

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
