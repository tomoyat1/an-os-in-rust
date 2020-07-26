all: build

build:
	cargo xbuild --target x86_64-unknown-uefi
