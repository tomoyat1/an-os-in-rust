# An Operating System in Rust

> Now with Cow (paging, not the mooing kind)

## Requirements

- Rust nightly-2026-05-31
    - Be sure to add `rust-src` component, as it will be needed to build rust core library for `x86_64-unknown-uefi`.
    - Linux host
      ```console
      % cd /path/to/an-os-in-rust
      % rustup toolchain install nightly-2026-06-26-x86_64-unknown-linux-gnu
      % rustup override set nightly-2026-06-26-x86_64-unknown-linux-gnu # This sets the toolchain for the an-os-in-rust directory.
      % rustup component add rust-src --toolchain nightly-2026-06-26-x86_64-unknown-linux-gnu  # This adds the `rust-src` component for the nightly toolchain
      ```
    - macOS host
      ```console
      % cd /path/to/an-os-in-rust
      % rustup toolchain install nightly-2026-06-26-aarch64-apple-darwin
      % rustup override set nightly-2026-06-26-aarch64-apple-darwin # This sets the toolchain for the an-os-in-rust directory.
      % rustup component add rust-src --toolchain nightly-2026-06-26-aarch64-apple-darwin  # This adds the `rust-src` component for the nightly toolchain
      ```
- An UEFI compatible machine (Tested on QEMU with OVMF)

## Building

The following will build both the bootloader and the kernel. Results will be found under `target/`.

```console
% make
```

## Running

1. Copy the bootloader executable `target/x86_64-unknown-uefi/debug/bootx64.efi` to `\EFI\BOOT\bootx64.efi` in your
   virtual machine's EFI System Partition.
2. Copy the kernel executable `target/x86_64-unknown-aosir/debug/aosir` to `\aosir` in your
   virtual machine's EFI System Partition.
3. Turn on your machine.
   ```console
   % sudo qemu-system-x86_64 \
     -S \
     -gdb tcp:localhost:1234 \
     -drive if=pflash,format=raw,readonly,file=./qemu/OVMF_CODE-with-csm.fd \
     -hda fat:rw:./qemu/hda \
     -monitor stdio \
     -drive if=pflash,format=raw,file=./qemu/OVMF_VARS-with-csm.fd \
     -d cpu_reset \
     -m 4096 \
     -nic tap,ifname=tap0,model=rtl8139
   ```
4. Observe that it does nothing useful ;)
5. If you use `arping` to broadcast an ARP request, the kernel will respond.
   ```console
   % sudo arping -I tap0 -U -S 192.168.18.1  192.168.18.40 // Both source and destination IP address don't matter now.
   ARPING 192.168.18.40
   42 bytes from 52:54:00:12:34:56 (192.168.18.40): index=0 time=7.971 msec
   42 bytes from 52:54:00:12:34:56 (192.168.18.40): index=1 time=4.276 msec
   42 bytes from 52:54:00:12:34:56 (192.168.18.40): index=2 time=4.179 msec
   42 bytes from 52:54:00:12:34:56 (192.168.18.40): index=3 time=5.585 msec
   ...
   ```
