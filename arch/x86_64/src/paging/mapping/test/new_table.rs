use super::*;
use paging_common::physical::PageAllocator;

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
