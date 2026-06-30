use crate::drivers::net::rtl8139::{NICS, RTL8139};
use crate::drivers::pci;
use crate::locking::semaphore::Semaphore;
use crate::locking::spinlock::WithSpinLock;
use crate::serial;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Write;
use core::fmt::{Debug, Display, Formatter};
use core::ptr::addr_of_mut;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{AcqRel, Acquire};

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

struct RxBuf {
    bytes: Box<[u8; 1526]>,
    len: usize,
}

impl RxBuf {
    fn new() -> Self {
        // SAFETY: The buffer will get overwritten when receiving a frame,
        //         so we can use the uninitialized memory.
        let bytes = unsafe { Box::new_uninit().assume_init() };
        RxBuf { bytes, len: 0 }
    }
}

/// The network stack for a network interface, with an RX ring buffer.
pub struct Interface {
    device: pci::BDF,

    bufs: [WithSpinLock<RxBuf>; 4],
    recv_empty: Semaphore,
    recv_full: Semaphore,
    recv_head: AtomicUsize,
    recv_tail: AtomicUsize,
}

impl Interface {
    pub fn new(device: pci::BDF) -> Self {
        Interface {
            bufs: [
                WithSpinLock::new(RxBuf::new()),
                WithSpinLock::new(RxBuf::new()),
                WithSpinLock::new(RxBuf::new()),
                WithSpinLock::new(RxBuf::new()),
            ],
            recv_empty: Semaphore::new(4, 4),
            recv_full: Semaphore::new(0, 4),
            recv_head: AtomicUsize::new(0),
            recv_tail: AtomicUsize::new(0),
            device,
        }
    }

    pub fn recv_frame(&self, bytes: &[u8]) {
        if !self.recv_empty.try_wait() {
            return;
        }
        let next = self.recv_head.update(AcqRel, Acquire, |n| (n + 1) % 4);
        let mut buf = self.bufs[next].lock();
        buf.bytes[0..bytes.len()].copy_from_slice(bytes);
        buf.len = bytes.len();

        self.recv_full.signal();
    }

    fn handle_frame(&self) -> Result<(), Error> {
        self.recv_full.wait();
        let next = self.recv_tail.update(AcqRel, Acquire, |n| (n + 1) % 4);
        let buf = self.bufs[next].lock();
        let buf = &buf.bytes[0..buf.len];

        let result = match ethernet::Frame::try_from_bytes(&buf) {
            Ok(frame) => {
                writeln!(
                    serial::Handle::new(),
                    "src: {}, dest: {}, EtherType: {}",
                    &frame.src(),
                    &frame.dest(),
                    &frame.ethertype(),
                );
                match frame.ethertype() {
                    EtherType::ARP => {
                        let interface = { NICS.lock().get(&self.device).unwrap().clone() };
                        let arp_writer = arp::reply_writer(frame.payload(), interface.id());
                        interface.transmit(frame.dest(), [None; 2], EtherType::ARP, arp_writer);
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
            Err(s) => Err(Error {
                error_type: ErrorType::InvalidFrame,
                message: s,
            }),
        };
        self.recv_empty.signal();
        result
    }
}

pub static NETWORK_STACK: WithSpinLock<BTreeMap<pci::BDF, Arc<Interface>>> =
    WithSpinLock::new(BTreeMap::new());

pub fn run() {
    // TODO: The following will likely deadlock if we have multiple NICS, but in order to do things
    //       properly we need Scheduler.spawn() to receive closures.
    let nets = {
        NETWORK_STACK
            .lock()
            .values()
            .map(|n| n.clone())
            .collect::<Vec<_>>()
    };
    loop {
        for net in nets.iter() {
            match net.handle_frame() {
                Ok(_) => {}
                Err(e) => {
                    writeln!(serial::Handle::new(), "Error receiving frame: {}", e);
                }
            };
        }
    }
}
