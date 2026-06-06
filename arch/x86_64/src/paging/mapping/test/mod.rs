mod cow;
mod fork;
mod map;
mod new_table;
mod phys_addr;
mod unmap;

use super::*;
use interface::Environment;

#[derive(Copy, Clone, Default)]
struct UserlandTest(*mut u8);

impl Environment for UserlandTest {
    const PAGING_STRUCTURE_BASE: usize = 0;
    fn paging_structure_base(&self) -> *mut u8 {
        self.0
    }
    fn flush_tlb(&self) {}
}

const PAGING_STRUCTURE_REGION_LEN: usize = 0x200000 / size_of::<PagingStruct>();
