use std::io::Command;
use std::os;

fn main() {

    let out_dir = os::getenv("OUT_DIR").unwrap();

    Command::new("make").args(&["-C", "native"]).status().unwrap();

    println!("cargo:rustc-flags=-L {}", out_dir);

}
