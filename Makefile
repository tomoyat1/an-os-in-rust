all: build

clean:
	cargo clean

build:
	cargo build -Z build-std=core,alloc  --target x86_64-unknown-aosir.json
