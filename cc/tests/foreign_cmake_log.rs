use std::path::{Path, PathBuf};

fn log() -> String {
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    let rootpaths = std::env::var("DX_FOREIGN_CMAKE_LOG").expect("DX_FOREIGN_CMAKE_LOG");
    let rootpath = rootpaths
        .split_whitespace()
        .find(|p| p.ends_with("CMake.log"))
        .expect("CMake.log");
    let path: PathBuf = Path::new(&srcdir).join(workspace).join(rootpath);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn configure_uses_the_selected_clang() {
    assert!(log().contains("The C compiler identification is Clang"));
}

#[test]
fn configured_flags_reach_the_compiler() {
    assert!(log().contains("DX_FOREIGN_CMAKE_FLAG"));
}

#[test]
fn no_host_compiler_is_used() {
    let log = log();
    assert!(!log.contains("/usr/bin/cc"));
    assert!(!log.contains("/usr/bin/gcc"));
}
