use crate::arch::x86_64::mm;
use crate::arch::x86_64::port;
use crate::drivers::pci::{BarNumber, PCIDevice};
use crate::drivers::{pci, serial};
use crate::locking::spinlock::WithSpinLock;

use crate::arch::x86_64::mm::mapper;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Write};
use core::mem::MaybeUninit;
use util::pointer::SyncMutPointer;
use util::volatile::Volatile;

/// Vendor ID of Intel
const ICH9_AHCI_VENDOR_ID: u16 = 0x8086;

/// Device ID of RTL8139.
const ICH9_AHCI_DEVICE_ID: u16 = 0x2922;

pub static AHCI_CONTROLLERS: WithSpinLock<Vec<Arc<AHCIController>>> = WithSpinLock::new(Vec::new());

pub fn init() -> usize {
    let mut pci = pci::Handle::new();
    // TODO: get ANY AHCI controller, not just the 6-port one on the ICH9.
    let devices = pci.get_device(ICH9_AHCI_VENDOR_ID, ICH9_AHCI_DEVICE_ID);

    let mut controllers = AHCI_CONTROLLERS.lock();

    for device in devices {
        if let Ok(controller) = AHCIController::init(device) {
            controllers.push(controller);
        };
    }

    controllers.len()
}

pub enum Error {
    PortUnsupported(u8),
}

pub enum DeviceType {
    None,
    SATA,
    Unknown,
}

#[repr(C)]
struct GeneralHostControlRegisters {
    capabilities: Volatile<u32>,
    global_hba_control: Volatile<u32>,
    interrupt_status: Volatile<u32>,
    ports_implemented: Volatile<u32>,
    version: Volatile<u32>,
}

#[repr(C)]
struct PortRegisters {
    command_list_base_address: Volatile<u64>,
    fis_base_address: Volatile<u64>,
    interrupt_status: Volatile<u32>,
    interrupt_enable: Volatile<u32>,
    command_status: Volatile<u32>,
    reserved: u32,
    task_file_data: Volatile<u32>,
    signature: Volatile<u32>,
    sata_status: Volatile<u32>,
    sata_control: Volatile<u32>,
    sata_error: Volatile<u32>,
    sata_active: Volatile<u32>,
    command_issue: Volatile<u32>,
    sata_notification: Volatile<u32>,
    fis_based_switching_control: Volatile<u32>,
    device_sleep: Volatile<u32>,
    // Vendor-specific registers are omitted
}

pub struct AHCIController {
    pub(crate) pci: PCIDevice,

    general_host_control: SyncMutPointer<GeneralHostControlRegisters>,
    ports: [Option<SyncMutPointer<PortRegisters>>; 32],
}

impl AHCIController {
    fn init(mut pci: PCIDevice) -> Result<Arc<AHCIController>, ()> {
        writeln!(
            serial::Handle::new(),
            "BAR5: {:x}",
            pci.read_bar_register(BarNumber::BAR5)
        );

        let bar5 = pci.read_bar_register(BarNumber::BAR5);
        let abar_addr = match bar5 & 0x1 {
            0 => (bar5 & !0xf) as usize,
            1 => panic!("BAR5 ABAR should be memory mapped!"),
            _ => {
                panic!("Bottom bit of BAR5 was not 0 or 1!")
            }
        };

        mapper().as_mut().unwrap().map_mmio(abar_addr & !0xfff);

        // TODO: think about pointer provenance when we write tests for the driver.
        let general_host_control: SyncMutPointer<GeneralHostControlRegisters> =
            ((abar_addr + mm::MMIO_BASE) as *mut GeneralHostControlRegisters).into();

        let ports_implemented = unsafe { (*general_host_control).ports_implemented.read() };
        let ports = core::array::from_fn(|i| {
            if ports_implemented & (1 << i) != 0 {
                let port_addr = abar_addr + mm::MMIO_BASE + 0x100 + i * 0x80;
                Some((port_addr as *mut PortRegisters).into())
            } else {
                None
            }
        });

        let ahci_controller = AHCIController {
            pci,
            general_host_control,
            ports,
        };

        ahci_controller.enable_ahci(true);

        let port_0_implemented = ahci_controller.is_port_implemented(0);

        let port_0_clb = (*(ahci_controller.ports[0].as_ref().unwrap()))
            .command_list_base_address
            .read();
        let port_0_fis = (*(ahci_controller.ports[0].as_ref().unwrap()))
            .fis_base_address
            .read();

        let port_0_device_type = (ahci_controller).port_device_type(0);
        let ahci_controller = Arc::new(ahci_controller);

        Ok(ahci_controller)
    }

    /// Enables or disables AHCI mode
    pub fn enable_ahci(&self, enable: bool) {
        let mut ghc = self.general_host_control.global_hba_control.read();
        ghc = if enable {
            ghc | 1 << 31
        } else {
            ghc & !(1 << 31)
        };
        self.general_host_control.global_hba_control.write(ghc);
    }

    /// Returns whether the given port is implemented.
    /// `port` is a port number, starting from 0.
    pub fn is_port_implemented(&self, port: u8) -> bool {
        unsafe { (*self.general_host_control).ports_implemented.read() & (1 << port) != 0 }
    }

    /// Get the device type attached to the `port`.
    /// `port` is a port number, starting from 0.
    pub fn port_device_type(&self, port: u8) -> DeviceType {
        let port = &self.ports[port as usize];
        if port.is_none() {
            return DeviceType::None;
        }
        let sata_status = port.as_ref().unwrap().sata_status.read();
        let device_detection = sata_status & 0xf;
        if device_detection != 3 {
            return DeviceType::None;
        };
        let device_ipm = (sata_status >> 8) & 0xf;
        if device_ipm != 1 {
            return DeviceType::None;
        };
        let signature = port.as_ref().unwrap().signature.read();
        match signature {
            0x00000101 => DeviceType::SATA,
            _ => DeviceType::Unknown,
        }
    }
}
