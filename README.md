# An Operating System in Rust
> Doesn't do anything useful.

## Requirements
- [cargo-xbuild](https://github.com/rust-osdev/cargo-xbuild)
    - This project is not compatible with builtin cargo `build-std` feature yet.
- Latest nightly Rust toolchain.
    - Be sure to add `rust-src` component, as it will be needed to build rust core library for `x86_64-unknown-uefi`.
    - Steps:
      ```console
      % rustup default nightly  # This sets default toolchain to nightly
      % rustup component add rust-src  # This adds the `rust-src` component for nightly toolchain
      ```
- An UEFI compatible machine (Tested on Hyper-V VM)

## Building
```console
% make
```

## Running
1. Copy executable to your machine's EFI system partition
1. Turn on your machine.
1. Observe that it does nothing useful ;)
