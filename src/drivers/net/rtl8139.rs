use alloc::vec::Vec;

use crate::drivers::pci;
use crate::drivers::pci::PCIDevice;

/// Vendor ID of Realtek
const RTL8139_VENDOR_ID: u16 = 0x10ec;

/// Device ID of RTL8139. This is taken from the datasheet.
const RTL8139_DEVICE_ID: u16 = 0x8139;

pub fn init(pci: &pci::PCI) -> Vec<RTL8139>{
    let devices = pci.get_device(RTL8139_VENDOR_ID, RTL8139_DEVICE_ID);
    let mut v = Vec::<RTL8139>::new();
    for pci in devices {
        v.push(RTL8139{ pci })
    };
    v
}


pub struct RTL8139<'pci> {
    pci: &'pci PCIDevice,

    // RTL8139 specific fields follow
}
