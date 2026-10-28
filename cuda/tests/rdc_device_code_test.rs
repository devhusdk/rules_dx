use dx_testing::fatbin::{self, Entry, Kind};

fn runfile(path: &str) -> Vec<u8> {
    let resolved = dx_testing::resolve_runfiles(path);
    std::fs::read(&resolved).unwrap_or_else(|e| panic!("read {}: {e}", resolved.display()))
}

fn expected() -> Vec<Entry> {
    vec![
        Entry {
            kind: Kind::Elf,
            arch: 89,
        },
        Entry {
            kind: Kind::Elf,
            arch: 120,
        },
    ]
}

fn sorted(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort();
    entries.dedup();
    entries
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn archives_hold_relocatable_sass_for_each_sm() {
    let archives = std::env::var("DX_CUDA_ARCHIVES").unwrap();
    let mut checked = 0;
    for path in archives.split_whitespace().filter(|p| p.ends_with(".a")) {
        let entries = fatbin::device_code(&runfile(path)).unwrap();
        assert_eq!(sorted(entries), sorted(expected()), "{path}");
        checked += 1;
    }
    assert!(checked > 0, "no static archive in {archives}");
}

#[test]
fn binary_is_device_linked() {
    let binary = runfile(&std::env::var("DX_CUDA_BINARY").unwrap());
    assert_eq!(
        sorted(fatbin::device_code(&binary).unwrap()),
        sorted(expected())
    );
    assert!(
        contains(&binary, b"__cudaRegisterLinkedBinary"),
        "binary has no device-link registration"
    );
}
