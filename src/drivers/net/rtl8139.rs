use alloc::vec::Vec;

use crate::drivers::pci;
use crate::drivers::pci::PCIDevice;
use crate::arch::x86_64::port;

/// Vendor ID of Realtek
const RTL8139_VENDOR_ID: u16 = 0x10ec;

/// Device ID of RTL8139. This is taken from the datasheet.
const RTL8139_DEVICE_ID: u16 = 0x8139;

pub fn init(pci: &mut pci::PCI) -> Vec<RTL8139>{
    let devices = pci.get_device(RTL8139_VENDOR_ID, RTL8139_DEVICE_ID);
    let mut v = Vec::<RTL8139>::new();
    for pci in devices {
        v.push(RTL8139{ pci })
    };
    v
}

pub struct RTL8139<'pci> {
    // This should ideally be made module private.
    pub(crate) pci: &'pci mut PCIDevice,

    // RTL8139 specific fields follow
}

impl RTL8139<'_> {
    pub fn outl(&self, offset: u16, data: u32) {
        // Assume here that bar1 contains an IO port addr.
        // TODO: support memory mapped registers.
        let ioaddr = (self.pci.bar1 ^ 0x1) as u16;
        unsafe {
            port::outl(ioaddr + offset, data)
        }
    }
}
