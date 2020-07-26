#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(alloc)]
extern crate alloc;
extern crate compiler_builtins;
extern crate uefi;
extern crate uefi_services;

use alloc::vec::*;
use log::info;
use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};
use uefi::table::boot::{EventType, SearchType, TimerTrigger, Tpl};

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);

    // clear screen
    {
        let bs = system_table.boot_services();
        let gop = bs
            .locate_protocol::<GraphicsOutput>()
            .expect("Graphics Output Protocol support is required!");
        let gop = gop.expect("warnings occured when opening GOP");
        let gop = unsafe { &mut *gop.get() };

        let mode_info = gop.current_mode_info();
        let (width_px, height_px) = mode_info.resolution();
        gop.blt(BltOp::VideoFill {
            color: BltPixel::new(0x35, 0x33, 0x2b),
            dest: (0, 0),
            dims: (width_px, height_px),
        });
    }

    info!("Logging initialized :)");

    // Print out the UEFI revision number
    {
        let rev = system_table.uefi_revision();
        let (major, minor) = (rev.major(), rev.minor());

        info!("UEFI revision");
        info!("UEFI {}.{}", major, minor);
    }

    // Proceed to bootstrapping the kernel.
    loop {}
}
