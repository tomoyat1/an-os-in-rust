use alloc::vec::Vec;
use core::num::Wrapping;

use crate::arch::x86_64::port;
use crate::locking::spinlock::WithSpinLock;

const REG_CAP_PTR: u16 = 0x34;

// Interrupt vector for RTL8139 MSI. This should be configured per-device
// per-device, but we go with a hard-coded constant for simplicity for the time being.
const MSI_VECTOR: u32 = 0x26;

const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

static mut PCI: WithSpinLock<Option<PCI>> = WithSpinLock::new(None);

pub fn init(lapic_id: u32) {
    let p = PCI {
        devices: enumerate_pci_bus(lapic_id),
    };
    unsafe {
        let mut pci = PCI.lock();
        *pci = Some(p);
    }
}

pub struct PCI {
    devices: Vec<PCIDevice>,
}

// Note: An instance of PCI that is static mut should exist. See comment in rtl8139 driver code.

impl PCI {
    pub fn get_device(&mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        let vendor_id = vendor_id;
        let device_id = device_id;

        let p = |x: &mut PCIDevice| -> bool { x.device_id == device_id && x.vendor_id == vendor_id };

        let mut v = Vec::<PCIDevice>::new();

        // Give ownership of matched devices to the caller.
        for d in self.devices.drain_filter(p) {
            v.push(d)
        }
        v
    }
}

/// This represents a PCI device on a PCI bus.
/// TODO: Consider field visibility.
pub struct PCIDevice {
    bus_number: u16,
    device_number: u16,

    device_id: u16,
    vendor_id: u16,
    subsystem_id: u16,
    subsystem_vendor_id: u16,

    msi_capability_pointer: Option<u16>,
}

impl PCIDevice {
    unsafe fn outl(&self, offset: u16, function: u32, data: u32) {
        let n_bus = self.bus_number as u32;
        let n_device = self.device_number as u32;
        let function: u32 = (function & 0b111) << 8;
        let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11 | function | offset as u32;

        // Set register to write to.
        port::outl(CONFIG_ADDRESS, cfg_addr);

        // Actually write.
        port::outl(CONFIG_DATA, data)
    }

    unsafe fn inl(&self, offset: u16, function: u32) -> u32 {
        let n_bus = self.bus_number as u32;
        let n_device = self.device_number as u32;
        let function: u32 = (function & 0b111) << 8;
        let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11 | function | (offset & 0xFC) as u32;

        // Set register to write to.
        port::outl(CONFIG_ADDRESS, cfg_addr);

        // Actually read.
        port::inl(CONFIG_DATA)
    }

    pub fn read_control_register(&self, function: u32) -> u16 {
        let read = unsafe { self.inl(0x4, function) };
        (read & 0x0000FFFF) as u16
    }

    pub fn write_control_register(&self, control: u16, function: u32) {
        let status: u32 = 0x0 << 16;
        let data = status | control as u32;
        unsafe { self.outl(0x4, function, data) }
    }

    pub fn read_status_register(&self, function: u32) -> u16 {
        let read = unsafe { self.inl(0x4, function) };
        ((read & 0xffff0000) >> 16) as u16
    }

    pub fn read_bar1(&self, function: u32) -> u32 {
        unsafe { self.inl(0x10, function) }
    }

    pub fn write_bar1(&self, function: u32, data: u32) {
        unsafe { self.outl(0x10, function, data) }
    }
}

/// Enumerates PCI bus for devices.
fn enumerate_pci_bus(lapic_id: u32) -> Vec<PCIDevice> {
    let mut devices = Vec::<PCIDevice>::new();
    for n_bus in 0..256 as u32 {
        for n_device in 0..32 as u32 {
            // We assume single function devices for now.
            let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11;
            let cfg_data = unsafe {
                port::outl(CONFIG_ADDRESS, cfg_addr);
                port::inl(CONFIG_DATA)
            };

            if cfg_data & 0xffff == 0xffff {
                // Invalid vendor ID.
                // Nothing found at bus:device combination.
                continue;
            }

            let vendor_id = (cfg_data & 0xffff) as u16;
            let device_id = ((cfg_data & 0xffff0000) >> 16) as u16;

            let mut device = PCIDevice {
                bus_number: n_bus as u16,
                device_number: n_device as u16,
                vendor_id,
                device_id,

                // TODO: properly fill in these fields, and possibly more.
                subsystem_id: 0,
                subsystem_vendor_id: 0,
                msi_capability_pointer: None,
            };

            // Look for MSI capbility struture
            // TODO: generalize this to enumerate all capabilities in the future.
            let status = device.read_status_register(0);
            if status & 0b10000 != 0 {
                let mut ptr = REG_CAP_PTR;
                while ptr != 0 {
                    let (ctrl, n_ptr, cap_id) = {
                        let v = unsafe { device.inl(ptr, 0) };
                        ((v & 0xffff0000) >> 16, (v & 0xff00) >> 8, v & 0xff)
                    };
                    if cap_id != 0x05 {
                        ptr = n_ptr as u16;
                        continue;
                    }
                    device.msi_capability_pointer = Some(ptr);

                    // enable MSI
                    unsafe {
                        device.outl(ptr, 0x0, 0x1);
                    };

                    // Set message address. Just use 32 bit address for now.
                    let dest_id: u32 = lapic_id << 12;
                    let addr: u32 = 0xfee00000 | dest_id | (0b00 << 2);
                    unsafe { device.outl(ptr + 0x4, 0, addr) }

                    // Set message data.
                    let data: u32 = MSI_VECTOR; // edge triggered, fixed delivery mode
                    unsafe { device.outl(ptr + 0x8, 0, data) }

                    break;
                }
            }

            devices.push(device);
        }
    }

    return devices;
}

impl WithSpinLock<Option<PCI>> {
    pub fn get_device(&mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        let mut pci = unsafe { self.lock() };
        let pci = pci.as_mut();
        if let Some(pci) = pci {
            pci.get_device(vendor_id, device_id)
        } else {
            alloc::vec!()
        }
    }
}

pub struct Handle;

impl Handle {
    pub fn get_device<'a>(mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        unsafe { PCI.get_device(vendor_id, device_id) }
    }
}
