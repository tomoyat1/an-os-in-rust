use alloc::vec::Vec;

use crate::arch::x86_64::port;

pub fn init() -> PCI {
    PCI {
        devices: enumerate_pci_bus(),
    }
}

pub struct PCI {
    devices: Vec<PCIDevice>,
}

impl PCI {
    pub fn get_device(&self, vendor_id: u16, device_id: u16) -> Vec<&PCIDevice> {
        let vendor_id = vendor_id;
        let device_id = device_id;
        let iter = self.devices
            .iter()
            .filter(move |x| x.vendor_id == vendor_id && x.device_id == device_id);
        let mut v = Vec::<&PCIDevice>::new();
        for d in iter {
            v.push(d)
        }
        v
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
fn enumerate_pci_bus() -> Vec<PCIDevice> {
    let mut devices = Vec::<PCIDevice>::new();
    for n_bus in 0..255 as u32 {
        for n_device in 0..32 as u32 {
            let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11;
            let cfg_data = unsafe {
                port::outl(0xcf8, cfg_addr);
                port::inl(0xcfc)
            };

            if cfg_data == 0xffffffff {
                continue;
            }

            let vendor_id = (cfg_data & 0xffff) as u16;
            let device_id = ((cfg_data & 0xffff0000) >> 16) as u16;
            devices.push(PCIDevice {
                bus_number: n_bus as u16,
                device_number: n_device as u16,
                vendor_id,
                device_id,

                // TODO: properly fill in these fields, and possibly more.
                bar1: 0,
                bar2: 0,
                bar3: 0,
                bar4: 0,
                bar5: 0,
                subsystem_id: 0,
                subsystem_vendor_id: 0,
            });
        }
    }

    return devices;
}
