use super::*;

/// Present; when 0, the entry is ignored
pub const PRESENT_FLAG: usize = 1;

/// Read/write; if 0, writes are not allowed to the region that the entry controls.
pub const RW_FLAG: usize = 1 << 1;

/// User/supervisor; if 0, user-mode accesses are not allowed to the region that the entry controls.
pub const US_FLAG: usize = 1 << 2;

/// Page-level write-through
pub const PWT_FLAG: usize = 1 << 3;

/// Page-level cache disable
pub const PCD_FLAG: usize = 1 << 4;

/// Accessed; when 1, the entry has been used for linear address translation.
pub const ACCESSED_FLAG: usize = 1 << 5;

/// Dirty; when 1, the region that the entry controls has been written to.
pub const DIRTY_FLAG: usize = 1 << 6;

/// Page size; when 1, the entry directly maps memory. When 0, the entry references a subordinate paging structure.
pub const PS_FLAG: usize = 1 << 7;

/// Global; determines whether the translation is global.
pub const GLOBAL_FLAG: usize = 1 << 8;

/// Page attribute table; determines the memory thpe used to access the region that the entry controls.
pub const PAT_FLAG: usize = 1 << 12;

/// A paging structure entry.
#[repr(C)]
pub struct PageEntry {
    bytes: usize,
}

impl PageEntry {
    pub const fn new(flags: usize, phys_addr: usize) -> Self {
        PageEntry {
            bytes: flags | (phys_addr & MASK_51_12),
        }
    }

    pub fn get_flags(&self, flag: usize) -> usize {
        self.bytes & flag
    }

    pub fn set_flags(&mut self, flag: usize, value: bool) {
        if value {
            self.bytes |= flag;
        } else {
            self.bytes &= !flag;
        }
    }

    pub fn get_addr(&self) -> usize {
        self.bytes & MASK_51_12
    }

    pub fn set_addr(&mut self, addr: usize) {
        self.bytes |= addr & MASK_51_12;
    }
}
