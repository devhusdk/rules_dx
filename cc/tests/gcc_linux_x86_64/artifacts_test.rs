use dx_testing::{image_machine, read_runfiles, resolve_runfiles, starlark_const};

#[test]
fn gcc_x86_64_artifacts_carry_the_pinned_compiler_and_glibc_floor() {
    let pins = read_runfiles(&std::env::var("PINS").expect("PINS"));
    let version = std::env::var("GCC_VERSION").expect("GCC_VERSION");
    let floor = glibc_version(&starlark_const(&pins, "GCC_GLIBC", "pins.bzl")["glibc_".len()..]);
    let stamp = format!("GCC: (GNU) {version}");

    let paths = std::env::var("GCC_X86_64").expect("GCC_X86_64");
    let paths: Vec<&str> = paths.split_whitespace().collect();
    assert_eq!(paths.len(), 3, "expected three artifacts, got {paths:?}");
    for rel in paths {
        let path = resolve_runfiles(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
        assert_eq!(
            image_machine(&bytes).as_deref(),
            Some("x86_64"),
            "{rel}: machine"
        );
        assert!(
            dx_testing::contains_bytes(&bytes, stamp.as_bytes()),
            "{rel}: missing {stamp:?}"
        );
        assert!(!dx_testing::contains_bytes(&bytes, b"clang version"), "{rel}: built by clang");
        assert!(
            !dx_testing::contains_bytes(&bytes, b"libstdc++.so"),
            "{rel}: links libstdc++ dynamically"
        );
        if !rel.ends_with(".so") {
            assert!(
                dx_testing::contains_bytes(&bytes, b"/lib64/ld-linux-x86-64.so.2"),
                "{rel}: missing the glibc interpreter"
            );
        }
        let needed = highest_glibc(&bytes);
        assert!(
            needed <= floor,
            "{rel}: needs GLIBC_{}.{} above the floor {}.{}",
            needed.0,
            needed.1,
            floor.0,
            floor.1
        );
    }
}

fn glibc_version(text: &str) -> (u32, u32) {
    let (major, minor) = text.split_once('.').expect("glibc version");
    (major.parse().expect("major"), minor.parse().expect("minor"))
}

fn highest_glibc(bytes: &[u8]) -> (u32, u32) {
    const MARK: &[u8] = b"GLIBC_2.";
    let mut highest = (0, 0);
    for start in bytes
        .windows(MARK.len())
        .enumerate()
        .filter(|(_, window)| *window == MARK)
        .map(|(index, _)| index + MARK.len())
    {
        let digits: String = bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .map(|&byte| char::from(byte))
            .collect();
        if let Ok(minor) = digits.parse() {
            highest = highest.max((2, minor));
        }
    }
    highest
}
