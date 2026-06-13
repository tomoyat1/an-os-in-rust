use super::*;
use crate::paging::error::PagingError;
use crate::paging::table::{
    PagingLevel, PagingStruct, PagingStructEntry, ALL_FLAGS, PD, PDPT, PML4, PRESENT_FLAG, PS_FLAG,
    PT, RW_FLAG,
};
use alloc::collections::{BTreeMap, BTreeSet};
use core::ptr;
use core::ptr::write_bytes;
use core::sync::atomic::Ordering::SeqCst;
use core::sync::atomic::{AtomicUsize, Ordering};
use interface::Environment;
use paging_common::physical::{Block, PageAllocator};
use util::pointer::SyncMutPointer;

#[cfg(test)]
#[path = "./mapping/test/mod.rs"]
mod test;

const BOOT_PAGE_TABLE_COUNT: usize = 7;

struct MappedPage {
    phys_addr: usize,
    size: PageSize,
    refs: AtomicUsize,
    aliasing_paging_structures: BTreeSet<(usize, usize)>,
}

struct LeafEntry {
    table: *mut PagingStruct,
    phys_addr: usize,
    idx: usize,
    page_size: PageSize,
}

impl<'a> From<&LeafEntry> for &'a mut PagingStructEntry {
    fn from(leaf: &LeafEntry) -> &'a mut PagingStructEntry {
        unsafe { (*leaf.table).get_entry_mut(leaf.idx) }
    }
}

// TODO: make this a trait if we support architectures other than x86_64.
pub struct Mapper<E>
where
    E: Environment,
{
    base: SyncMutPointer<PagingStruct>,
    length: usize,
    // TODO: Bump style allocation will break with offset mapping.
    //       Use better allocation method
    next: usize,

    // Contains representations of mapped physical pages, _only for userland half_.
    mapped_pages: BTreeMap<usize, MappedPage>,

    page_allocator: PageAllocator,
    environment: E,

    // Address to temporarily map pages to when copying data from aliased RO page to fresh page.
    cow_dest: SyncMutPointer<u8>,
}

impl<E: Environment> Mapper<E> {
    pub fn new(
        base: *mut PagingStruct,
        length: usize,
        next: usize,
        page_allocator: PageAllocator,
        environment: E,
        cow_dest: *mut u8,
    ) -> Self {
        let ptr = unsafe { base.add(7) } as *mut u8;
        unsafe {
            write_bytes(ptr, 0u8, length - BOOT_PAGE_TABLE_COUNT * 0x1000);
        }
        Mapper {
            base: base.into(),
            length,
            next,
            mapped_pages: BTreeMap::new(),
            page_allocator,
            environment,
            cow_dest: cow_dest.into(),
        }
    }

    fn map(&mut self, phys_addr: usize, virt_addr: usize) -> Result<(), PagingError> {
        // TODO: support mapping huge pages
        const MASK_11_0: usize = (1 << 12) - 1;
        if phys_addr & MASK_11_0 != 0 {
            return Err(PagingError::MisalignedAddress(phys_addr, PageSize::Normal));
        }
        if virt_addr & MASK_11_0 != 0 {
            return Err(PagingError::MisalignedAddress(virt_addr, PageSize::Normal));
        }
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct;
        let (pdpt, _) = self.walk_or_create_table::<PML4>(pml4, virt_addr);
        let (pd, _) = self.walk_or_create_table::<PDPT>(pdpt, virt_addr);
        let (pt, _) = self.walk_or_create_table::<PD>(pd, virt_addr);

        let idx = (virt_addr & PT::MASK) >> PT::SHIFT;
        let pte = unsafe { (*pt).get_entry_mut(idx) };
        pte.set_addr(phys_addr & MASK_51_12);
        pte.set_flags(PRESENT_FLAG | RW_FLAG, true);

        if (virt_addr >> 51) == 0 {
            let phys_page = self.mapped_pages.get_mut(&pte.get_addr());
            match phys_page {
                Some(phys_page) => {
                    phys_page.refs.fetch_add(1, SeqCst);
                    unsafe {
                        phys_page
                            .aliasing_paging_structures
                            .insert(((*pml4).phys_addr::<E>(), virt_addr));
                    }
                    // SAFETY: table_for_phys_addr and walk_to_leaf are pure reads of
                    //         self.base and never touch self.mapped_pages, so the data behind
                    //         this pointer remains valid and unmodified through the loop.
                    let set_ptr =
                        &phys_page.aliasing_paging_structures as *const BTreeSet<(usize, usize)>;
                    unsafe {
                        for (aliasing, _) in (*set_ptr).iter() {
                            let aliasing_pml4 = self.table_for_phys_addr(*aliasing);
                            let leaf = self
                                .walk_to_leaf(aliasing_pml4, virt_addr)
                                .expect("Aliasing paging structures must map address");
                            let entry: &mut PagingStructEntry = (&leaf).into();
                            entry.set_flags(RW_FLAG, false)
                        }
                    }
                }
                None => {
                    let mut aliasing_paging_structures = BTreeSet::new();
                    unsafe {
                        aliasing_paging_structures.insert(((*pml4).phys_addr::<E>(), virt_addr));
                    }
                    self.mapped_pages.insert(
                        phys_addr,
                        MappedPage {
                            phys_addr,
                            size: PageSize::Normal,
                            refs: AtomicUsize::new(1),
                            aliasing_paging_structures,
                        },
                    );
                }
            }
        }
        self.environment.flush_tlb();
        Ok(())
    }

    fn unmap(&mut self, virt_addr: usize) -> Result<(), PagingError> {
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct;
        let leaf = self
            .walk_to_leaf(pml4, virt_addr)
            // TODO: Make this function return a Result
            .expect("Page to be unmapped must be mapped");
        match leaf.page_size {
            PageSize::Gigantic => {
                const MASK: usize = (1 << 30) - 1;
                if virt_addr & MASK != 0 {
                    return Err(PagingError::MisalignedAddress(
                        virt_addr,
                        PageSize::Gigantic,
                    ));
                }
            }
            PageSize::Huge => {
                const MASK: usize = (1 << 21) - 1;
                if virt_addr & MASK != 0 {
                    return Err(PagingError::MisalignedAddress(virt_addr, PageSize::Huge));
                }
            }
            PageSize::Normal => {
                const MASK: usize = (1 << 12) - 1;
                if virt_addr & MASK != 0 {
                    return Err(PagingError::MisalignedAddress(virt_addr, PageSize::Normal));
                }
            }
        }

        let entry: &mut PagingStructEntry = (&leaf).into();
        *entry = PagingStructEntry::default();

        if (virt_addr >> 51) == 0 {
            let phys_page = self
                .mapped_pages
                .get_mut(&leaf.phys_addr)
                .expect("Mapped pages should be in mapped_pages")
                as *mut MappedPage;
            // SAFETY: We either dereference the pointer and decrement the ref count or we remove
            //         the MappedPage, but not both.
            let cr3 = unsafe {
                let table = self.environment.paging_structure_base() as *mut PagingStruct;
                (*table).phys_addr::<E>()
            };

            unsafe {
                if (*phys_page).refs.load(SeqCst) > 1 {
                    (*phys_page).refs.fetch_sub(1, SeqCst);
                    assert!(
                        (*phys_page)
                            .aliasing_paging_structures
                            .remove(&(cr3, virt_addr)),
                        "Aliasing cr3 must have been in aliasing list: ({cr3:x}, {virt_addr:x})"
                    );
                    if (*phys_page).refs.load(SeqCst) == 1 {
                        assert_eq!(
                            (*phys_page).aliasing_paging_structures.len(),
                            1,
                            "There must be exactly one aliasing paging structure"
                        );
                        let (pml4, _) = (*phys_page)
                            .aliasing_paging_structures
                            .first()
                            .expect("There must be exactly one aliasing paging structure");
                        let pml4 = self.table_for_phys_addr(*pml4);
                        let leaf = self
                            .walk_to_leaf(pml4, virt_addr)
                            .expect("virt_addr must be mapped");
                        let entry: &mut PagingStructEntry = (&leaf).into();
                        entry.set_flags(RW_FLAG, true);
                    }
                } else {
                    self.mapped_pages.remove(&leaf.phys_addr);
                }
            }
        }

        // TODO: Garbage collection of no longer used paging structures.
        //       This only has meaning once we have memory reclamation in Mapper.
        Ok(())
    }

    pub fn alloc_page_at(&mut self, virt_addr: usize) -> Result<(), PagingError> {
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
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct;
        let leaf = self.walk_to_leaf(pml4, virt_addr)?;
        match leaf.page_size {
            PageSize::Gigantic => Some(leaf.phys_addr | (virt_addr & ((1 << PDPT::SHIFT) - 1))),
            PageSize::Huge => Some(leaf.phys_addr | (virt_addr & ((1 << PD::SHIFT) - 1))),
            PageSize::Normal => Some(leaf.phys_addr | (virt_addr & ((1 << PT::SHIFT) - 1))),
        }
    }

    pub fn fork(&mut self, paging_struct_base: *mut PagingStruct) -> usize {
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
            let dst_cr3 = unsafe {
                (*dst_pml4).get_entry_mut(i).set_flags(flags, true);
                let dst_table_addr = (*dst_pdpt).phys_addr::<E>();
                (*dst_pml4).get_entry_mut(i).set_addr(dst_table_addr);
                (*dst_pml4).phys_addr::<E>()
            };

            let src_pdpt = unsafe {
                let entry = (*src_pml4).get_entry(i);
                let phys_addr = entry.get_addr();
                self.table_for_phys_addr(phys_addr)
            };
            self.recursively_clone(src_pdpt, dst_pdpt, dst_cr3, 3, i << 39);
        }

        unsafe { (*dst_pml4).phys_addr::<E>() }
    }

    fn recursively_clone(
        &mut self,
        src: *mut PagingStruct,
        dst: *mut PagingStruct,
        dst_cr3: usize,
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
                mp.aliasing_paging_structures.insert((dst_cr3, virt_addr));
            } else {
                // src_entry is a pdpte or pde with PS == 0
                if src_flags & PRESENT_FLAG != PRESENT_FLAG {
                    continue;
                }
                let dst_table = self.new_table();
                unsafe {
                    let dst_entry = (*dst).get_entry_mut(i);
                    let dst_table_addr = (*dst_table).phys_addr::<E>();
                    dst_entry.set_addr(dst_table_addr);
                    dst_entry.set_flags(src_flags, true);
                    dst_entry.set_flags(RW_FLAG, true)
                }

                let src_table = self.table_for_phys_addr(src_addr);
                self.recursively_clone(src_table, dst_table, dst_cr3, level - 1, virt_addr);
            }
        }
    }

    pub fn cow(&mut self, virt_addr: *mut u8) -> Result<(), PagingError> {
        let pml4 = self.environment.paging_structure_base() as *mut PagingStruct;

        let leaf = self
            .walk_to_leaf(pml4, virt_addr as usize)
            .expect("Page should be mapped");
        match leaf.page_size {
            PageSize::Gigantic => {
                panic!("Copy-on-write of gigantic pages is unsupported");
            }
            PageSize::Huge => {
                panic!("Copy-on-write of huge pages is unsupported");
            }
            PageSize::Normal => {
                let src_phys_page = self
                    .mapped_pages
                    .get(&leaf.phys_addr)
                    .expect("Mapped page should be in mapped_pages");
                assert!(
                    src_phys_page.refs.load(SeqCst) > 1,
                    "Mapped page ref count should be greater than 1"
                );
            }
        }

        let new_page = self.cow_tmp_map();

        self.cow_copy(virt_addr, self.cow_dest.0);

        self.unmap(virt_addr as usize)?;
        self.unmap(usize::from(&self.cow_dest))?;
        self.map(new_page.get_addr(), virt_addr as usize)
    }

    fn cow_tmp_map(&mut self) -> Block {
        let new_page = self
            .page_allocator
            .allocate(12)
            .expect("Physical memory exhausted!");
        self.map(new_page.get_addr(), usize::from(&self.cow_dest));
        new_page
    }

    fn cow_copy(&mut self, src: *mut u8, dest: *mut u8) {
        // TODO: support huge and potentially gigantic pages.
        unsafe {
            ptr::copy_nonoverlapping(src, dest, 0x1000);
        }
    }

    fn new_table(&mut self) -> *mut PagingStruct {
        let new_table = unsafe { self.base.0.add(self.next) };
        self.next += 1;
        new_table
    }

    fn table_for_phys_addr(&self, phys_addr: usize) -> *mut PagingStruct {
        unsafe {
            let idx = (E::PAGING_STRUCTURE_BASE + phys_addr - usize::from(&self.base))
                / size_of::<PagingStruct>();
            self.base.0.add(idx)
        }
    }

    /// Walk the paging structure of type `L` to get the entry of `virt_addr`.
    /// If the virtual address is not mapped, returns `None`.
    /// Returns the **physical address** of the resolved paging structure.
    fn walk<L: PagingLevel>(
        &self,
        table: *const PagingStruct,
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
        table: *mut PagingStruct,
        virt_addr: usize,
    ) -> (*mut PagingStruct, usize) {
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
                    entry.set_addr((*next_table).phys_addr::<E>());
                    entry.set_flags(flags, true);
                }
                (next_table, flags)
            }
        }
    }

    fn walk_to_leaf(&self, pml4: *mut PagingStruct, virt_addr: usize) -> Option<LeafEntry> {
        let (pml4e_addr, _) = self.walk::<PML4>(pml4, virt_addr)?;
        let pdpt = self.table_for_phys_addr(pml4e_addr);

        let (pdpte_addr, pdpte_flags) = self.walk::<PDPT>(pdpt, virt_addr)?;
        if pdpte_flags & PS_FLAG == PS_FLAG {
            return Some(LeafEntry {
                table: pdpt,
                phys_addr: pdpte_addr,
                idx: PDPT::entry_idx(virt_addr),
                page_size: PageSize::Gigantic,
            });
        }
        let pd = self.table_for_phys_addr(pdpte_addr);
        let (pde_addr, pde_flags) = self.walk::<PD>(pd, virt_addr)?;
        if pde_flags & PS_FLAG == PS_FLAG {
            return Some(LeafEntry {
                table: pd,
                phys_addr: pde_addr,
                idx: PD::entry_idx(virt_addr),
                page_size: PageSize::Huge,
            });
        }
        let pt = self.table_for_phys_addr(pde_addr);
        let (pte_addr, _) = self.walk::<PT>(pt, virt_addr)?;
        Some(LeafEntry {
            table: pt,
            phys_addr: pte_addr,
            idx: PT::entry_idx(virt_addr),
            page_size: PageSize::Normal,
        })
    }
}
