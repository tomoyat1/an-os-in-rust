#![no_std]
use core::arch::asm;
extern crate interface;
use interface::Arch;

#[derive(Copy, Clone)]
pub struct X86_64();
impl Arch for X86_64 {
    fn paging_structure_base(&self) -> usize {
        let cr3: usize;
        unsafe {
            asm!(
            "mov {tmp}, cr3",
            tmp = out(reg) cr3
            );
        }
        cr3
    }

    /// flush_tlb() flushes the TLB.
    fn flush_tlb(&self) {
        unsafe {
            asm!(
            "mov {tmp}, cr3",
            "mov cr3, {tmp}",
            tmp = out(reg) _,
            )
        }
    }
}
