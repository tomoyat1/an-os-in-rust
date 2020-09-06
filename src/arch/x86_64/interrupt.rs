use alloc::vec;

use crate::drivers::acpi;
use core::ptr::write_volatile;

extern "C" {
    fn page_fault_isr();
    fn general_protection_fault_isr();
    fn ps2_keyboard_isr();
    fn pit_isr();
    fn reload_idt(idtr: *const IDTR);
}

#[repr(C)]
#[repr(packed)]
struct IDTR {
    limit: u16,
    base: usize,
}

struct IOAPIC {
    base_addr: usize,
}

struct LocalAPIC {
    base_addr: usize,
}

type IDT = vec::Vec<u128>;

pub fn init_int(madt: acpi::MADT) {
    let mut idt = vec::Vec::<u128>::with_capacity(40);
    unsafe {
        idt.set_len(40);
    }

    // page fault handler
    {
        let mut descriptor: u128 = 0;
        let handler = page_fault_isr as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0b1110
        descriptor |= 8 << 44; // Present flag

        idt[0xe] = descriptor;
    }

    // general protection fault handler
    {
        let mut descriptor: u128 = 0;
        let handler = general_protection_fault_isr as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0b1110
        descriptor |= 8 << 44; // Present flag

        idt[0xd] = descriptor;
    }

    // pit handler
    // maybe I fucked this up and handler address non-canonical in terms of IA-32e?
    {
        let mut descriptor: u128 = 0;
        let handler = pit_isr as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0b1110
        descriptor |= 8 << 44; // Present flag

        idt[0x20] = descriptor;
    }
    // yet, deemed insufficient by CPU, raise #GP with 0x202 as error code
    // maybe this has something to do with a task state segment (or lack thereof)

    // Set IDTR
    let idtr = IDTR {
        limit: 40 * 8 - 1,
        base: idt.as_ptr() as usize,
    };
    unsafe {
        reload_idt(&idtr as *const IDTR);
    }

    // The following assumes the runtime environment is APIC based.
    // Behaviour is undefined on systems without APIC.
    // mask_pic();
    let lapic = LocalAPIC::new(madt.lapic_addr);
    let ioapic = IOAPIC::new(madt.ioapic_addr);
    // Don't consider global interrupt base for now.
    ioapic.remap(lapic.id());

}

#[no_mangle]
unsafe extern "C" fn general_protection_fault_handler() {
    let foo = 1 + 1;
    /* no-op */
}

#[no_mangle]
unsafe extern "C" fn page_fault_handler() {
    let foo = 1 + 1;
    /* no-op */
}

#[no_mangle]
unsafe extern "C" fn ps2_keyboard_handler() {
    let foo = 1 + 1;
    /* no-op */
}

#[no_mangle]
unsafe extern "C" fn pit_handler() {
    let foo = 1 + 1;
    /* no-op */
}

fn mask_pic() {
    unsafe {
        asm!(
            "mov {0:l}, 0xff",
            "out 0xa1, {0:l}",
            "out 0x21, {0:l}",
            out(reg_abcd) _,
        )
    }
}

impl IOAPIC {
    fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    fn write(&self, index: u32, value: u32) {
        let ioregsel = self.base_addr as *mut u32;
        let iowin = (self.base_addr + 0x10) as *mut u32;
        unsafe {
            write_volatile(ioregsel, index);
            write_volatile(iowin, value);
        }
    }

    fn remap(&self, lapic_id: u32) {
        // PS/2 keyboard
        self.write(0x12, 0x21);
        self.write(0x13, (lapic_id << 24) & 0x0f000000);

        // PIT
        // The following assumes that PIT is wired to ISA line 0 and remapped to line 2 of I/O APIC
        // TODO: parse MADT for remappings
        self.write(0x14, 0x20);
        self.write(0x15, (lapic_id << 24) & 0x0f000000);

        // Mouse (masked)
        self.write(0x28, 0x100FF);
        self.write(0x19, (lapic_id << 24) & 0x0f000000);

        // Spurious Interrupt Vector
        self.write(0xf0, (0x1 << 8) + 0xff);

        // Enable interrupts
        unsafe {
            asm!(
                "sti"
            );
        }
    }
}

impl LocalAPIC {
    fn new(base_addr: usize) -> Self {
        Self{
            base_addr,
        }
    }

    /// Get local APIC ID of executing processor.
    fn id(&self) -> u32 {
        self.read(0x20)
    }

    /// Read register at index.
    fn read(&self, index: usize) -> u32 {
        let reg = (self.base_addr + index as usize) as *mut u32;
        unsafe {
            *reg
        }
    }
}
