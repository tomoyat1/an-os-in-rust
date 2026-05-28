#![no_std]

pub trait Arch {
    fn paging_structure_base(&self) -> usize;

    fn flush_tlb(&self);
}
