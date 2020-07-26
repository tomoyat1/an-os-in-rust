#![no_std]
#![no_main]
#![feature(abi_efiapi)]

#![feature(alloc)]
extern crate alloc;
extern crate uefi;
extern  crate uefi_services;
extern crate compiler_builtins;


use uefi::prelude::*;
use log::info;

#[entry]
fn efi_main(_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    // Initialize logging.
    uefi_services::init(&system_table);
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
