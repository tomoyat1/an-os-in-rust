use alloc::vec;
use core::borrow::Borrow;
use core::ptr::{write_volatile, read_volatile};

use crate::drivers::acpi;
use crate::locking;
use crate::locking::spinlock::WithSpinLock;
use super::pit;

extern "C" {
    fn page_fault_isr();
    fn general_protection_fault_isr();
    fn ps2_keyboard_isr();
    fn pit_isr();
    fn reload_idt(idtr: *const IDTR);
}

pub static mut IOAPIC: WithSpinLock<IOAPIC> = WithSpinLock::new(IOAPIC::new(0));

pub static mut LOCAL_APIC: WithSpinLock<LocalAPIC> = WithSpinLock::new(LocalAPIC::new(0));

#[repr(C)]
#[repr(packed)]
struct IDTR {
    limit: u16,
    base: usize,
}

type IDT = vec::Vec<u128>;

pub fn init(madt: acpi::MADT) {
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

    // Set IDTR
    let idtr = IDTR {
        limit: 40 * 16 - 1,
        base: idt.as_ptr() as usize,
    };
    unsafe {
        reload_idt(&idtr as *const IDTR);
    }

    // The following assumes the runtime environment is APIC based.
    // Behaviour is undefined on systems without APIC.
    mask_pic();
    let lapic = LocalAPIC::new(madt.lapic_addr);
    let ioapic = IOAPIC::new(madt.ioapic_addr);
    // Don't consider global interrupt base for now.
    ioapic.remap(lapic.id());

    unsafe {
        let mut static_ioapic = IOAPIC.lock();
        *static_ioapic = ioapic;
        let mut static_lapic = LOCAL_APIC.lock();
        *static_lapic = lapic;
    }
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
    // no-op
    // TODO: Do stuff with tick
    pit::pit_tick();
    let mut lapic = LOCAL_APIC.lock();
    lapic.write(0xb0, 0)
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

/// Masks specified I/O APIC line when mask is true.
pub fn mask_line(mask: bool, vector: u32) {
    let ioapic = unsafe { IOAPIC.lock() };
    ioapic.mask_line(mask, vector);
}

pub struct IOAPIC {
    base_addr: usize,
}

impl IOAPIC {
    const fn new(base_addr: usize) -> Self {
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

    fn read(&self, index: u32) -> u32 {
        let ioregsel = self.base_addr as *mut u32;
        let iowin = (self.base_addr + 0x10) as *mut u32;
        unsafe {
            write_volatile(ioregsel, index);
            read_volatile(iowin)
        }
    }

    fn remap(&self, lapic_id: u32) {
        // PS/2 keyboard
        self.write(0x12, 0x21);
        self.write(0x13, (lapic_id << 24) & 0x0f000000);

        // PIT
        // The following assumes that PIT is wired to ISA line 0 and remapped to line 2 of I/O APIC
        // TODO: parse MADT for remappings
        // Also, we mask PIT until it is properly initialized later.
        self.write(0x14, 0x10020);
        self.write(0x15, (lapic_id << 24) & 0x0f000000);

        // Mouse (masked)
        self.write(0x28, 0x100FF);
        self.write(0x19, (lapic_id << 24) & 0x0f000000);

        // Spurious Interrupt Vector
        self.write(0xf0, (0x1 << 8) + 0xff);

        // Enable interrupts
        unsafe {
            asm!("sti");
        }
    }

    fn mask_line(&self, mask: bool, vector: u32) {
        let idx = (vector * 2) + 0x10;
        let redtlb_low = self.read(idx);
        let redtlb_low = if mask {
            redtlb_low | 0x10000
        } else {
            redtlb_low & 0xfffeffff
        };
        self.write(idx, redtlb_low);
    }
}

pub struct LocalAPIC {
    base_addr: usize,
}

impl LocalAPIC {
    const fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    /// Get local APIC ID of executing processor.
    fn id(&self) -> u32 {
        self.read(0x20)
    }

    /// Read register at index.
    fn read(&self, index: usize) -> u32 {
        let reg = (self.base_addr + index as usize) as *mut u32;
        unsafe { *reg }
    }

    // Write register at index.
    fn write(&self, index: usize, value: u32) {
        let mut reg = (self.base_addr + index as usize) as *mut u32;
        unsafe { write_volatile(reg, value) }
    }
}
