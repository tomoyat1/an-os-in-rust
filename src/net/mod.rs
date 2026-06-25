use crate::drivers::net::rtl8139::{NICS, RTL8139};
use crate::serial;

use alloc::boxed::Box;
use core::fmt::Write;
use core::fmt::{Debug, Display, Formatter};

pub mod arp;
pub mod ethernet;
use ethernet::{EtherType, MACAddress};

#[derive(Debug)]
enum ErrorType {
    InvalidFrame,
    Unknown,
}

#[derive(Debug)]
pub struct Error<'a> {
    pub error_type: ErrorType,
    pub message: &'a str,
}

impl Display for Error<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}: {}", self.error_type, self.message)
    }
}

impl core::error::Error for Error<'_> {}

pub fn recv_frame(bytes: &[u8], mac: MACAddress) -> Result<(), Error> {
    match ethernet::Frame::try_from_bytes(bytes) {
        Ok(frame) => match frame.ethertype() {
            EtherType::ARP => {
                writeln!(
                    serial::Handle::new(),
                    "src: {}, dest: {}, EtherType: {}",
                    &frame.src(),
                    &frame.dest(),
                    &frame.ethertype(),
                );
                let mut buf = Box::<[u8; 64]>::new([0; 64]);
                let arp_writer = arp::reply_writer(frame.payload(), mac);
                let rtl8139 = NICS.lock()[0].clone();
                rtl8139.transmit(frame.dest(), [None; 2], EtherType::ARP, arp_writer);
                Ok(())
            }
            _ => Ok(()),
        },
        Err(s) => Err(Error {
            error_type: ErrorType::InvalidFrame,
            message: s,
        }),
    }
}
