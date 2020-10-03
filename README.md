# An Operating System in Rust
> Doesn't do anything useful.

## Requirements
- Latest nightly Rust toolchain.
    - Be sure to add `rust-src` component, as it will be needed to build rust core library for `x86_64-unknown-uefi`.
    - Steps:
      ```console
      % rustup default nightly  # This sets default toolchain to nightly
      % rustup component add rust-src  # This adds the `rust-src` component for nightly toolchain
      ```
- An UEFI compatible machine (Tested on QEMU with OVMF)

## Building
The following will build both the bootloader and the kernel. Results will be found under `target/`.
```console
% make
```

## Running
1. Copy the bootloader executable `target/x86_64-unknown-uefi/debug/bootloader.efi` to `\EFI\BOOT\bootx64.efi` in your virtual machine's EFI System Partition.
1. Copy the kernel executable `target/x86_64-unknown-aosir/debug/an-operating-system-in-rust` to `\aosir` in your virtual machine's EFI System Partition.
1. Turn on your machine.
1. Observe that it does nothing useful ;)
