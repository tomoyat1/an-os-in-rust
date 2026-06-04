use super::*;
use crate::paging::table::{
    PagingStruct, PagingStructEntry, ALL_FLAGS, PRESENT_FLAG, PS_FLAG, RW_FLAG,
};
use alloc::collections::BTreeMap;

use core::ptr::write_bytes;
use core::sync::atomic::Ordering::SeqCst;
use core::sync::atomic::{AtomicUsize, Ordering};
use interface::Environment;
use paging_common::physical::PageAllocator;

#[cfg(test)]
#[path = "./mapping/test/mod.rs"]
mod test;

const BOOT_PAGE_TABLE_COUNT: usize = 7;

trait PagingLevel {
    const MASK: usize;
    const SHIFT: usize;

    fn entry_idx(virt_addr: usize) -> usize {
        (virt_addr & Self::MASK) >> Self::SHIFT
    }
}

struct PML4;
impl PagingLevel for PML4 {
    const MASK: usize = MASK_47_39;
    const SHIFT: usize = 39;
}

struct PDPT;
impl PagingLevel for PDPT {
    const MASK: usize = MASK_38_30;
    const SHIFT: usize = 30;
}

struct PD;
impl PagingLevel for PD {
    const MASK: usize = MASK_29_21;
    const SHIFT: usize = 21;
}

struct PT;
impl PagingLevel for PT {
    const MASK: usize = MASK_20_12;
    const SHIFT: usize = 12;
}

pub struct PagingStructBase<E>(*mut PagingStruct<E>)
where
    E: Environment + Clone + Default;

unsafe impl<E> Send for PagingStructBase<E> where E: Environment + Clone + Default {}
unsafe impl<E> Sync for PagingStructBase<E> where E: Environment + Clone + Default {}

impl<E> From<&PagingStructBase<E>> for usize
where
    E: Environment + Clone + Default,
{
    fn from(base: &PagingStructBase<E>) -> usize {
        base.0 as usize
    }
}

struct MappedPage {
    phys_addr: usize,
    size: PageSize,
    refs: AtomicUsize,
}

// TODO: make this a trait if we support architectures other than x86_64.
pub struct Mapper<E>
where
    E: Environment + Clone + Default,
{
    base: PagingStructBase<E>,
    length: usize,
    // TODO: Bump style allocation will break with offset mapping.
    //       Use better allocation method
    next: usize,

    // Contains representations of mapped physical pages, _only for userland half_.
    mapped_pages: BTreeMap<usize, MappedPage>,

    page_allocator: PageAllocator,
    environment: E,
}

impl<E> Mapper<E>
where
    E: Environment + Clone + Default,
{
    pub fn new(
        base: *mut PagingStruct<E>,
        length: usize,
        next: usize,
        page_allocator: PageAllocator,
        environment: E,
    ) -> Self {
        let ptr = unsafe { base.add(7) } as *mut u8;
        unsafe {
            write_bytes(ptr, 0u8, length - BOOT_PAGE_TABLE_COUNT * 0x1000);
        }
        Mapper {
            base: PagingStructBase(base),
            length,
            next,
            mapped_pages: BTreeMap::new(),
            page_allocator,
            environment,
        }
    }

    fn map(&mut self, phys_addr: usize, virt_addr: usize) {
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct<E>;
        let (pdpt, _) = self.walk_or_create_table::<PML4>(pml4, virt_addr);
        let (pd, _) = self.walk_or_create_table::<PDPT>(pdpt, virt_addr);
        let (pt, _) = self.walk_or_create_table::<PD>(pd, virt_addr);

        let idx = (virt_addr & PT::MASK) >> PT::SHIFT;
        unsafe {
            let pte = (*pt).get_entry_mut(idx);
            pte.set_addr(phys_addr & MASK_51_12);
            pte.set_flags(PRESENT_FLAG | RW_FLAG, true);
        }
        if (virt_addr >> 51) == 0 {
            self.mapped_pages.insert(
                phys_addr,
                MappedPage {
                    phys_addr,
                    size: PageSize::Normal,
                    refs: AtomicUsize::new(1),
                },
            );
        }
        self.environment.flush_tlb();
    }

    fn unmap(&mut self, virt_addr: usize) -> Option<()> {
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct<E>;
        let (pml4e_addr, _) = self.walk::<PML4>(pml4, virt_addr)?;
        let pdpt = self.table_for_phys_addr(pml4e_addr);
        let (pdpte_addr, pdpte_flags) = self.walk::<PDPT>(pdpt, virt_addr)?;
        if pdpte_flags & PS_FLAG == PS_FLAG {
            const MASK: usize = (1 << 30) - 1;
            assert_eq!(
                virt_addr & MASK,
                0,
                "Virtual address must be 1 GiB multiple when unmapping gigantic page."
            );
            unsafe {
                let idx = PDPT::entry_idx(virt_addr);
                let entry = (*pdpt).get_entry_mut(idx);
                *entry = PagingStructEntry::<E>::default();
            }

            let phys_page = self
                .mapped_pages
                .get_mut(&pdpte_addr)
                .expect("Mapped pages should be in mapped_pages")
                as *mut MappedPage;
            // SAFETY: We either dereference the pointer and decrement the ref count or we remove
            //         the MappedPage, but not both.
            unsafe {
                if (*phys_page).refs.load(SeqCst) > 1 {
                    (*phys_page).refs.fetch_sub(1, SeqCst);
                } else {
                    self.mapped_pages.remove(&pdpte_addr);
                }
            }

            return Some(());
        }
        let pd = self.table_for_phys_addr(pdpte_addr);
        let (pde_addr, pde_flags) = self.walk::<PD>(pd, virt_addr)?;
        if pde_flags & PS_FLAG == PS_FLAG {
            const MASK: usize = (1 << 21) - 1;
            assert_eq!(
                virt_addr & MASK,
                0,
                "Virtual address must be 2 MiB multiple when unmapping huge page."
            );
            unsafe {
                let idx = PD::entry_idx(virt_addr);
                let entry = (*pd).get_entry_mut(idx);
                *entry = PagingStructEntry::<E>::default();
            }

            let phys_page = self
                .mapped_pages
                .get_mut(&pde_addr)
                .expect("Mapped pages should be in mapped_pages")
                as *mut MappedPage;
            // SAFETY: We either dereference the pointer and decrement the ref count or we remove
            //         the MappedPage, but not both.
            unsafe {
                if (*phys_page).refs.load(SeqCst) > 1 {
                    (*phys_page).refs.fetch_sub(1, SeqCst);
                } else {
                    self.mapped_pages.remove(&pde_addr);
                }
            }

            return Some(());
        }
        let pt = self.table_for_phys_addr(pde_addr);
        let (pte_addr, _) = self.walk::<PT>(pt, virt_addr)?;
        unsafe {
            let idx = PT::entry_idx(virt_addr);
            let entry = (*pt).get_entry_mut(idx);
            *entry = PagingStructEntry::<E>::default();
        }

        let phys_page =
            self.mapped_pages
                .get_mut(&pte_addr)
                .expect("Mapped pages should be in mapped_pages") as *mut MappedPage;
        // SAFETY: We either dereference the pointer and decrement the ref count or we remove
        //         the MappedPage, but not both.
        unsafe {
            if (*phys_page).refs.load(SeqCst) > 1 {
                (*phys_page).refs.fetch_sub(1, SeqCst);
            } else {
                let _ = self
                    .mapped_pages
                    .remove(&pte_addr)
                    .expect("Removed MappedPage must have been in mapped_pages");
            }
        }

        Some(())

        // TODO: Garbage collection of no longer used paging structures.
        //       This only has meaning once we have memory reclamation in Mapper.
    }

    pub fn alloc_page_at(&mut self, virt_addr: usize) {
        let phys_addr = self.page_allocator.allocate(12);
        match phys_addr {
            Some(phys_addr) => self.map(phys_addr.get_addr(), virt_addr),
            None => {
                panic!("No available physical pages!")
            }
        }
    }

    pub fn map_mmio(&mut self, phys_addr: usize) {
        let virt_addr = phys_addr + MMIO_BASE;
        self.map(phys_addr, virt_addr);
    }

    pub fn phys_addr(&self, virt_addr: usize) -> Option<usize> {
        let pml4 = unsafe {
            self.environment
                .paging_structure_base()
                .add(E::PAGING_STRUCTURE_BASE)
        } as *const PagingStruct<E>;

        let (pml4e_addr, _) = self.walk::<PML4>(pml4, virt_addr)?;
        let pdpt = self.table_for_phys_addr(pml4e_addr);
        let (pdpte_addr, pdpte_flags) = self.walk::<PDPT>(pdpt, virt_addr)?;
        if pdpte_flags & PS_FLAG == PS_FLAG {
            return Some(pdpte_addr | (virt_addr & ((1 << PDPT::SHIFT) - 1)));
        }
        let pd = self.table_for_phys_addr(pdpte_addr);

        let (pde_addr, pde_flags) = self.walk::<PD>(pd, virt_addr)?;
        if pde_flags & PS_FLAG == PS_FLAG {
            return Some(pde_addr | (virt_addr & ((1 << PD::SHIFT) - 1)));
        }
        let pt = self.table_for_phys_addr(pde_addr);

        let (page, _) = self.walk::<PT>(pt, virt_addr)?;
        Some(page | (virt_addr & ((1 << PT::SHIFT) - 1)))
    }

    pub fn fork(&mut self, paging_struct_base: *mut PagingStruct<E>) -> usize {
        let src_pml4 = paging_struct_base;
        let dst_pml4 = self.new_table();

        // Shallow copy kernel address space
        for i in 256..512usize {
            let (entry_addr, flags) = unsafe {
                let entry = (*src_pml4).get_entry(i);
                (entry.get_addr(), entry.get_flags(ALL_FLAGS))
            };
            if flags & PRESENT_FLAG != PRESENT_FLAG {
                continue;
            }
            unsafe {
                let dst_entry = (*dst_pml4).get_entry_mut(i);
                dst_entry.set_addr(entry_addr);
                dst_entry.set_flags(flags, true);
            }
        }
        for i in 0..256usize {
            let (entry_addr, flags) = unsafe {
                let entry = (*src_pml4).get_entry(i);
                (entry.get_addr(), entry.get_flags(ALL_FLAGS))
            };
            if flags & PRESENT_FLAG != PRESENT_FLAG {
                continue;
            }
            let dst_pdpt = self.new_table();
            unsafe {
                (*dst_pml4).get_entry_mut(i).set_flags(flags, true);
                let dst_table_addr = (*dst_pdpt).phys_addr();
                (*dst_pml4).get_entry_mut(i).set_addr(dst_table_addr)
            }

            let src_pdpt = unsafe {
                let entry = (*src_pml4).get_entry(i);
                let phys_addr = entry.get_addr();
                self.table_for_phys_addr(phys_addr)
            };
            self.recursively_clone(src_pdpt, dst_pdpt, 3, i << 39);
        }

        unsafe { (*dst_pml4).phys_addr() }
    }

    fn recursively_clone(
        &mut self,
        src: *mut PagingStruct<E>,
        dst: *mut PagingStruct<E>,
        level: usize, // 3: pdpt, 2: pd, 1: pt
        virt_addr: usize,
    ) {
        for i in 0..512usize {
            let (src_addr, src_flags) = unsafe {
                let src_entry = (*src).get_entry(i);
                (src_entry.get_addr(), src_entry.get_flags(ALL_FLAGS))
            };
            let shift = match level {
                3 => 30usize,
                2 => 21usize,
                1 => 12usize,
                _ => unreachable!(),
            };
            let virt_addr = virt_addr | (i << shift);
            if level == 1 || src_flags & PS_FLAG == PS_FLAG {
                // src is a pt, or a pdpt / pd with PS == 1
                if src_flags & PRESENT_FLAG != PRESENT_FLAG {
                    continue;
                }
                unsafe {
                    let dst_entry = (*dst).get_entry_mut(i);
                    dst_entry.set_addr(src_addr);
                    dst_entry.set_flags(src_flags, true);
                    dst_entry.set_flags(RW_FLAG, false)
                }
                unsafe {
                    let src_entry = (*src).get_entry_mut(i);
                    src_entry.set_flags(RW_FLAG, false);
                }
                let mp = self.mapped_pages.get_mut(&src_addr).expect(
                    "MappedPage for physical page should have been created when getting mapped",
                );
                mp.refs.fetch_add(1, Ordering::Release);
            } else {
                // src_entry is a pdpte or pde with PS == 0
                if src_flags & PRESENT_FLAG != PRESENT_FLAG {
                    continue;
                }
                let dst_table = self.new_table();
                unsafe {
                    let dst_entry = (*dst).get_entry_mut(i);
                    let dst_table_addr = (*dst_table).phys_addr();
                    dst_entry.set_addr(dst_table_addr);
                    dst_entry.set_flags(src_flags, true);
                    dst_entry.set_flags(RW_FLAG, false)
                }

                let src_table = self.table_for_phys_addr(src_addr);
                self.recursively_clone(src_table, dst_table, level - 1, virt_addr);
            }
        }
    }

    fn new_table(&mut self) -> *mut PagingStruct<E> {
        let new_table = unsafe { self.base.0.add(self.next) };
        self.next += 1;
        new_table
    }

    fn table_for_phys_addr(&self, phys_addr: usize) -> *mut PagingStruct<E> {
        unsafe {
            let idx = (E::PAGING_STRUCTURE_BASE + phys_addr - usize::from(&self.base))
                / size_of::<PagingStruct<E>>();
            self.base.0.add(idx)
        }
    }

    /// Walk the paging structure of type `L` to get the entry of `virt_addr`.
    /// If the virtual address is not mapped, returns `None`.
    /// Returns the **physical address** of the resolved paging structure.
    fn walk<L: PagingLevel>(
        &self,
        table: *const PagingStruct<E>,
        virt_addr: usize,
    ) -> Option<(usize, usize)> {
        let idx = L::entry_idx(virt_addr);
        let (addr, flags) = unsafe {
            let entry = (*table).get_entry(idx);
            (entry.get_addr(), entry.get_flags(ALL_FLAGS))
        };
        match flags & PRESENT_FLAG {
            PRESENT_FLAG => Some((addr, flags)),
            _ => None,
        }
    }

    /// Walk the paging structure of type `L` to get the entry of `virt_addr`, or
    /// create a new paging structure if it doesn't exist.
    /// Returns the **physical address** of the resolved or newly created paging structure.
    fn walk_or_create_table<L: PagingLevel>(
        &mut self,
        table: *mut PagingStruct<E>,
        virt_addr: usize,
    ) -> (*mut PagingStruct<E>, usize) {
        match self.walk::<L>(table, virt_addr) {
            Some((next_table, flags)) => {
                let next_table = self.table_for_phys_addr(next_table);
                (next_table, flags)
            }
            None => {
                // Table does not exist, create new table.
                let idx = L::entry_idx(virt_addr);
                let next_table = self.new_table();
                let flags = PRESENT_FLAG | RW_FLAG;
                unsafe {
                    let entry = (*table).get_entry_mut(idx);
                    entry.set_addr((*next_table).phys_addr());
                    entry.set_flags(flags, true);
                }
                (next_table, flags)
            }
        }
    }
}
