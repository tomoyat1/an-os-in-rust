use alloc::format;
use alloc::string::String;
use core::mem;

use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use core::str::from_utf8;

const E_IDENT: usize = 16;

#[repr(C)]
struct ElfHeader {
    e_ident: [u8; E_IDENT],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: usize,
    e_phoff: usize,
    e_shoff: usize, // Section header table's file offset in bytes.
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16, // Number of entries in section header table.
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct ProgramHeader {
    p_type: u32,
    p_flags: u32,
    p_offset: usize,
    p_vaddr: usize,
    p_paddr: usize,
    p_filesz: usize,
    p_memsz: usize,
    p_aligh: usize,
}

pub fn load_elf(
    elf_file: &[u8],
) -> Result<unsafe extern "C" fn(&bootlib::types::BootData), String> {
    let elf_header: *const u8 = &elf_file[0];
    let elf_header = unsafe { &*(elf_header as *const ElfHeader) };
    let magic = from_utf8(&elf_header.e_ident[1..4])
        .map_err(|e| format!("failed to read magic: {:?}", e))?;
    assert_eq!(magic, "ELF");

    let program_header: *const u8 = &elf_file[elf_header.e_phoff];
    let program_headers = unsafe {
        let head = &*(program_header as *const ProgramHeader);
        &*slice_from_raw_parts::<ProgramHeader>(head, elf_header.e_phnum as usize)
    };
    assert_eq!(size_of::<ProgramHeader>(), elf_header.e_phentsize as usize);

    // Load segments
    for ph in program_headers.iter() {
        if ph.p_paddr == 0 {
            continue;
        }
        let paddr: *mut u8 = ph.p_paddr as *mut u8;
        let buf = unsafe { &mut *slice_from_raw_parts_mut::<u8>(paddr, ph.p_memsz) };
        for b in buf.iter_mut() {
            *b = 0
        }
        buf[0..ph.p_filesz].copy_from_slice(&elf_file[ph.p_offset..ph.p_offset + ph.p_filesz])
    }

    // use transmute() to forcefully cast e_entry to a fn()
    let void_ptr = elf_header.e_entry as *const ();
    let fn_ptr: unsafe extern "C" fn(&bootlib::types::BootData) =
        unsafe { mem::transmute(void_ptr) };
    Ok(fn_ptr)
}
