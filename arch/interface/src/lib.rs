#![no_std]

pub trait Environment {
    const PAGING_STRUCTURE_BASE: usize;

    /// Returns the virtual address of the base paging structure.
    fn paging_structure_base(&self) -> *mut u8;

    fn flush_tlb(&self);
}
