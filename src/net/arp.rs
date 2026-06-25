use crate::net::ethernet::{EtherType, FrameBuilder, MACAddress};

use alloc::vec::Vec;

enum Opcode {
    Request = 1,
    Reply = 2,
    Unknown,
}

impl From<u16> for Opcode {
    fn from(value: u16) -> Self {
        match value {
            1 => Opcode::Request,
            2 => Opcode::Reply,
            _ => Opcode::Unknown,
        }
    }
}

impl From<Opcode> for u16 {
    fn from(value: Opcode) -> Self {
        match value {
            Opcode::Request => 1,
            Opcode::Reply => 2,
            Opcode::Unknown => 0,
        }
    }
}

#[repr(transparent)]
pub(crate) struct ARP<'a> {
    bytes: &'a [u8],
}

impl<'a> ARP<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    pub fn hardware_address_space(&self) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.bytes[0..2]);
        u16::from_be_bytes(bytes)
    }

    pub fn protocol_address_space(&self) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.bytes[2..4]);
        u16::from_be_bytes(bytes)
    }

    pub fn hardware_address_length(&self) -> u8 {
        self.bytes[4]
    }

    pub fn protocol_address_length(&self) -> u8 {
        self.bytes[5]
    }

    pub fn opcode(&self) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.bytes[6..8]);
        u16::from_be_bytes(bytes)
    }

    pub fn sender_hardware_address(&self) -> &[u8] {
        &self.bytes[8..8 + self.hardware_address_length() as usize]
    }

    pub fn sender_protocol_address(&self) -> &[u8] {
        let start = 8 + self.hardware_address_length() as usize;
        &self.bytes[start..start + self.protocol_address_length() as usize]
    }

    pub fn target_hardware_address(&self) -> &[u8] {
        let start =
            8 + self.hardware_address_length() as usize + self.protocol_address_length() as usize;
        &self.bytes[start..start + self.hardware_address_length() as usize]
    }

    pub fn target_protocol_address(&self) -> &[u8] {
        let start = 8
            + self.hardware_address_length() as usize
            + self.protocol_address_length() as usize
            + self.hardware_address_length() as usize;
        &self.bytes[start..start + self.protocol_address_length() as usize]
    }
}

pub(crate) struct ARPBuilder<'a, S: builder::Step> {
    buf: &'a mut [u8],
    pos: usize,
    _phantom: core::marker::PhantomData<S>,
}

impl<'a> ARPBuilder<'a, builder::HardwareAddressSpace> {
    pub fn new(buf: &'a mut [u8]) -> ARPBuilder<'a, builder::HardwareAddressSpace> {
        ARPBuilder {
            buf,
            pos: 0,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn hardware_address_space(
        mut self,
        space: u16,
    ) -> ARPBuilder<'a, builder::ProtocolAddressSpace> {
        self.buf[self.pos..self.pos + 2].copy_from_slice(&space.to_be_bytes());
        self.pos += 2;
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::ProtocolAddressSpace> {
    pub fn protocol_address_space(
        mut self,
        space: u16,
    ) -> ARPBuilder<'a, builder::HardwareAddressLength> {
        self.buf[self.pos..self.pos + 2].copy_from_slice(&space.to_be_bytes());
        self.pos += 2;
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::HardwareAddressLength> {
    pub fn hardware_address_length(
        mut self,
        length: u8,
    ) -> ARPBuilder<'a, builder::ProtocolAddressLength> {
        self.buf[self.pos] = length;
        self.pos += 1;
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::ProtocolAddressLength> {
    pub fn protocol_address_length(mut self, length: u8) -> ARPBuilder<'a, builder::Opcode> {
        self.buf[self.pos] = length;
        self.pos += 1;
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::Opcode> {
    pub fn opcode(mut self, opcode: Opcode) -> ARPBuilder<'a, builder::SenderHardwareAddress> {
        self.buf[self.pos..self.pos + 2].copy_from_slice(&u16::from(opcode).to_be_bytes());
        self.pos += 2;
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::SenderHardwareAddress> {
    pub fn sender_hardware_address(
        mut self,
        address: &[u8],
    ) -> ARPBuilder<'a, builder::SenderProtocolAddress> {
        self.buf[self.pos..self.pos + address.len()].copy_from_slice(address);
        self.pos += address.len();
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::SenderProtocolAddress> {
    pub fn sender_protocol_address(
        mut self,
        address: &[u8],
    ) -> ARPBuilder<'a, builder::TargetHardwareAddress> {
        self.buf[self.pos..self.pos + address.len()].copy_from_slice(address);
        self.pos += address.len();
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::TargetHardwareAddress> {
    pub fn target_hardware_address(
        mut self,
        address: &[u8],
    ) -> ARPBuilder<'a, builder::TargetProtocolAddress> {
        self.buf[self.pos..self.pos + address.len()].copy_from_slice(address);
        self.pos += address.len();
        ARPBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> ARPBuilder<'a, builder::TargetProtocolAddress> {
    pub fn target_protocol_address(mut self, address: &[u8]) -> ARP<'a> {
        self.buf[self.pos..self.pos + address.len()].copy_from_slice(address);
        self.pos += address.len();
        ARP {
            bytes: &self.buf[..self.pos],
        }
    }
}

pub(crate) mod builder {
    pub trait Step {}

    pub struct HardwareAddressSpace;
    impl Step for HardwareAddressSpace {}

    pub struct ProtocolAddressSpace;
    impl Step for ProtocolAddressSpace {}

    pub struct HardwareAddressLength;
    impl Step for HardwareAddressLength {}

    pub struct ProtocolAddressLength;
    impl Step for ProtocolAddressLength {}

    pub struct Opcode;
    impl Step for Opcode {}

    pub struct SenderHardwareAddress;
    impl Step for SenderHardwareAddress {}

    pub struct SenderProtocolAddress;
    impl Step for SenderProtocolAddress {}

    pub struct TargetHardwareAddress;
    impl Step for TargetHardwareAddress {}

    pub struct TargetProtocolAddress;
    impl Step for TargetProtocolAddress {}
}

pub fn reply_writer(
    recv_bytes: &[u8],
    sha: MACAddress,
) -> impl FnOnce(&mut [u8]) -> usize + use<'_> {
    let request = ARP::from_bytes(recv_bytes);
    let sha: [u8; 6] = sha.into();

    move |buf| {
        ARPBuilder::new(buf)
            .hardware_address_space(request.hardware_address_space())
            .protocol_address_space(request.protocol_address_space())
            .hardware_address_length(request.hardware_address_length())
            .protocol_address_length(request.protocol_address_length())
            .opcode(Opcode::Reply)
            .sender_hardware_address(&sha)
            .sender_protocol_address(request.target_protocol_address())
            .target_hardware_address(request.sender_hardware_address())
            .target_protocol_address(request.target_protocol_address())
            .len()
    }
}
