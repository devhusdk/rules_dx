use dx_testing::fatbin::{self, Entry, Kind, sorted};

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

#[test]
fn archives_hold_relocatable_sass_for_each_sm() {
    let archives = std::env::var("DX_CUDA_ARCHIVES").unwrap();
    let mut checked = 0;
    for path in archives
        .split_whitespace()
        .filter(|p| p.ends_with(".a") || p.ends_with(".lib"))
    {
        let entries = fatbin::device_code(&dx_testing::read_runfile(path)).unwrap();
        assert_eq!(sorted(entries), sorted(expected()), "{path}");
        checked += 1;
    }
    assert!(checked > 0, "no static archive in {archives}");
}

#[test]
fn binary_is_device_linked() {
    let binary = dx_testing::read_runfile(&std::env::var("DX_CUDA_BINARY").unwrap());
    assert_eq!(
        sorted(fatbin::device_code(&binary).unwrap()),
        sorted(expected())
    );
    // PE files carry no symbol names for the linked-binary registration.
    assert!(
        cfg!(windows) || dx_testing::contains_bytes(&binary, b"__cudaRegisterLinkedBinary"),
        "binary has no device-link registration"
    );
}
