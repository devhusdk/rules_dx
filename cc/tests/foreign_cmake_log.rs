use std::path::{Path, PathBuf};

fn path(var: &str, name: &str) -> PathBuf {
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    let rootpaths = std::env::var(var).unwrap_or_else(|_| panic!("{var}"));
    let rootpath = rootpaths
        .split_whitespace()
        .find(|p| p.ends_with(name))
        .unwrap_or_else(|| panic!("{name}"));
    Path::new(&srcdir).join(workspace).join(rootpath)
}

fn read(var: &str, name: &str) -> String {
    let path = path(var, name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn log() -> String {
    read("DX_FOREIGN_CMAKE_LOG", "CMake.log")
}

fn gcc_log() -> String {
    read("DX_FOREIGN_CMAKE_LOG_GCC", "gcc_configure.log")
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

#[test]
fn configure_uses_the_provisioned_gcc() {
    let log = gcc_log();
    assert!(log.contains("The C compiler identification is GNU"));
    assert!(log.contains("DX_FOREIGN_CMAKE_FLAG"));
    assert!(!log.contains("/usr/bin/cc"));
    assert!(!log.contains("/usr/bin/gcc"));
}

#[test]
fn switching_compiler_reconfigures() {
    let (clang, gcc) = (log(), gcc_log());
    assert_ne!(clang, gcc);
    assert!(clang.contains("Clang") && !clang.contains("identification is GNU"));
    for log in [clang, gcc] {
        assert!(!log.contains("Reusing"));
    }
}

#[test]
fn the_generated_zconf_header_is_installed() {
    let path = path("DX_FOREIGN_CMAKE_INCLUDE", "include").join("zconf.h");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(text.lines().any(|l| l == "#define Z_HAVE_UNISTD_H"));
    assert!(text.lines().any(|l| l == "/* #undef Z_PREFIX */"));
    assert!(!text.contains("#cmakedefine"));
}
