use alloc::format;
use alloc::string::String;
use alloc::vec;
use core::mem::size_of;
use core::ptr;
use core::ptr::slice_from_raw_parts;

pub struct MADT {
    pub lapic_addr: usize,
    pub ioapic_addr: usize, // TODO: consider case where multiple I/O APICs exist.
    pub global_system_interrupt_base: u32,

    pub interrupt_mappings: vec::Vec<InterruptMapping>,
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
    signature: [u8; 8], // byte offset: 0
    checksum: u8,       // byte offset: 8
    oemid: [u8; 6],     // byte offset: 9
    revision: u8,       // byte offset: 15
    rstd_addr: u32,     // byte offset: 16
    length: u32,        // byte offset: 20
    xsdt_addr: usize,   // byte offset: 24
    ext_chksum: u8,     // byte offset: 32
    _reserved: [u8; 3], // byte offset: 33
}

#[repr(C)]
struct XSDT {
    signature: [u8; 4],        // byte offset: 0
    length: u32,               // byte offset: 4
    revision: u8,              // byte offset: 8
    checksum: u8,              // byte offset: 9
    oemid: [u8; 6],            // byte offset: 10
    oem_table_id: [u8; 8],     // byte offset: 16
    oem_revision: [u8; 4],     // byte offset: 24
    creator_id: [u8; 4],       // byte offset: 28
    creator_revision: [u8; 4], // byte offset: 32
    entry: *const usize,       // byte offset: 36
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
    let rsdp = unsafe { ptr::read_unaligned(rsdp as *const RSDP) };
    assert_eq!(&rsdp.signature, b"RSD PTR ");

    let xsdt_addr = rsdp.xsdt_addr;
    let xsdt = unsafe { ptr::read_unaligned(xsdt_addr as *const XSDT) };
    assert_eq!(&xsdt.signature, b"XSDT");

    let len = ((xsdt.length - 36) / 8) as usize;
    // HACK: since we know the offset of xsdt.entry from the ACPI specs, calculate its address manually.
    let xsdt_entry_addr = rsdp.xsdt_addr + 36;
    let xsdt_entry_addr = unsafe { xsdt_entry_addr as *const usize };
    // let xsdt_entry = unsafe { &*slice_from_raw_parts(xsdt_entry_addr, len) };

    let mut madt = MADT {
        lapic_addr: 0,
        ioapic_addr: 0,
        global_system_interrupt_base: 0,
        interrupt_mappings: vec::Vec::new(),
    };
    for e in 0..len {
        let entry_addr = unsafe { ptr::read_unaligned(xsdt_entry_addr.offset(e as isize)) };
        let header = unsafe { ptr::read_unaligned(entry_addr as *const DescriptionHeader) };
        let signature = core::str::from_utf8(&header.signature).expect("failed to parse signature");

        if signature == "APIC" {
            madt = _parse_madt(entry_addr, header.length);
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

#[derive(Copy, Clone)]
pub struct InterruptMapping {
    pub(crate) irq_number: u8,
    pub(crate) global_system_interrupt: u8,
}

fn _parse_madt(madt_addr: usize, length: u32) -> MADT {
    let madt = (madt_addr + 36) as *const _MADT;
    let madt = unsafe { &*madt };

    // interrupt controller structure contents are within [head, tail)
    let mut head = madt_addr + 44;
    let tail = madt_addr + length as usize;

    let mut madt_info = MADT {
        lapic_addr: madt.lapic_addr as usize,
        ioapic_addr: 0,
        global_system_interrupt_base: 0,
        interrupt_mappings: vec::Vec::new(),
    };
    while head < tail {
        let ty = unsafe { *(head as *const u8) };
        let length = unsafe { *((head + 1) as *const u8) };
        match ty {
            1 => {
                // I/O APIC
                let controller = head as *const InterruptController<IOAPIC>;
                let controller = unsafe { &*controller };
                madt_info.ioapic_addr = controller.type_specific.addr as usize;
                madt_info.global_system_interrupt_base =
                    controller.type_specific.global_system_interrupt_base;
            }
            2 => {
                // Interrupt source override
                let mapping = head as *const InterruptController<InterruptSourceOverride>;
                let mapping = unsafe { &*mapping };
                let mapping = InterruptMapping {
                    irq_number: mapping.type_specific.source,
                    global_system_interrupt: mapping.type_specific.global_system_interrupt,
                };
                madt_info.interrupt_mappings.push(mapping)
            }
            5 => {
                // Local APIC address override
                let lapic_override = head as *const InterruptController<LAPCIOverride>;
                let lapic_override = unsafe { &*lapic_override };
                madt_info.lapic_addr = lapic_override.type_specific.addr as usize;
            }
            _ => {}
        }
        head += length as usize;
    }
    madt_info
}
