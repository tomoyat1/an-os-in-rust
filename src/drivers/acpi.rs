use alloc::format;
use alloc::string::String;
use core::ptr::slice_from_raw_parts;
use crate::arch::x86_64::mm::init_mm;

pub struct MADT {
    pub lapic_addr: usize,
    pub ioapic_addr: usize, // TODO: consider case where multiple I/O APICs exist.
    pub global_system_interrupt_base: u32,
}

#[repr(C)]
struct _MADT {
    // LAPIC name comes from x86,
    pub lapic_addr: u32,
    pub flags: u32,

    // Size of struct depends on its type, so as a whole treat it as a byte array.
    // This is a heterogeneous list.
    // Do some black magic to provide a safe interface.
    int_ctrlr_struct: u8,
}

#[repr(C)]
struct RSDP {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rstd_addr: u32,
    length: u32,
    xsdt_addr: usize,
    // omit extended checksum
}

#[repr(C)]
struct XSDT {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: [u8; 4],
    creator_id: [u8; 4],
    creator_revision: [u8; 4],
    entry: *const usize, // offset 36
}

#[repr(C)]
struct DescriptionHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: [u8; 4],
    creator_id: [u8; 4],
    creator_revision: [u8; 4],
    data: u8, // offset 36
}

pub fn parse_madt(rsdp: *const core::ffi::c_void) -> core::result::Result<MADT, String> {
    let rsdp = unsafe { &*(rsdp as *const RSDP) };
    let signature = core::str::from_utf8(&rsdp.signature)
        .map_err(|e| format!("failed to read signature: {:?}", e))?;
    assert_eq!(signature, "RSD PTR ");

    let xsdt = unsafe {&*(rsdp.xsdt_addr as *const XSDT)};
    let len = ((xsdt.length - 36) / 8) as usize;
    // HACK: since we know the offset of xsdt.entry from the ACPI specs, calculate its address manually.
    let xsdt_entry = rsdp.xsdt_addr + 36;
    let xsdt_entry = xsdt_entry as *const usize;
    let xsdt_entry = unsafe {&*slice_from_raw_parts(xsdt_entry, len)};

    let mut madt = MADT{
        lapic_addr: 0,
        ioapic_addr: 0,
        global_system_interrupt_base: 0,
    };
    for &ptr in xsdt_entry.iter() {
        let header = ptr as *const DescriptionHeader;
        let header = unsafe {&*header};
        let signature = core::str::from_utf8(&header.signature)
            .expect("failed to parse signature");

        if signature == "APIC" {
            madt = _parse_madt(ptr, header.length);
        }

    }

    Ok(madt)
}

#[repr(C)]
struct InterruptController<A> {
    ty: u8,
    length: u8,
    type_specific: A,
}

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct IOAPIC {
    id: u8,
    _reserved: u8,

    // This is the 32-bit physical address to which I/O APIC's registers are mapped to.
    pub addr: u32,

    // This is where this I/O APIC's interrupt inputs start.
    pub global_system_interrupt_base: u32,
}

#[repr(packed)]
struct LAPCIOverride {
    _reserved: u16,
    addr: u64,
}

#[repr(packed)]
#[derive(Copy, Clone)]
struct InterruptSourceOverride {
    bus: u8, // This should be 0, which represents ISA

    // Bus-relative interrupt source (IRQ no.)
    source: u8,

    // Global System Interrupt which the bus-relative IRQ will signal.
    global_system_interrupt: u8,
    flags: u16,
}

fn _parse_madt(madt_addr: usize, length: u32) -> MADT {
    let madt = (madt_addr + 36) as *const _MADT;
    let madt = unsafe {&*madt};

    // interrupt controller structure contents are within [head, tail)
    let mut head = madt_addr + 44;
    let tail = madt_addr + length as usize;

    let mut madt_info = MADT{
        lapic_addr: madt.lapic_addr as usize,
        ioapic_addr: 0,
        global_system_interrupt_base: 0
    };
    while head < tail {
        let ty = unsafe {*(head as *const u8)};
        let length = unsafe {*((head + 1) as *const u8)};
        match ty {
            1 => { // I/O APIC
                let controller = head as *const InterruptController<IOAPIC>;
                let controller = unsafe {&*controller};
                madt_info.ioapic_addr = controller.type_specific.addr as usize;
                madt_info.global_system_interrupt_base = controller.type_specific.global_system_interrupt_base;
            },
            2 => { // Interrupt source override
                let mapping = head as *const InterruptController<InterruptSourceOverride>;
                let mapping = unsafe {&*mapping};
                let foo = 1+1;
            },
            5  => { // Local APIC address override
                let lapic_override = head as *const InterruptController<LAPCIOverride>;
                let lapic_override = unsafe {&*lapic_override};
                madt_info.lapic_addr = lapic_override.type_specific.addr as usize;
            }
            _ => {}
        }
        head += length as usize;
    }
    return madt_info;
}
