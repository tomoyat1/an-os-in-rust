use alloc::vec::Vec;
use crate::arch::x86_64::port;

/// Vendor ID of Realtek
const RTL8139_VENDOR_ID: u16 = 0x10ec;

/// Device ID of RTL8139. This is taken from the datasheet.
const RTL8139_DEVICE_ID: u16 = 0x8139;

pub fn init() -> bool {
    let v = enumerate_pci_bus();
    match v.len() {
        1 => true,
        _ => false,
    }
}

pub struct PCIDevice {
    bus_number: u16,
    device_number: u16,

    device_id: u16,
    vendor_id: u16,
    bar1: usize,
    bar2: usize,
    bar3: usize,
    bar4: usize,
    bar5: usize,
    subsystem_id: u16,
    subsystem_vendor_id: u16,
}

/// Enumerates PCI bus for devices.
/// For now, this just searches for the RTL8139 and ignores everythin else.
/// TODO: Create an abstraction and split this off to PCI module.
fn enumerate_pci_bus() -> Vec<PCIDevice> {
    let mut devices = Vec::<PCIDevice>::new();
    'enumerate: for n_bus in 0..255 as u32 {
        for n_device in 0..32 as u32 {
            let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11;
            let cfg_data = unsafe {
                port::outl(0xcf8, cfg_addr);
                port::inl(0xcfc)
            };

            if cfg_data == 0xffffffff {
                continue
            }

            // For now, just return the first RTL8139 we find.
            let want = (RTL8139_DEVICE_ID as u32) << 16 | RTL8139_VENDOR_ID as u32;
            if cfg_data == want {
                // TODO: read more from PCI configuration space
                devices.push(PCIDevice{
                    bus_number: n_bus as u16,
                    device_number: n_device as u16,
                    vendor_id: RTL8139_VENDOR_ID,
                    device_id: RTL8139_DEVICE_ID,

                    // TODO: properly fill in these fields, and possibly more.
                    bar1: 0,
                    bar2: 0,
                    bar3: 0,
                    bar4: 0,
                    bar5: 0,
                    subsystem_id: 0,
                    subsystem_vendor_id: 0
                });
                break 'enumerate;
            }
        }
    }

    return devices
}
