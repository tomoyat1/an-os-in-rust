use alloc::vec::Vec;
use core::sync::atomic::spin_loop_hint;

use crate::drivers::pci;
use crate::drivers::pci::PCIDevice;
use crate::arch::x86_64::port;

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
pub fn init(pci: &mut pci::PCI) -> Vec<RTL8139>{
    let devices = pci.get_device(RTL8139_VENDOR_ID, RTL8139_DEVICE_ID);
    let mut v = Vec::<RTL8139>::new();

    for pci_dev in devices {
        {
            match RTL8139::init(pci_dev) {
                Ok(rtl8139) => {
                    v.push(rtl8139)
                }, Err(()) => {
                    continue
                }
            }
        }
    };
    v
}

pub struct RTL8139<'pci> {
    // This should ideally be made module private.
    pub(crate) pci: &'pci mut PCIDevice,

    // RTL8139 specific fields follow
    rx_buf: Vec<u8>
}

impl RTL8139<'_> {
    fn init(pci: &mut pci::PCIDevice) -> Result<RTL8139, ()> {
        // Use the rx buffer size 8192 + 16 + 1500 bytes. 8192 + 16 lets us write 0 for the buffer size
        // specification in the next step (receive configuration), and the extra 1500 bytes is for overflow when using
        // WRAP = 1 in the RCR.
        let rx_buf_len = 8192 + 16 + 1500;
        let mut rx_buf = Vec::<u8>::with_capacity(rx_buf_len);
        unsafe {
            rx_buf.set_len(rx_buf_len);
        }

        let mut rtl8139 = RTL8139{
            pci,
            rx_buf,
        };

        // Enable bus mastering. This lets PCI device to perform DMA.
        // Assume that everything is set to 0 after PCI device RST#.
        rtl8139.pci.write_control_register(0b0000000000000100, 0);

        // Power on device
        unsafe {
            rtl8139.outb(REG_CONFIG_1, 0x0);
        };

        // Software Reset
        unsafe  {
            rtl8139.outb(REG_RBSTART, 0x10);
            while rtl8139.inb(REG_RBSTART) & 0x10 != 0 {
                spin_loop_hint();
            }
        };

        // Set up interrupts.
        // 0x0005 sets the ROK and TOK bits, which means we get interrupts when successfully
        // send or receive packets.
        unsafe {
            rtl8139.outw(REG_IMR, 0x0005);
        }

        // Init recv buffer
        // TODO: get physical address of rx_buf. This will require additions to virtual mem code.
        let not_rx_buf_addr = rtl8139.rx_buf.as_ptr();
        unsafe {
            rtl8139.outl(REG_RBSTART, not_rx_buf_addr as u32)
        }

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

        Ok(rtl8139)
    }

    #[inline]
    fn ioaddr(&self, offset: u16) ->  u16 {
        let base =  (self.pci.bar1 ^ 0x1) as u16;
        base + offset
    }

    unsafe fn outb(&self, offset: u16, data: u8) {
        port::outb(self.ioaddr(offset), data)
    }

    unsafe fn inb(&self, offset: u16) -> u8 {
        port::inb(self.ioaddr(offset) + offset)
    }

    unsafe fn outw(&self, offset: u16, data: u16) {
        port::outw(self.ioaddr(offset), data)
    }

    unsafe fn outl(&self, offset: u16, data: u32) {
        // Assume here that bar1 contains an IO port addr.
        // TODO: support memory mapped registers.
        port::outl(self.ioaddr(offset) + offset, data)
    }
}

// TODO: write actual code for ISR handler.
//       This function should
//       1. Figure out what kind of event woke us up by reading the interrupt status register.
//       2. Handle the event.
//       3. Clear the corresponding bit in the interrupt status reg.
