# An Operating System in Rust
> NIC driver included!

## Requirements
- Rust 1.71.0-nightly (4a59ba4d5 2023-05-01) toolchain.
    - Be sure to add `rust-src` component, as it will be needed to build rust core library for `x86_64-unknown-uefi`.
    - Steps:
      ```console
      % rustup toolchain install --force nightly-2023-05-01
      % rustup default nightly-2023-05-01-x86_64-unknown-linux-gnu  # This sets default toolchain to nightly-2023-05-12
      % rustup component add rust-src --toolchain nightly-2023-05-01-x86_64-unknown-linux-gnu  # This adds the `rust-src` component for the nightly toolchain
      ```
- An UEFI compatible machine (Tested on QEMU with OVMF)

## Building
The following will build both the bootloader and the kernel. Results will be found under `target/`.
```console
% make
```

## Running
1. Copy the bootloader executable `target/x86_64-unknown-uefi/debug/bootx64.efi` to `\EFI\BOOT\bootx64.efi` in your virtual machine's EFI System Partition.
1. Copy the kernel executable `target/x86_64-unknown-aosir/debug/aosir` to `\aosir` in your virtual machine's EFI System Partition.
1. Turn on your machine. 
   ```console
   % sudo qemu-system-x86_64 \
     -S \
     -gdb tcp:localhost:1234 \
     -drive if=pflash,format=raw,readonly,file=/home/tomoyat1/src/qemu/OVMF_CODE-with-csm.fd \
     -hda fat:rw:/home/tomoyat1/src/qemu/hda \
     -monitor stdio \
     -drive if=pflash,format=raw,file=/home/tomoyat1/src/qemu/OVMF_VARS-with-csm.fd \
     -d cpu_reset \
     -m 4096 \
     -nic tap,ifname=tap0,model=rtl8139
   ```
3. Observe that it does nothing useful ;)
4. If you use `arping` to broadcast a ARP request, the results of parsing the ethernet frame header will be outputted to serial console.
   ```console
   % sudo arping -I tap0 -U -S 192.168.11.16  192.168.11.1 // Both source and destination IP address don't matter now.
   ```
