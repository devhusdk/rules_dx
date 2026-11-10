use dx_testing::resolve_runfiles;

const EM_AARCH64: u16 = 183;

#[test]
fn linux_arm64_build_is_an_aarch64_elf() {
    let path = resolve_runfiles(&std::env::var("GREET_ARM64").expect("GREET_ARM64"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    assert_eq!(&bytes[..4], b"\x7fELF", "not an ELF file");
    assert_eq!(bytes[5], 1, "not little endian");
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    assert_eq!(machine, EM_AARCH64, "e_machine");
}
