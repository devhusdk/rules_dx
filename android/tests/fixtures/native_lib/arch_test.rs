//! Device, emulator and API 34 libraries carry the architecture and API level they were built for.

use std::env;
use std::process::Command;

fn readelf(flag: &str, library: &str) -> String {
    let tool = env::var("DX_READELF").expect("DX_READELF");
    let path = env::var(library).unwrap_or_else(|_| panic!("{library}"));
    let output = Command::new(tool)
        .args([flag, &path])
        .output()
        .expect("run llvm-readelf");
    assert!(output.status.success(), "llvm-readelf {flag} {path} failed");
    String::from_utf8(output.stdout).expect("utf8 output")
}

fn machine(library: &str) -> String {
    readelf("--file-header", library)
        .lines()
        .find(|line| line.contains("Machine:"))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn device_library_is_aarch64() {
    let machine = machine("DX_DEVICE_SO");
    assert!(machine.contains("AArch64"), "device library: {machine}");
}

#[test]
fn emulator_library_is_x86_64() {
    let machine = machine("DX_EMULATOR_SO");
    assert!(machine.contains("X86-64"), "emulator library: {machine}");
}

#[test]
fn api34_library_targets_api_level_34() {
    let notes = readelf("--notes", "DX_API34_SO");
    let data = notes
        .lines()
        .find_map(|line| line.trim().strip_prefix("description data:"))
        .unwrap_or_else(|| panic!("no android ident note in {notes}"));
    let level: Vec<u8> = data
        .split_whitespace()
        .take(4)
        .map(|byte| u8::from_str_radix(byte, 16).expect("hex byte"))
        .collect();
    assert_eq!(level, 34u32.to_le_bytes(), "{notes}");
}
