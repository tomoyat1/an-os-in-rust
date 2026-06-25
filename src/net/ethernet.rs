use alloc::vec::Vec;
use core::fmt::{Display, Formatter};
use core::mem::MaybeUninit;
use core::slice;

pub mod raw {
    pub type MACAddress = [u8; 6];

    pub type EtherType = [u8; 2];

    pub type VLANTag = [u8; 2];

    pub type CRC = [u8; 4];
}

#[repr(transparent)]
pub(crate) struct MACAddress(raw::MACAddress);

impl From<[u8; 6]> for MACAddress {
    fn from(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }
}

impl From<MACAddress> for [u8; 6] {
    fn from(mac: MACAddress) -> Self {
        mac.0
    }
}

impl Display for MACAddress {
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
    ARP,
    Other([u8; 2]),
}

impl EtherType {
    pub fn as_bytes(&self) -> [u8; 2] {
        match self {
            Self::ARP => [0x08, 0x06],
            Self::Other(bytes) => *bytes,
        }
    }
}

impl Display for EtherType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ARP => {
                write!(f, "ARP")
            }
            Self::Other(bytes) => {
                write!(f, "0x{:x}{:x}", bytes[0], bytes[1])
            }
        }
    }
}

#[repr(transparent)]
pub(crate) struct Frame<'a> {
    bytes: &'a [u8],
}

impl<'a> Frame<'a> {
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, &'a str> {
        if bytes.len() < 64 || bytes.len() > 1534 {
            return Err("Ethernet frame length out of valid bounds");
        };
        Ok(Self { bytes })
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    pub fn dest(&self) -> MACAddress {
        let mut bytes: raw::MACAddress = [0; 6];
        bytes.copy_from_slice(&self.bytes[0..6]);
        MACAddress::from(bytes)
    }

    pub fn src(&self) -> MACAddress {
        let mut bytes: raw::MACAddress = [0; 6];
        bytes.copy_from_slice(&self.bytes[6..12]);
        MACAddress::from(bytes)
    }

    pub fn vlan_tags(&self) -> [Option<[u8; 2]>; 2] {
        let mut tags = [None; 2];
        let mut bytes = &self.bytes[12..];
        let mut i = 0usize;
        loop {
            let (tpid_or_ethertype, b) = bytes.split_at(size_of::<raw::EtherType>());
            bytes = b;
            match *tpid_or_ethertype {
                [0x81, 0x00] => {
                    if i >= 2 {
                        continue;
                    }
                    let (t, _) = b.split_at(size_of::<raw::VLANTag>());
                    let mut tag = [0u8; 2];
                    tag.copy_from_slice(t);
                    tags[i] = Some(tag)
                }
                _ => break,
            }
        }

        tags
    }

    pub fn ethertype(&self) -> EtherType {
        let mut bytes = &self.bytes[12..];
        loop {
            let (tpid_or_ethertype, b) = bytes.split_at(size_of::<raw::EtherType>());
            match *tpid_or_ethertype {
                [0x81, 0x00] => {
                    (_, bytes) = b.split_at(size_of::<raw::VLANTag>());
                    continue;
                }
                _ => {
                    let mut et_bytes = [0u8; 2];
                    et_bytes.copy_from_slice(tpid_or_ethertype);
                    let ethertype = match *tpid_or_ethertype {
                        [0x8, 0x6] => EtherType::ARP,
                        _ => EtherType::Other(et_bytes),
                    };
                    return ethertype;
                }
            }
        }
    }

    pub fn payload(&self) -> &[u8] {
        let payload_with_crc = self.payload_with_crc();
        &payload_with_crc[0..payload_with_crc.len() - 4]
    }

    pub fn crc(&self) -> [u8; 4] {
        let payload_with_crc = self.payload_with_crc();
        let crc_bytes = &payload_with_crc[payload_with_crc.len() - 4..];
        let mut crc = [0u8; 4];
        crc.copy_from_slice(crc_bytes);
        crc
    }

    fn payload_with_crc(&self) -> &[u8] {
        let mut bytes = &self.bytes[12..];
        loop {
            let (tpid_or_ethertype, b) = bytes.split_at(size_of::<raw::EtherType>());
            match *tpid_or_ethertype {
                [0x81, 0x00] => {
                    (_, bytes) = b.split_at(size_of::<raw::VLANTag>());
                    continue;
                }
                _ => {
                    bytes = b;
                    break;
                }
            }
        }
        bytes
    }
}

pub(crate) struct FrameBuilder<'a, S: builder::Step> {
    buf: &'a mut [u8],
    pos: usize,
    _phantom: core::marker::PhantomData<S>,
}

impl<'a> FrameBuilder<'a, builder::Dest> {
    pub fn new(buf: &'a mut [u8]) -> FrameBuilder<'a, builder::Dest> {
        FrameBuilder {
            buf,
            pos: 0,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn dest(mut self, dest: MACAddress) -> FrameBuilder<'a, builder::Src> {
        self.buf[self.pos..self.pos + 6].copy_from_slice(&dest.0);
        self.pos += 6;
        FrameBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> FrameBuilder<'a, builder::Src> {
    pub fn src(mut self, src: MACAddress) -> FrameBuilder<'a, builder::VLANTag> {
        self.buf[self.pos..self.pos + 6].copy_from_slice(&src.0);
        self.pos += 6;
        FrameBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> FrameBuilder<'a, builder::VLANTag> {
    pub fn vlan_tags(
        mut self,
        tags: [Option<raw::VLANTag>; 2],
    ) -> FrameBuilder<'a, builder::EtherType> {
        let bytes = tags
            .into_iter()
            .flatten()
            .flat_map(|tag| [0x81, 0x00].into_iter().chain(tag));
        for b in bytes {
            self.buf[self.pos] = b;
            self.pos += 1;
        }

        FrameBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn ethertype(mut self, ethertype: EtherType) -> FrameBuilder<'a, builder::Payload> {
        self.buf[self.pos..self.pos + 2].copy_from_slice(&ethertype.as_bytes());
        self.pos += 2;
        FrameBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> FrameBuilder<'a, builder::EtherType> {
    pub fn ethertype(mut self, ethertype: EtherType) -> FrameBuilder<'a, builder::Payload> {
        self.buf[self.pos..self.pos + 2].copy_from_slice(&ethertype.as_bytes());
        self.pos += 2;
        FrameBuilder {
            buf: self.buf,
            pos: self.pos,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> FrameBuilder<'a, builder::Payload> {
    pub fn payload(mut self, writer: impl FnOnce(&mut [u8]) -> usize) -> Frame<'a> {
        let len = writer(&mut self.buf[self.pos..]);
        let end = self.pos + len;
        Frame {
            bytes: &self.buf[..end],
        }
    }
}

pub(crate) mod builder {
    pub trait Step {}

    pub struct Dest;
    impl Step for Dest {}

    pub struct Src;
    impl Step for Src {}

    pub struct VLANTag;
    impl Step for VLANTag {}

    pub struct EtherType;
    impl Step for EtherType {}

    pub struct Payload;
    impl Step for Payload {}
}
