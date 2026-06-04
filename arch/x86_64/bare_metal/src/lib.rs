#![no_std]
extern crate interface;
extern crate x86_64;

use core::arch::asm;
use interface::Environment;

#[derive(Copy, Clone, Default)]
pub struct X86_64BareMetal();
impl Environment for X86_64BareMetal {
    const PAGING_STRUCTURE_BASE: usize = x86_64::paging::PAGING_STRUCTURE_BASE;

    fn paging_structure_base(&self) -> *mut u8 {
        let cr3: usize;
        unsafe {
            asm!(
            "mov {tmp}, cr3",
            tmp = out(reg) cr3
            );
        }
        (cr3 + Self::PAGING_STRUCTURE_BASE) as *mut u8
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
