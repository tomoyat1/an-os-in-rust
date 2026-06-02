use super::*;
use paging_common::physical::PageAllocator;

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

    let got_phys_addr = mapper.phys_addr(virt_addr).expect("Mapping should exist");
    assert_eq!(
        got_phys_addr, phys_addr,
        "Physical address should match: {got_phys_addr:#x} != {phys_addr:#x}"
    );
    unsafe { alloc::alloc::dealloc(base, layout) };
}
