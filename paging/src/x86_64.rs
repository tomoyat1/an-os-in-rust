use core::arch::asm;

pub const MASK_51_12: usize = 0x000ffffffffff000;
pub const MASK_51_30: usize = 0x000ffffffc0000000;
pub const MASK_51_21: usize = 0x0000fffffffe00000;
pub const MASK_47_12: usize = 0x0000fffffffff000;
pub const MASK_47_39: usize = 0x0000ff8000000000;
pub const MASK_47_30: usize = 0x0000fffffc0000000;
pub const MASK_38_30: usize = 0x0000007fc0000000;
pub const MASK_29_21: usize = 0x000000003fe00000;
pub const MASK_29_0: usize = 0x000000003fffffff;
pub const MASK_20_12: usize = 0x00000000001ff000;
pub const MASK_20_0: usize = 0x00000000001fffff;

pub const PAGING_STRUCTURE_BASE: usize = 0xffffff8000000000;

pub fn read_cr3() -> usize {
    let cr3: usize;
    unsafe {
        asm!(
            "mov {tmp}, cr3",
            tmp = out(reg) cr3
        );
    }
    cr3
}
