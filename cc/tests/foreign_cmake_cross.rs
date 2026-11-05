use std::path::{Path, PathBuf};

fn log() -> String {
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    let rootpaths =
        std::env::var("DX_FOREIGN_CMAKE_CROSS_LOG").expect("DX_FOREIGN_CMAKE_CROSS_LOG");
    let rootpath = rootpaths
        .split_whitespace()
        .find(|p| p.ends_with("CMake.log"))
        .expect("CMake.log");
    let path: PathBuf = Path::new(&srcdir).join(workspace).join(rootpath);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn compiles_for_the_arm64_target() {
    assert!(log().contains("-target aarch64-linux-gnu"));
}

#[test]
fn configure_runs_no_target_binary() {
    let log = log();
    assert!(log.contains("Check for working C compiler:"));
    assert!(log.contains("- skipped"));
    assert!(!log.contains("try_run"));
}

#[test]
fn configure_is_a_cross_compile() {
    let log = log();
    let line = log
        .lines()
        .find(|l| l.contains("CMAKE_CROSSCOMPILING") && l.contains("TRUE"))
        .unwrap_or_else(|| panic!("no CMAKE_CROSSCOMPILING TRUE in log"));
    assert!(!line.contains("FALSE"));
}

#[test]
fn no_host_compiler_is_used() {
    let log = log();
    assert!(!log.contains("/usr/bin/cc"));
    assert!(!log.contains("/usr/bin/gcc"));
}
