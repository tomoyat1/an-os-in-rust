use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Write;
use core::hint::spin_loop;

use crate::arch::x86_64::interrupt::{register_handler, IOAPIC, LOCAL_APIC};
use crate::arch::x86_64::{mm, port};
use crate::drivers::pci::PCIDevice;
use crate::drivers::{acpi, pci};

// For debugging
use crate::drivers::serial;
use crate::locking::spinlock::WithSpinLock;

pub static mut NICS: WithSpinLock<Vec<Arc<RTL8139>>> = WithSpinLock::new(Vec::new());

/// Vendor ID of Realtek
const RTL8139_VENDOR_ID: u16 = 0x10ec;

/// Device ID of RTL8139. This is taken from the datasheet.
const RTL8139_DEVICE_ID: u16 = 0x8139;

/// Registers
const REG_CONFIG_1: u16 = 0x52;

const REG_COMMAND: u16 = 0x37;

const REG_RBSTART: u16 = 0x30;

// Interrupt Mask Register
const REG_IMR: u16 = 0x3c;

// Interrupt Status Register
const REG_ISR: u16 = 0x3e;

const REG_RCR: u16 = 0x44;

/// Initializes all RTL8139s on the PCI bus.
pub fn init<'a>(interrupt_mappings: &Vec<acpi::InterruptMapping>) -> usize {
    let devices = pci::Handle.get_device(RTL8139_VENDOR_ID, RTL8139_DEVICE_ID);

    // SAFETY: we have and exclusive lock on the static mut NICS.
    let mut nics = unsafe { NICS.lock() };

    for pci_dev in devices {
        if let Ok(rtl8139) = RTL8139::init(pci_dev) {
            let mapping = interrupt_mappings
                .iter()
                .filter(|x| x.irq_number == rtl8139.pci.interrupt_line)
                .next();
            if let Some(mapping) = mapping {
                // Register the interrupt handler.
                register_handler(rtl8139.vector, rtl8139_handler);

                // SAFETY: Not really safe yet.
                unsafe {
                    let lapic_id = LOCAL_APIC.lock().id();
                    let ioapic = IOAPIC.lock();

                    // Set up IDT vector with the I/O APIC. Map global_system_interrupt to any open IDT vector.
                    // We know that 0x26 is empty.
                    ioapic.remap(
                        0,
                        mapping.global_system_interrupt as u32,
                        rtl8139.vector as u32,
                    );
                }
            }
            nics.push(rtl8139)
        }
    }
    nics.len()
}

pub struct RTL8139 {
    // This should ideally be made module private.
    pub(crate) pci: PCIDevice,

    // The interrupt vector
    pub(crate) vector: u8,

    // RTL8139 specific fields follow
    rx_buf: Vec<u8>,
}

impl RTL8139 {
    fn init<'a>(mut pci: pci::PCIDevice) -> Result<Arc<RTL8139>, ()> {
        // Use the rx buffer size 8192 + 16 + 1500 bytes. 8192 + 16 lets us write 0 for the buffer size
        // specification in the next step (receive configuration), and the extra 1500 bytes is for overflow when using
        // WRAP = 1 in the RCR.
        let rx_buf_len = 8192 + 16 + 1500;
        let mut rx_buf = Vec::<u8>::with_capacity(rx_buf_len);
        unsafe {
            rx_buf.set_len(rx_buf_len);
        }

        // Everything "works" even though rtl8139 is not mutable because we don't
        // properly do any mutual exclusion for `out` and `in` calls.
        // Things will break when we have processes transmitting and
        // receiving concurrently.
        // TODO: Put the device behind a proper lock.
        //       Need to design the locking mechanism first.
        let rtl8139 = RTL8139 {
            pci,
            rx_buf,
            vector: 0x26, // We magically know that this vector is empty, for now.
        };
        let rtl8139 = Arc::new(rtl8139);

        // Enable bus mastering and IOEN. This lets PCI device to perform DMA.
        rtl8139.pci.write_control_register(0x0005, 0);

        // Power on device
        unsafe {
            rtl8139.outb(REG_CONFIG_1, 0x0);
        };

        // Software Reset
        unsafe {
            rtl8139.outb(REG_COMMAND, 0x10);
            while rtl8139.inb(REG_COMMAND) & 0x10 != 0 {
                spin_loop();
            }
        };

        // Init recv buffer
        let rx_buf_addr = rtl8139.rx_buf.as_ptr();
        let rx_buf_addr = mm::phys_addr(rx_buf_addr);
        unsafe { rtl8139.outl(REG_RBSTART, rx_buf_addr as u32) }

        // Receive configuration
        // Accept
        // - broadcast
        // - multicast
        // - unicast to device MAC address
        // - unicast to any MAC address
        // In other words, any valid packet.
        let accept_config: u32 = 0b1111;

        // Configure WRAP behaviour so that packets overflowing the rx ring buffer would be written
        // to the end in space following the buffer.
        let wrap: u32 = 0b1 << 7;
        unsafe {
            rtl8139.outl(REG_RCR, accept_config | wrap);
        }

        // Enable transmitter and receiver.
        unsafe {
            rtl8139.outb(REG_COMMAND, 0x0c);
        }

        // Set up interrupts.
        // 0x0005 sets the ROK and TOK bits, which means we get interrupts when successfully
        // send or receive packets.
        unsafe {
            rtl8139.outw(REG_IMR, 0x0005);
        }

        Ok(rtl8139)
    }

    #[inline]
    fn ioaddr(&self, offset: u16) -> u16 {
        // If calls to pci.read_bar0() get to slow we should cache the address in RAM.
        // RTL8139 driver should own the cached field.

        // TODO: support multiple function devices.
        let bar_0 = self.pci.read_bar0(0);
        let base = match bar_0 & 0x1 {
            // Memory space BAR
            0 => bar_0 & !0xf,
            // IO space BAR
            1 => bar_0 & !0x3,
            _ => {
                panic!("Bottom bit of BAR 0 was not 0 or 1!")
            }
        } as u16;
        base + offset
    }

    unsafe fn outb(&self, offset: u16, data: u8) {
        port::outb(self.ioaddr(offset), data)
    }

    unsafe fn inb(&self, offset: u16) -> u8 {
        port::inb(self.ioaddr(offset))
    }

    unsafe fn outw(&self, offset: u16, data: u16) {
        port::outw(self.ioaddr(offset), data)
    }

    unsafe fn inw(&self, offset: u16) -> u16 {
        port::inw(self.ioaddr(offset))
    }

    unsafe fn outl(&self, offset: u16, data: u32) {
        port::outl(self.ioaddr(offset), data)
    }

    fn handle_interrupt(&self) {
        // SAFETY: Not confirmed to be safe yet.
        let status = unsafe { self.inw(REG_ISR) };

        // Reset status register so that another frame can be sent / received.
        // SAFETY: Not confirmed to be safe yet.
        unsafe { self.outw(REG_ISR, 0x05) }

        if status & 0x1 == 0x1 {
            writeln!(
                serial::Handle,
                "[INFO] IRQ for RTL8139, bus {}, device {}",
                self.pci.bus_number,
                self.pci.device_number
            );
        }

        if status & 0x4 == 0x4 {
            // not supported yet
        }
    }
}

// This function should be called from the ISR for all RTL8139s, and should determine which one
// got the interrupt.
// TODO: This can be made into a common stub handler for all (PCI) devices.
//       PCIDevice should own each of the per-device structs as trait objects, as well as
//       the vector number.
//       ISRs in isr.s would be defined (generated by a macro at compile time) for each *vector*, instead of each device type.
//       ISRs will call the common stub handler, passing the vector as an argument like how it's done below.
#[no_mangle]
pub extern "C" fn rtl8139_handler(vector: u64) {
    // This is bad

    // SAFETY: not safe. We will probably deadlock if we try to send and receive at the same time.
    let nics = unsafe { NICS.lock() };
    for n in nics.iter() {
        if n.vector != vector as u8 {
            continue;
        }
        n.handle_interrupt()
    }

    unsafe {
        let lapic = LOCAL_APIC.lock();
        lapic.end_of_interrupt();
    }
}
