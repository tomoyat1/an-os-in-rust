use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("gcc")
        .args(&["src/arch/x86_64/boot.s", "-c", "-mcmodel=large", "-g", "-o"])
        .arg(&format!("{}/libboot.a", out_dir))
        .status()
        .unwrap();

    Command::new("gcc")
        .args(&["src/arch/x86_64/pm.s", "-c", "-mcmodel=large", "-g", "-o"])
        .arg(&format!("{}/libpm.a", out_dir))
        .status()
        .unwrap();

    Command::new("gcc")
        .args(&["src/arch/x86_64/isr.s", "-c", "-mcmodel=large", "-g", "-o"])
        .arg(&format!("{}/libisr.a", out_dir))
        .status()
        .unwrap();

    Command::new("gcc")
        .args(&["src/arch/x86_64/task_switch.s", "-c", "-mcmodel=large", "-g", "-o"])
        .arg(&format!("{}/libtaskswitch.a", out_dir))
        .status()
        .unwrap();

    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=boot");
    println!("cargo:rustc-link-lib=pm");
    println!("cargo:rustc-link-lib=isr");
    println!("cargo:rustc-link-lib=taskswitch");
    println!("cargo:rerun-if-changed=src/boot/boot.s");
    println!("cargo:rerun-if-changed=src/boot/pm.s");
    println!("cargo:rerun-if-changed=src/boot/isr.s");
    println!("cargo:rerun-if-changed=src/boot/task_switch.s");
}
