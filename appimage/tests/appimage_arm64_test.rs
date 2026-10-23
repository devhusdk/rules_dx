use std::path::PathBuf;

use dx_testing::serde_json::Value;

const ELF_MACHINE_AARCH64: u16 = 183;

fn built(suffix: &str) -> PathBuf {
    let files = std::env::var("DX_APPIMAGE_ARM64").expect("DX_APPIMAGE_ARM64 must be set");
    let rel = files
        .split_whitespace()
        .find(|path| path.ends_with(suffix))
        .unwrap_or_else(|| panic!("no built file ending {suffix}"));
    dx_testing::resolve_runfiles(rel)
}

fn runtime_path() -> PathBuf {
    let rel = std::env::var("DX_APPIMAGE_ARM64_RUNTIME").expect("runtime variable");
    dx_testing::resolve_runfiles(&rel)
}

fn identity() -> Value {
    dx_testing::read_json(&built(".identity.json")).expect("identity json")
}

fn machine(bytes: &[u8]) -> u16 {
    assert_eq!(&bytes[..4], b"\x7fELF", "not an ELF file");
    u16::from_le_bytes([bytes[18], bytes[19]])
}

#[test]
fn image_is_the_pinned_aarch64_runtime_plus_a_squashfs() {
    let runtime = std::fs::read(runtime_path()).expect("runtime");
    let image = std::fs::read(built(".AppImage")).expect("image");
    assert_eq!(machine(&image), ELF_MACHINE_AARCH64);
    assert_eq!(machine(&runtime), ELF_MACHINE_AARCH64);
    let differing: Vec<usize> = (0..runtime.len())
        .filter(|&index| image[index] != runtime[index])
        .collect();
    if let (Some(&first), Some(&last)) = (differing.first(), differing.last()) {
        assert!(
            last - first < 16,
            "runtime bytes changed outside the reserved digest field: {first}..{last}"
        );
    }
    assert_eq!(&image[runtime.len()..runtime.len() + 4], b"hsqs");
    assert!(image.len() > runtime.len() + 4096);
}

#[test]
fn identity_names_the_aarch64_cell_and_the_target_bytes() {
    let identity = identity();
    assert_eq!(identity["arch"], "aarch64");
    assert_eq!(identity["interpreter"], "/lib/ld-linux-aarch64.so.1");
    assert!(
        identity["min_glibc"]
            .as_str()
            .is_some_and(|v| v.starts_with("2.")),
        "{}",
        identity["min_glibc"]
    );
    assert_eq!(
        identity["runtime"]["sha256"].as_str(),
        Some(
            dx_testing::sha256_file(&runtime_path())
                .expect("hash runtime")
                .as_str()
        )
    );
    assert_eq!(
        identity["image_sha256"].as_str(),
        Some(
            dx_testing::sha256_file(&built(".AppImage"))
                .expect("hash image")
                .as_str()
        )
    );
    let binary = built("dxhello_bin");
    let bytes = std::fs::read(&binary).expect("target binary");
    assert_eq!(machine(&bytes), ELF_MACHINE_AARCH64);
    let library = built("libgreet_shared.so");
    assert_eq!(
        machine(&std::fs::read(&library).expect("target library")),
        ELF_MACHINE_AARCH64
    );
    let files = identity["files"].as_object().expect("files object");
    let packaged = |path: &str| files[path].as_str().expect(path).to_string();
    assert_eq!(
        packaged("usr/bin/dxhello"),
        dx_testing::sha256_file(&binary).expect("hash binary")
    );
    assert_eq!(
        packaged("usr/lib/libgreet_shared.so"),
        dx_testing::sha256_file(&library).expect("hash library")
    );
    let mut names: Vec<&str> = files.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "dxhello.desktop",
            "dxhello.png",
            "usr/bin/dxhello",
            "usr/lib/libgreet_shared.so",
            "usr/share/dxhello/fixtures/greeting.txt",
            "usr/share/licenses/dxhello/NOTICE.txt",
        ]
    );
}
