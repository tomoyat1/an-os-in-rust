use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("gcc").args(&["src/arch/x86_64/boot.s", "-c", "-mcmodel=large", "-o"])
        .arg(&format!("{}/libboot.a", out_dir))
        .status().unwrap();

    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=boot");
    println!("cargo:rerun-if-changed=src/boot/boot.s");
}
