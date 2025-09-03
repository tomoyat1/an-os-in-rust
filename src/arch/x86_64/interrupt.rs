use alloc::vec;
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use super::pit;
use crate::drivers::{acpi, serial};
use crate::locking::spinlock::WithSpinLock;

extern "C" {
    fn page_fault_isr();
    fn general_protection_fault_isr();
    fn ps2_keyboard_isr();
    fn pit_isr();
    fn com0_isr();
    fn reload_idt(idtr: *const IDTR);
}

extern "C" {
    static mut device_isr_entries: [[u8; 7]; 96];
}

pub static mut IOAPIC: WithSpinLock<IOAPIC> = WithSpinLock::new(IOAPIC::new(0));

pub static mut LOCAL_APIC: WithSpinLock<LocalAPIC> = WithSpinLock::new(LocalAPIC::new(0));

static mut IDT: WithSpinLock<[u128; 40]> = WithSpinLock::new([0; 40]);

// TODO: lock this properly
static mut IRQ_HANDLERS: [usize; 128] = [0; 128];

#[repr(C)]
#[repr(packed)]
struct IDTR {
    limit: u16,
    base: usize,
}

type IDT = vec::Vec<u128>;

pub fn init(madt: &acpi::MADT) -> u32 {
    let mut idt = unsafe { IDT.lock() };
    // Set up each fault and IRQ handler.
    // TODO: make IRQ handlers pluggable from outside this module.
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

    // serial port handler
    {
        let mut descriptor: u128 = 0;
        let handler = com0_isr as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0b1110
        descriptor |= 8 << 44; // Present flag

        idt[0x24] = descriptor;
    }

    // Interrupt 0x26
    // TODO: set up for all interrupts in 0x20-0x7f inclusive.
    {
        let mut descriptor: u128 = 0;
        let handler = unsafe { &device_isr_entries[0x26] } as *const u8 as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0b1110
        descriptor |= 8 << 44; // Present flag

        idt[0x26] = descriptor;
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
    let lapic_id = lapic.id();
    ioapic.remap_all(lapic_id);

    unsafe {
        let mut static_ioapic = IOAPIC.lock();
        *static_ioapic = ioapic;
        let mut static_lapic = LOCAL_APIC.lock();
        *static_lapic = lapic;
    }

    lapic_id
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
    let mut lapic = LOCAL_APIC.lock();
    lapic.write(0xb0, 0)
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

#[no_mangle]
unsafe extern "C" fn com0_handler() {
    serial::read_com1();
    let mut lapic = LOCAL_APIC.lock();
    lapic.write(0xb0, 0)
}

pub fn register_handler(vector: u8, handler: extern "C" fn(u64)) {
    // SAFETY: not safe yet
    unsafe {
        IRQ_HANDLERS[vector as usize] = handler as usize;
    }
}

#[no_mangle]
unsafe extern "C" fn device_handler(vector: u64) {
    // SAFETY: NOT safe, because the static mut [usize; 128] is not behind any lock.
    let handler = unsafe { IRQ_HANDLERS[vector as usize] } as *const ();

    // SAFETY: All code that has been written to IRQ_HANDLERS should have made sure that what
    //         valid they write are function address.
    let handler = unsafe {
        // Should `handler` be null checked? Or is it alright to just page fault.
        core::mem::transmute::<*const (), fn(u64)>(handler)
    };
    handler(vector)
}

/// Mask 8259 PIC.
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

    pub(crate) fn remap(&self, lapic_id: u32, int_signal: u32, vector: u32) {
        let low = 0x10 + int_signal * 2;
        let high = 0x10 + int_signal * 2 + 1;
        self.write(low, vector);
        self.write(high, (lapic_id << 24) & 0x0f000000);
    }

    fn remap_all(&self, lapic_id: u32) {
        // PS/2 keyboard
        // self.write(0x12, 0x21);
        // self.write(0x13, (lapic_id << 24) & 0x0f000000);
        self.remap(0, 1, 0x21);

        // PIT
        // The following assumes that PIT is wired to ISA line 0 and remapped to line 2 of I/O APIC; confirmed from MADT
        // TODO: parse MADT for remappings on boot.
        // Also, we mask PIT until it is properly initialized later.
        // self.write(0x14, 0x10020);
        // self.write(0x15, (lapic_id << 24) & 0x0f000000);
        self.remap(lapic_id, 2, 0x10020);

        // COM 1, 3
        // self.write(0x18, 0x24);
        // self.write(0x19, (lapic_id << 24) & 0x0f000000);
        self.remap(lapic_id, 4, 0x24);

        // Mouse (masked)
        // TODO: how do we know that the mouse is on I/O APIC line 12?
        self.write(0x28, 0x100FF);
        self.write(0x29, (lapic_id << 24) & 0x0f000000);
        self.remap(lapic_id, 12, 0x100ff);

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
    pub(crate) fn id(&self) -> u32 {
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

    pub fn end_of_interrupt(&self) {
        self.write(0xb0, 0)
    }
}
