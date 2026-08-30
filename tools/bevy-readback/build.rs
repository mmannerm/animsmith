use std::{env, process::Command};

const EXACT_RUSTC: &str = "rustc 1.95.0 (59807616e 2026-04-14)";

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = env::var_os("RUSTC").expect("Cargo did not provide RUSTC");
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("cannot execute the compiler selected by Cargo");
    if !output.status.success() {
        panic!("the compiler selected by Cargo did not report its version");
    }
    let version = String::from_utf8(output.stdout).expect("rustc version is not UTF-8");
    let version = version.trim();
    assert_eq!(
        version, EXACT_RUSTC,
        "the exact-version Bevy harness must be compiled with {EXACT_RUSTC}"
    );
    println!("cargo:rustc-env=ANIMSMITH_BEVY_READBACK_RUSTC={version}");
}
