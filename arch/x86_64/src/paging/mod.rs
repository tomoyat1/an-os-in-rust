extern crate interface;

pub mod mapping;
pub mod table;
pub const MASK_51_12: usize = 0x000f_ffff_ffff_f000;
pub const MASK_51_30: usize = 0x000f_ffff_ffc0_0000_00;
pub const MASK_51_21: usize = 0x0000_ffff_fffe_0000_0;
pub const MASK_47_12: usize = 0x0000_ffff_ffff_f000;
pub const MASK_47_39: usize = 0x0000_ff80_0000_0000;
pub const MASK_47_30: usize = 0x0000_ffff_fc00_0000_0;
pub const MASK_38_30: usize = 0x0000_007f_c000_0000;
pub const MASK_29_21: usize = 0x0000_0000_3fe0_0000;
pub const MASK_29_0: usize = 0x0000_0000_3fff_ffff;
pub const MASK_20_12: usize = 0x0000_0000_001f_f000;
pub const MASK_20_0: usize = 0x0000_0000_001f_ffff;
pub const PAGING_STRUCTURE_BASE: usize = 0xffff_ff80_0000_0000;
