use dx_testing::resolve_runfiles;
use std::process::Command;

#[test]
fn release_cell_script_parses() {
    let status = Command::new("bash")
        .arg("-n")
        .arg(resolve_runfiles("tools/ci/release_cell.sh"))
        .status()
        .expect("spawn bash");
    assert!(status.success());
}
