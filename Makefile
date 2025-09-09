.PHONY: build build-kernel build-bootloader instal
all: build

clean:
	cargo clean

build: build-kernel build-bootloader

build-kernel:
	cargo build -Z build-std=core,alloc  --target x86_64-unknown-aosir.json

build-bootloader:
	cd bootloader && cargo build -Z build-std=core,alloc,std --target x86_64-unknown-uefi

install:
	cp ./target/x86_64-unknown-aosir/debug/aosir ~/src/qemu/hda/aosir
	cp ./target/x86_64-unknown-uefi/debug/bootx64.efi ~/src/qemu/hda/EFI/BOOT/bootx64.efi
