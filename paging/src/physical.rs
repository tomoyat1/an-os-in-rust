#[cfg(not(test))]
use alloc::collections::BTreeSet;
#[cfg(test)]
use std::collections::BTreeSet;

#[cfg(not(test))]
use alloc::vec::Vec;
#[cfg(test)]
use std::vec::Vec;

const PAGE_SIZE_ORDER: usize = 12;
const PAGE_SIZE: usize = 1 << PAGE_SIZE_ORDER;

// From 4KiB to 512KiB
const BLOCK_ORDER_COUNT: usize = 7;

pub struct PageAllocator {
    free_lists: Vec<BTreeSet<usize>>,
}

impl PageAllocator {
    pub const fn new() -> Self {
        let mut pa = PageAllocator {
            free_lists: Vec::new(),
        };
        pa
    }

    pub fn init(&mut self, init_free: &[(usize, usize)]) {
        for _order in 0..=BLOCK_ORDER_COUNT {
            self.free_lists.push(BTreeSet::new());
        }

        for b in init_free {
            let mut start = b.0;
            let mut size = b.1;
            loop {
                let mut shift = PAGE_SIZE;
                for sz in PAGE_SIZE_ORDER..=PAGE_SIZE_ORDER + BLOCK_ORDER_COUNT {
                    let block_size = 1 << sz;
                    if !is_buddy_in_range(start, block_size, start, size) {
                        self.free(Block {
                            addr: start,
                            order: sz,
                        });
                        break;
                    }
                    shift = block_size << 1;
                }

                start += shift;
                size -= shift;

                if size == 0 {
                    break;
                }
            }
        }
    }

    pub fn allocate(&mut self, order: usize) -> Option<Block> {
        let list_idx = order - PAGE_SIZE_ORDER;
        if list_idx <= self.free_lists.len() {
            return None;
        }

        if let Some(&addr) = self.free_lists[list_idx].iter().next() {
            self.free_lists[list_idx].remove(&addr);
            return Some(Block { addr, order });
        }

        // Split larger block
        let large_block = self.allocate(list_idx + 1)?;
        let block_size = 1 << order;
        let buddy = large_block.addr ^ block_size;
        self.free_lists[list_idx].insert(buddy);
        Some(Block {
            addr: large_block.addr,
            order,
        })
    }

    pub fn free(&mut self, block: Block) {
        let list_idx = block.order - PAGE_SIZE_ORDER;
        let block_size = 1 << block.order;
        let buddy = block.addr ^ block_size;

        if self.free_lists[list_idx].remove(&buddy) && list_idx + 1 < self.free_lists.len() {
            let merged = block.addr & !block_size;
            self.free(Block {
                addr: merged,
                order: block.order + 1,
            })
        } else {
            self.free_lists[list_idx].insert(block.addr);
        }
    }
}

fn is_buddy_in_range(addr: usize, block_size: usize, start: usize, size: usize) -> bool {
    let buddy = addr ^ block_size;
    if block_size == PAGE_SIZE {
        (start..start + size).contains(&buddy)
    } else {
        (start..start + size).contains(&buddy)
            && is_buddy_in_range(buddy, block_size >> 1, start, size)
    }
}

struct Block {
    addr: usize,
    order: usize,
}

impl Block {
    pub(crate) fn get_addr(&self) -> usize {
        self.addr
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_buddy_in_range() {
        assert_eq!(
            is_buddy_in_range(0x2000, 0x1000, 0x2000, 0x2000),
            true,
            "case 1"
        );
        assert_eq!(
            is_buddy_in_range(0x4000, 0x2000, 0x2000, 0x6000),
            true,
            "case 2"
        );
        assert_eq!(
            is_buddy_in_range(0x4000, 0x2000, 0x2000, 0x4000),
            false,
            "case 3"
        );
        assert_eq!(
            is_buddy_in_range(0x2000, 0x2000, 0x2000, 0x5000),
            false,
            "case 3"
        );
        assert_eq!(is_buddy_in_range(0x0, 0x2000, 0x0, 0x3000), false, "case 4");
    }

    #[test]
    fn init_works() {
        let mut pa = PageAllocator::new();
        // Range [0x2000, 0x13000].
        let init_free = [(0x2000, 0x11000)];
        pa.init(&init_free);

        assert_eq!(
            pa.free_lists[0].len(),
            1,
            "Case 1: Blocks with 0x1000 size should be 1"
        );
        assert_eq!(
            pa.free_lists[1].len(),
            2,
            "Case 1: Blocks with 0x2000 size should be 2"
        );
        assert_eq!(
            pa.free_lists[2].len(),
            1,
            "Case 1: Blocks with 0x4000 size should be 1"
        );
        assert_eq!(
            pa.free_lists[3].len(),
            1,
            "Case 1: Blocks with 0x4000 size should be 1"
        );

        let mut pa = PageAllocator::new();
        // Range [[0x2000, 0x4000], [0x6000, 0x2000]].
        let init_free = [(0x2000, 0x4000), (0x6000, 0x2000)];
        pa.init(&init_free);

        assert_eq!(
            pa.free_lists[0].len(),
            0,
            "Case 2: Blocks with 0x1000 size should be 0"
        );
        assert_eq!(
            pa.free_lists[1].len(),
            1,
            "Case 2: Blocks with 0x2000 size should be 1"
        );
        assert_eq!(
            pa.free_lists[2].len(),
            1,
            "Case 2: Blocks with 0x4000 size should be 1"
        );

        let mut pa = PageAllocator::new();
        // Range [[0x2000, 0x1000], [0x3000, 0x3000]].
        let init_free = [(0x2000, 0x1000), (0x3000, 0x3000)];
        pa.init(&init_free);

        assert_eq!(
            pa.free_lists[0].len(),
            0,
            "Case 3: Blocks with 0x1000 size should be 0"
        );
        assert_eq!(
            pa.free_lists[1].len(),
            2,
            "Case 3: Blocks with 0x2000 size should be 1"
        );

        let mut pa = PageAllocator::new();
        // Range [[0x0, 0x1000], [0x1000, 0x3000], [0x4000, 0x4000]].
        let init_free = [(0x0, 0x1000), (0x1000, 0x3000), (0x4000, 0x4000)];
        pa.init(&init_free);

        assert_eq!(
            pa.free_lists[0].len(),
            0,
            "Case 4: Blocks with 0x1000 size should be 0"
        );
        assert_eq!(
            pa.free_lists[1].len(),
            0,
            "Case 4: Blocks with 0x2000 size should be 0"
        );
        assert_eq!(
            pa.free_lists[2].len(),
            0,
            "Case 4: Blocks with 0x4000 size should be 0"
        );
        assert_eq!(
            pa.free_lists[3].len(),
            1,
            "Case 4: Blocks with 0x8000 size should be 1"
        );
    }
}
