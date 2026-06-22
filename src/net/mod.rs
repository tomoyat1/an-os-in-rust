use crate::net::ethernet::MACAddress;
use crate::serial;

use alloc::boxed::Box;
use core::fmt::Write;
use core::fmt::{Debug, Display, Formatter};

pub mod arp;
pub mod ethernet;

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
    match ethernet::Frame::from_bytes(bytes) {
        Ok(frame) => {
            match frame.header.ethertype {
                ethernet::EtherType::ARP => {
                    // Temporary buffer in the heap until we can get a buffer from the
                    // NIC driver.
                    let mut buf = Box::<[u8; 46]>::new([0; 46]);
                    arp::send_reply(frame.payload.as_slice(), mac, buf.as_mut());
                    writeln!(serial::Handle::new(), "Payload:");
                    for (i, byte) in buf.iter().enumerate() {
                        write!(serial::Handle::new(), "{:0>2x}", byte);
                        if i % 16 == 15 {
                            write!(serial::Handle::new(), "\n");
                            continue;
                        }
                        if i % 2 == 1 {
                            write!(serial::Handle::new(), " ");
                            continue;
                        }
                    }
                    writeln!(serial::Handle::new(), "");
                    Ok(())
                }
                _ => Ok(()),
            }
            // writeln!(
            //     serial::Handle::new(),
            //     "src: {}, dest: {}, EtherType: {}",
            //     &frame.header.src_mac,
            //     &frame.header.dest_mac,
            //     &frame.header.ethertype,
            // );
            // writeln!(serial::Handle::new(), "Payload:");
            // for (i, byte) in frame.payload.iter().enumerate() {
            //     write!(serial::Handle::new(), "{:0>2x}", byte);
            //     if i % 16 == 15 {
            //         write!(serial::Handle::new(), "\n");
            //         continue;
            //     }
            //     if i % 2 == 1 {
            //         write!(serial::Handle::new(), " ");
            //         continue;
            //     }
            // }
            // writeln!(serial::Handle::new(), "");
            // write!(serial::Handle::new(), "CRC: ");
            // for byte in frame.crc {
            //     write!(serial::Handle::new(), "{:0>2x}", byte);
            // }
            // writeln!(serial::Handle::new(), "\n");
            // Ok(())
        }
        Err(s) => Err(Error {
            error_type: ErrorType::InvalidFrame,
            message: s,
        }),
    }
}
