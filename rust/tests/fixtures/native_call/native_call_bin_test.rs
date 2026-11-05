use std::path::PathBuf;
use std::process::Command;

#[test]
fn binary_linked_with_cc_library_prints_sum() {
    let bin = PathBuf::from(std::env::var("NATIVE_CALL_BIN").unwrap());
    let output = Command::new(&bin).output().expect("run native_call_bin");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "5");
}
