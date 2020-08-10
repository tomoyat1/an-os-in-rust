const EI_NIDENT: usize = 16;

#[repr(C)]
struct ElfHeader {
    e_ident: [char; EI_NIDENT],
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
