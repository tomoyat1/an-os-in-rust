use alloc::vec::{self, Vec};
use core::num::Wrapping;

use crate::arch::x86_64::port;
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};

const REG_CAP_PTR: u16 = 0x34;

// Interrupt vector for RTL8139 MSI. This should be configured
// for each device, but we go with a hard-coded constant for simplicity for the time being.
const MSI_VECTOR: u32 = 0x26;

const CONFIG_ADDRESS: u16 = 0xcf8;
const CONFIG_DATA: u16 = 0xcfc;

static PCI: WithSpinLock<PCI> = WithSpinLock::new(PCI::new());

pub fn init(lapic_id: u32) {
    let mut pci = unsafe { PCI.lock() };
    pci.enumerate_pci_bus(lapic_id);
}

pub struct PCI {
    devices: Vec<PCIDevice>,
}

// Note: An instance of PCI that is static mut should exist. See comment in rtl8139 driver code.

impl PCI {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    fn visit_configuration_space(
        &mut self,
        n_bus: u32,
        n_device: u32,
        n_function: u32,
        lapic_id: u32,
    ) -> Option<PCIDevice> {
        // TODO: get rid of magic number 0x8000_0000
        let cfg_addr: u32 = 0x8000_0000 | n_bus << 16 | n_device << 11 | n_function << 8;
        let cfg_data = unsafe {
            port::outl(CONFIG_ADDRESS, cfg_addr);
            port::inl(CONFIG_DATA)
        };

        if cfg_data & 0xffff == 0xffff {
            // Invalid vendor ID.
            // Nothing found at bus:device combination.
            return None;
        }

        let vendor_id = (cfg_data & 0xffff) as u16;
        let device_id = ((cfg_data & 0xffff0000) >> 16) as u16;

        let mut device = PCIDevice {
            bus_number: n_bus as u16,
            device_number: n_device as u16,
            function_number: n_function as u8,
            vendor_id,
            device_id,

            interrupt_line: 0,
            interrupt_pin: 0,

            // TODO: properly fill in these fields, and possibly more.
            subsystem_id: 0,
            subsystem_vendor_id: 0,
            msi_capability_pointer: None,
        };

        unsafe {
            device.subsystem_vendor_id = (device.inl(0x2c) & 0xffff) as u16;
            device.subsystem_id = ((device.inl(0x2e) & 0xffff0000) >> 16) as u16;
            device.interrupt_line = (device.inl(0x3c) & 0xff) as u8;
            device.interrupt_pin = ((device.inl(0x3c) & 0xff00) >> 8) as u8;
        }

        // Look for MSI capability structure
        // TODO: generalize this to enumerate all capabilities in the future.
        let status = device.read_status_register();
        if status & 0b10000 != 0 {
            let mut ptr = REG_CAP_PTR;
            while ptr != 0 {
                let (ctrl, n_ptr, cap_id) = {
                    let v = unsafe { device.inl(ptr) };
                    ((v & 0xffff0000) >> 16, (v & 0xff00) >> 8, v & 0xff)
                };
                if cap_id != 0x05 {
                    ptr = n_ptr as u16;
                    continue;
                }
                device.msi_capability_pointer = Some(ptr);

                // enable MSI
                unsafe {
                    // TODO: this is incorrect. The write needs to be a write to bit 16 of the
                    //       u32 at capability pointer.
                    device.outl(ptr, 0x1);
                };

                // Set the message address. Just use 32-bit address for now.
                let dest_id: u32 = lapic_id << 12;
                let addr: u32 = 0xfee00000 | dest_id | (0b00 << 2);
                unsafe { device.outl(ptr + 0x4, addr) }

                // Set message data.
                let data: u32 = MSI_VECTOR; // edge triggered, fixed delivery mode
                unsafe { device.outl(ptr + 0x8, data) }

                break;
            }
        }
        Some(device)
    }

    fn enumerate_pci_bus(&mut self, lapic_id: u32) {
        for n_bus in 0..256 as u32 {
            for n_device in 0..32 as u32 {
                if let Some(device) = self.visit_configuration_space(n_bus, n_device, 0, lapic_id) {
                    if device.read_header_type() & 0b10000000 == 0b10000000 {
                        // TODO: read functions 1 to 7
                        for n_function in 0..8 {
                            if let Some(device) = self
                                .visit_configuration_space(n_bus, n_device, n_function, lapic_id)
                            {
                                self.devices.push(device)
                            }
                        }
                    }
                    self.devices.push(device);
                }
            }
        }
    }

    pub fn get_device(&mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        let vendor_id = vendor_id;
        let device_id = device_id;

        let p =
            |x: &mut PCIDevice| -> bool { x.device_id == device_id && x.vendor_id == vendor_id };

        let mut v = Vec::<PCIDevice>::new();

        // Give ownership of matched devices to the caller.
        for d in self.devices.extract_if(.., p) {
            v.push(d)
        }
        v
    }
}

/// This represents a PCI device on a PCI bus.
/// TODO: Consider field visibility.
pub struct PCIDevice {
    pub(crate) bus_number: u16,
    pub(crate) device_number: u16,
    pub(crate) function_number: u8,

    device_id: u16,
    vendor_id: u16,
    subsystem_id: u16,
    subsystem_vendor_id: u16,

    interrupt_pin: u8,
    pub(crate) interrupt_line: u8,

    msi_capability_pointer: Option<u16>,
}

impl PCIDevice {
    unsafe fn outl(&self, offset: u16, data: u32) {
        let n_bus = self.bus_number as u32;
        let n_device = self.device_number as u32;
        let function: u32 = (self.function_number as u32 & 0b111) << 8;
        let cfg_addr: u32 = 0x80000000 | n_bus << 16 | n_device << 11 | function | offset as u32;

        // Set register to write to.
        port::outl(CONFIG_ADDRESS, cfg_addr);

        // Actually write.
        port::outl(CONFIG_DATA, data)
    }

    unsafe fn inl(&self, offset: u16) -> u32 {
        let n_bus = self.bus_number as u32;
        let n_device = self.device_number as u32;
        let function: u32 = (self.function_number as u32 & 0b111) << 8;
        let cfg_addr: u32 =
            0x80000000 | n_bus << 16 | n_device << 11 | function | (offset & 0xFC) as u32;

        // Set register to write to.
        port::outl(CONFIG_ADDRESS, cfg_addr);

        // Actually read.
        port::inl(CONFIG_DATA)
    }

    pub fn read_control_register(&self) -> u16 {
        let read = unsafe { self.inl(0x4) };
        (read & 0x0000FFFF) as u16
    }

    pub fn write_control_register(&self, control: u16) {
        let status: u32 = 0x0 << 16;
        let data = status | control as u32;
        unsafe { self.outl(0x4, data) }
    }

    pub fn read_status_register(&self) -> u16 {
        let read = unsafe { self.inl(0x4) };
        ((read & 0xffff0000) >> 16) as u16
    }

    fn read_header_type(&self) -> u8 {
        ((unsafe { self.inl(0xC) } & 0x00FF0000) >> 16) as u8
    }

    pub fn read_bar0(&self) -> u32 {
        let header_type: u8 = self.read_header_type();
        match (header_type & 0b1111111) {
            0x0 => unsafe { self.inl(0x10) },
            _ => 0,
        }
    }

    pub fn write_bar1(&self, data: u32) {
        unsafe { self.outl(0x10, data) }
    }
}

impl WithSpinLock<PCI> {
    pub fn get_device(&mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        let mut pci = unsafe { self.lock() };
        pci.get_device(vendor_id, device_id)
    }
}

pub struct Handle<'a> {
    pci: WithSpinLockGuard<'a, PCI>,
}

impl<'a> Handle<'a> {
    pub fn new() -> Self {
        let pci = unsafe { PCI.lock() };
        Self { pci }
    }
    pub fn get_device(&mut self, vendor_id: u16, device_id: u16) -> Vec<PCIDevice> {
        self.pci.get_device(vendor_id, device_id)
    }
}
