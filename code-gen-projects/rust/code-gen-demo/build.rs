// build.rs

use std::env;
use std::io;
use std::io::*;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // Invoke cargo CLI
    let ion_cli = env::var("ION_CLI").unwrap_or("ion".to_string());
    println!("cargo:warn=Running command: {}", ion_cli);
    let mut cmd = std::process::Command::new(ion_cli);
    cmd.arg("generate")
        .arg("-l")
        .arg("rust")
        .arg("-d")
        .arg(format!("{}/../../schema", crate_dir))
        .arg("-o")
        .arg(&out_dir);

    println!("cargo:warn=Running: {:?}", cmd);

    let output = cmd.output().expect("failed to execute process");

    println!("status: {}", output.status);
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();

    assert!(output.status.success());

    println!("cargo:rerun-if-changed=input/");
    println!("cargo:rerun-if-changed=schema/");
}
