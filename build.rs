
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
fn compile() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    Command::new("make").args(&["-C", "native"]).status().unwrap();
    println!("cargo:rustc-flags=-L {}", out_dir);
}

#[cfg(not(target_os = "macos"))]
fn compile() {
}

fn main() {
    compile();
}
