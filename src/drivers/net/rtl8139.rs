use alloc::vec::Vec;
use core::sync::atomic::spin_loop_hint;
use core::fmt::Write;

use crate::arch::x86_64::port;
use crate::drivers::pci;
use crate::drivers::pci::PCIDevice;

// For debugging
use crate::drivers::serial;

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

const REG_RCR: u16 = 0x44;

/// Initializes all RTL8139s on the PCI bus.
pub fn init<'a>() -> Vec<RTL8139> {
    let devices = pci::Handle.get_device(RTL8139_VENDOR_ID, RTL8139_DEVICE_ID);
    let mut v = Vec::<RTL8139>::new();

    for pci_dev in devices {
        {
            match RTL8139::init(pci_dev) {
                Ok(rtl8139) => v.push(rtl8139),
                Err(()) => continue,
            }
        }
    }
    v
}

pub struct RTL8139 {
    // This should ideally be made module private.
    pub(crate) pci: PCIDevice,

    // RTL8139 specific fields follow
    rx_buf: Vec<u8>,
}

impl RTL8139 {
    fn init<'a>(mut pci: pci::PCIDevice) -> Result<RTL8139, ()> {
        // Use the rx buffer size 8192 + 16 + 1500 bytes. 8192 + 16 lets us write 0 for the buffer size
        // specification in the next step (receive configuration), and the extra 1500 bytes is for overflow when using
        // WRAP = 1 in the RCR.
        let rx_buf_len = 8192 + 16 + 1500;
        let mut rx_buf = Vec::<u8>::with_capacity(rx_buf_len);
        unsafe {
            rx_buf.set_len(rx_buf_len);
        }

        let mut rtl8139 = RTL8139 { pci, rx_buf };

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
                spin_loop_hint();
            }
        };

        // Init recv buffer
        // TODO: get physical address of rx_buf. This will require additions to virtual mem code.
        let not_rx_buf_addr = rtl8139.rx_buf.as_ptr();
        unsafe { rtl8139.outl(REG_RBSTART, not_rx_buf_addr as u32) }

        // Receive configuration
        // Accept
        // - broadcast
        // - multicast
        // - unicast to device MAC address
        // - unicast to any MAC address
        // In other words, any valid packet.
        let accept_config: u32 = 0b1110;

        // Configure WRAP behaviour so that packets overflowing the rx ring buffer would be written
        // to the end in space following the buffer.
        let wrap: u32 = 0b1 << 7;
        unsafe {
            rtl8139.outl(0x44, accept_config | wrap);
        }

        // Enable transmitter and receiver.
        unsafe {
            rtl8139.outl(REG_COMMAND, 0x0c);
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

    unsafe fn outl(&self, offset: u16, data: u32) {
        // Assume here that bar1 contains an IO port addr.
        // TODO: support memory mapped registers.
        port::outl(self.ioaddr(offset), data)
    }
}

// Note: The struct which represents a single RTL8139 can solely own all the data related to it.
//       It should be put behind a WithSpinLock<T> or better another locking structure with queuing semantics
//       so that both upper-half and lower-half can access it, but not simultaneously (locking structs imply RefCell<T>)
//
//       The IRQ handler should not be a static Fn but a heap-allocated closure containing the call to the RTL8139{}
//       method. For each initialized RTL8139 a closure should be heap-allocated and it's address (function pointer)
//       should be registered to the entity that owns the IRQ number.
//
//       The IRQ number should be owned by the PCI driver. The PCI driver should also have a 'static lifetime
//       so that the IRQ handling function can reference it (in turn the IRQ handling function needs to be a 'static Fn
//       so that the IRQ hander asm shim can link to it :rolling_eyes:)

// TODO: write actual code for ISR handler.
//       This function should
//       1. Figure out what kind of event woke us up by reading the interrupt status register.
//       2. Handle the event.
//       3. Clear the corresponding bit in the interrupt status reg.
// This should be per-rtl8139 instead of a single shared public func.
#[no_mangle]
pub fn rtl8139_handler() {
    // This is bad
    writeln!(serial::Handle, "IRQ for RTL8139!");
}
