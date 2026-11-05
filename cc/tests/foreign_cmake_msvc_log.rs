use std::path::Path;

fn log() -> String {
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    let rootpaths = std::env::var("DX_FOREIGN_CMAKE_MSVC_LOG").expect("DX_FOREIGN_CMAKE_MSVC_LOG");
    let rootpath = rootpaths
        .split_whitespace()
        .find(|p| p.ends_with("CMake.log"))
        .expect("CMake.log");
    let path = Path::new(&srcdir).join(workspace).join(rootpath);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn configure_uses_msvc() {
    assert!(log().contains("The C compiler identification is MSVC"));
}

#[test]
fn ninja_generator_is_used() {
    let log = log();
    assert!(!log.contains("Visual Studio 1"));
    assert!(!log.contains("NMake"));
}

#[test]
fn crt_flag_matches_the_toolchain() {
    let log = log();
    assert!(log.contains("/MD"));
    assert!(!log.contains("/MT "));
}
