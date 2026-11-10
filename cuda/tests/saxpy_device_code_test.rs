use dx_testing::fatbin::{self, Entry, Kind};

fn expected() -> Vec<Entry> {
    vec![
        Entry {
            kind: Kind::Ptx,
            arch: 89,
        },
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

#[test]
fn archive_holds_sass_for_each_sm_and_ptx_for_each_compute() {
    let archives = std::env::var("DX_CUDA_ARCHIVES").unwrap();
    let mut checked = 0;
    for path in archives
        .split_whitespace()
        .filter(|p| p.ends_with(".a") || p.ends_with(".lib"))
    {
        let bytes = dx_testing::read_runfile(path);
        let members: Vec<_> = fatbin::archive_members(&bytes)
            .unwrap()
            .iter()
            .map(|(name, data)| (name.clone(), data.len(), data.get(..8).map(<[u8]>::to_vec)))
            .collect();
        let entries = fatbin::device_code(&bytes).unwrap();
        assert_eq!(sorted(entries), sorted(expected()), "{path}: {members:?}");
        checked += 1;
    }
    assert!(checked > 0, "no static archive in {archives}");
}

#[test]
fn binary_holds_device_code_and_links_the_static_runtime() {
    let binary = dx_testing::read_runfile(&std::env::var("DX_CUDA_BINARY").unwrap());
    assert_eq!(
        sorted(fatbin::device_code(&binary).unwrap()),
        sorted(expected())
    );
    let needed = fatbin::needed_libraries(&binary).unwrap();
    assert!(!needed.is_empty(), "the binary links libc dynamically");
    assert!(
        needed
            .iter()
            .all(|lib| !lib.to_ascii_lowercase().contains("cudart")),
        "static runtime must not need libcudart: {needed:?}"
    );
}
