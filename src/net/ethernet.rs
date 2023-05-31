use alloc::vec::Vec;
use core::fmt::{Display, Formatter};
use core::mem::size_of;
use core::slice;

mod raw {
    pub type MACAddress = [u8; 6];

    pub type EtherType = [u8; 2];

    pub type VLANTag = [u8; 4];
}

#[repr(C)]
pub(crate) struct MACAddress([u8; 6]);

impl core::fmt::Display for MACAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5],
        )
    }
}

#[repr(C)]
pub(crate) enum EtherType {
    ARP(),
    Other([u8; 2]),
}

impl Display for EtherType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ARP() => {
                write!(f, "ARP")
            }
            Self::Other(bytes) => {
                write!(f, "0x{:x}{:x}", bytes[0], bytes[1])
            }
        }
    }
}

pub(crate) struct FrameHeader {
    pub(crate) src_mac: MACAddress,
    pub(crate) dest_mac: MACAddress,

    // IEEE 802.3Q tag (optional)
    vlan_tag: Vec<raw::VLANTag>,

    pub(crate) ethertype: EtherType,
}

pub(crate) struct Frame {
    pub(crate) header: FrameHeader,
    payload: Vec<u8>,
    crc: [u8; 4],
}

impl Frame {
    pub fn from_bytes(frame: &[u8]) -> Result<Frame, &str> {
        if frame.len() < size_of::<raw::MACAddress>() {
            return Err("Not enough bytes for MAC source");
        };
        let (src_mac, remaining) = {
            let (data, remaining) = frame.split_at(size_of::<raw::MACAddress>());
            let mut src_mac: [u8; 6] = [0; 6];
            src_mac.copy_from_slice(data);
            (MACAddress(src_mac), remaining)
        };
        if frame.len() < size_of::<raw::MACAddress>() {
            return Err("Not enough bytes for MAC destination");
        };
        let (dest_mac, remaining) = {
            let (data, remaining) = remaining.split_at(size_of::<raw::MACAddress>());
            let mut dest_mac: [u8; 6] = [0; 6];
            dest_mac.copy_from_slice(data);
            (MACAddress(dest_mac), remaining)
        };
        if frame.len() < size_of::<raw::EtherType>() {
            return Err("Not enough bytes for EtherType");
        }
        let mut remaining = remaining;
        let mut ethertype = EtherType::Other([0; 2]);
        let mut vlan_tag = Vec::<[u8; 4]>::new();

        loop {
            let (tpid_or_ethertype, r) = remaining.split_at(size_of::<raw::EtherType>());
            match *tpid_or_ethertype {
                [0x81, 0x00] | [0x8a, 0x88] => {
                    // VLAN tag.
                    let (data, r) = remaining.split_at(size_of::<[u8; 4]>());
                    remaining = r;
                    let mut tag: [u8; 4] = [0; 4];
                    tag.copy_from_slice(data);
                    vlan_tag.push(tag)
                }
                _ => {
                    remaining = r;
                    assert_eq!(tpid_or_ethertype.len(), size_of::<raw::EtherType>());
                    let mut et_bytes: [u8; 2] = [0; 2];
                    et_bytes.copy_from_slice(tpid_or_ethertype);
                    ethertype = match et_bytes {
                        [0x8, 0x6] => EtherType::ARP(),
                        _ => EtherType::Other(et_bytes),
                    };
                    break;
                }
            }
        }

        let payload = Vec::from(remaining);

        Ok(Self {
            header: FrameHeader {
                src_mac,
                dest_mac,
                vlan_tag,
                ethertype,
            },
            payload: Vec::<u8>::new(),
            crc: [0; 4],
        })
    }
}
