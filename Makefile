all: build

clean:
	cargo clean

build:
	cargo build -Z build-std=core,alloc  --target x86_64-unknown-aosir.json
	cd bootloader && cargo build -Z build-std=core,alloc,std --target x86_64-unknown-uefi

install:
	cp ./target/x86_64-unknown-aosir/debug/aosir ~/src/qemu/hda/aosir
	cp ./target/x86_64-unknown-uefi/debug/bootx64.efi ~/src/qemu/hda/EFI/BOOT/bootx64.efi
