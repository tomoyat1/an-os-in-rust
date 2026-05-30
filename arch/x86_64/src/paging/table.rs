use super::*;
use core::marker::PhantomData;
use interface::Environment;

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

/// Page attribute table; determines the memory type used to access the region that the entry controls.
pub const PAT_FLAG: usize = 1 << 12;

/// A paging structure entry.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PagingStructEntry<E: Environment + Clone> {
    bytes: usize,
    _phantom: PhantomData<E>,
}

impl<E: Environment + Clone> PagingStructEntry<E> {
    pub const fn new(flags: usize, phys_addr: usize) -> Self {
        PagingStructEntry::<E> {
            bytes: flags | (phys_addr & MASK_51_12),
            _phantom: PhantomData::<E>,
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

    /// Returns the physical address stored in the entry.
    pub fn get_addr(&self) -> usize {
        self.bytes & MASK_51_12
    }

    /// Returns the virtual address of the entry, at offset PAGING_STRUCTURE_BASE.
    pub fn get_virt_addr(&self) -> usize {
        self.get_addr() + E::PAGING_STRUCTURE_BASE
    }

    pub fn set_addr(&mut self, addr: usize) {
        self.bytes = (self.bytes & !MASK_51_12) | (addr & MASK_51_12);
    }
}

#[repr(C, align(0x1000))]
pub struct PagingStruct<E: Environment + Clone> {
    entries: [PagingStructEntry<E>; 512],
}

impl<E: Environment + Clone + Copy + Default> Default for PagingStruct<E> {
    fn default() -> Self {
        Self {
            entries: [PagingStructEntry::<E>::default(); 512],
        }
    }
}

impl<'a, E: Environment + Clone> PagingStruct<E> {
    pub fn get_entry(&'a self, idx: usize) -> &'a PagingStructEntry<E> {
        &self.entries[idx]
    }

    pub fn get_entry_mut(&'a mut self, idx: usize) -> &'a mut PagingStructEntry<E> {
        &mut self.entries[idx]
    }

    pub fn phys_addr(&self) -> usize {
        (self as *const PagingStruct<E>).expose_provenance() - E::PAGING_STRUCTURE_BASE
    }
}
