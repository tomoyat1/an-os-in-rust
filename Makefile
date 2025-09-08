all: build

clean:
	cargo clean

build:
	cargo build -Z build-std=core,alloc  --target x86_64-unknown-aosir.json
	cd bootloader && cargo build -Z build-std=core,alloc,std --target x86_64-unknown-uefi
