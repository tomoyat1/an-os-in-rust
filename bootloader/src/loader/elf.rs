use alloc::format;
use alloc::string::String;
use core::mem;

use crate::framebuffer::Framebuffer;
use core::fmt::Write;
use core::str::from_utf8;

const EI_NIDENT: usize = 16;

#[repr(C)]
struct ElfHeader {
    e_ident: [u8; EI_NIDENT],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64, // Section header table's file offset in bytes.
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize:  u16,
    e_phnum: u16,
    e_shentsize: u16, // Number of entries in section header table.
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct ProgramHeader {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_aligh: u64,
}

pub fn load_elf(elf_file: &[u8], fb: &mut Framebuffer) -> core::result::Result<(), String> {
    let elf_header: *const u8 = &elf_file[0];
    let elf_header = unsafe { &*(elf_header as *const ElfHeader)};
    let magic = from_utf8(&elf_header.e_ident[1..4])
        .map_err(|e| format!("failed to read magic: {:?}", e))?;
    writeln!(fb, "e_ident: {}", magic);

    // In the end, it doesn't even matter.
    Ok(())
}
