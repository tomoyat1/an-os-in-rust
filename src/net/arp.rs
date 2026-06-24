use crate::net::ethernet::{Builder, EtherType, MACAddress};

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

struct ARP {
    hardware_address_space: u16,
    protocol_address_space: u16,
    opcode: u16,
    sender_hardware_address: Vec<u8>,
    sender_protocol_address: Vec<u8>,
    target_hardware_address: Vec<u8>,
    target_protocol_address: Vec<u8>,
}

impl ARP {
    fn from_bytes(bytes: &[u8]) -> Self {
        let (hrd, remaining) = bytes.split_at(size_of::<u16>());
        let hardware_address_space = u16::from_be_bytes(hrd.try_into().unwrap());

        let (pro, remaining) = remaining.split_at(size_of::<u16>());
        let protocol_address_space = u16::from_be_bytes(pro.try_into().unwrap());

        let (hln, remaining) = remaining.split_at(size_of::<u8>());
        let hardware_address_length = u8::from_be_bytes(hln.try_into().unwrap());
        let mut sender_hardware_address = Vec::with_capacity(hardware_address_length as usize);
        let mut target_hardware_address = Vec::with_capacity(hardware_address_length as usize);

        let (pln, remaining) = remaining.split_at(size_of::<u8>());
        let protocol_address_length = u8::from_be_bytes(pln.try_into().unwrap());
        let mut sender_protocol_address = Vec::with_capacity(protocol_address_length as usize);
        let mut target_protocol_address = Vec::with_capacity(protocol_address_length as usize);

        let (op, remaining) = remaining.split_at(size_of::<u16>());
        let opcode = u16::from_be_bytes(op.try_into().unwrap());

        let (sha, remaining) = remaining.split_at(hardware_address_length as usize);
        sender_hardware_address.extend_from_slice(sha);

        let (spa, remaining) = remaining.split_at(protocol_address_length as usize);
        sender_protocol_address.extend_from_slice(spa);

        let (tha, remaining) = remaining.split_at(hardware_address_length as usize);
        target_hardware_address.extend_from_slice(tha);

        let (tpa, remaining) = remaining.split_at(protocol_address_length as usize);
        target_protocol_address.extend_from_slice(tpa);

        Self {
            hardware_address_space,
            protocol_address_space,
            opcode,
            sender_hardware_address,
            sender_protocol_address,
            target_hardware_address,
            target_protocol_address,
        }
    }

    fn reply(self, sha: MACAddress) -> Self {
        Self {
            opcode: 2, // TODO: make this a const
            sender_hardware_address: Vec::from(<[u8; 6]>::from(sha)),
            sender_protocol_address: self.target_protocol_address,
            target_hardware_address: self.sender_hardware_address,
            target_protocol_address: self.sender_protocol_address,
            ..self
        }
    }
    fn write_bytes(&self, buf: &mut [u8]) -> usize {
        let mut written = 0usize;

        let mut put = |src: &[u8]| {
            buf[written..written + src.len()].copy_from_slice(src);
            written += src.len();
        };

        put(&u16::to_be_bytes(self.hardware_address_space));
        put(&u16::to_be_bytes(self.protocol_address_space));
        put(&u8::to_be_bytes(self.sender_hardware_address.len() as u8));
        put(&u8::to_be_bytes(self.sender_protocol_address.len() as u8));
        put(&u16::to_be_bytes(self.opcode));
        put(&self.sender_hardware_address);
        put(&self.sender_protocol_address);
        put(&self.target_hardware_address);
        put(&self.target_protocol_address);

        written
    }
}

pub fn send_reply(recv_bytes: &[u8], sha: MACAddress, send_bytes: &mut [u8]) -> usize {
    let received = ARP::from_bytes(recv_bytes);
    match Opcode::from(received.opcode) {
        Opcode::Request => {}
        _ => return 0,
    }
    let reply = received.reply(sha);

    let mut arp_payload = [0u8; 28];
    let arp_len = reply.write_bytes(&mut arp_payload);

    let eth_dest =
        MACAddress::from(<[u8; 6]>::try_from(reply.target_hardware_address.as_slice()).unwrap());
    let eth_src =
        MACAddress::from(<[u8; 6]>::try_from(reply.sender_hardware_address.as_slice()).unwrap());

    Builder::new(send_bytes)
        .dest(eth_dest)
        .src(eth_src)
        .ethertype(EtherType::ARP)
        .payload(&arp_payload[..arp_len])
        .len()
}
