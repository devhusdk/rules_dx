use dx_testing::fatbin::{self, Entry, Kind};

fn runfile(path: &str) -> Vec<u8> {
    let resolved = dx_testing::resolve_runfiles(path);
    std::fs::read(&resolved).unwrap_or_else(|e| panic!("read {}: {e}", resolved.display()))
}

/// `compute_80:compute_80,sm_80` gives PTX and SASS for 8.0; `sm_90` gives SASS only.
fn expected() -> Vec<Entry> {
    vec![
        Entry {
            kind: Kind::Ptx,
            arch: 80,
        },
        Entry {
            kind: Kind::Elf,
            arch: 80,
        },
        Entry {
            kind: Kind::Elf,
            arch: 90,
        },
    ]
}

fn sorted(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort();
    entries.dedup();
    entries
}

#[test]
fn kernels_archive_holds_device_code_for_each_arch() {
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
fn app_holds_device_code_and_links_the_static_runtime() {
    let binary = runfile(&std::env::var("DX_CUDA_BINARY").unwrap());
    assert_eq!(
        sorted(fatbin::device_code(&binary).unwrap()),
        sorted(expected())
    );
    let needed = fatbin::needed_libraries(&binary).unwrap();
    assert!(!needed.is_empty(), "the app links libc dynamically");
    assert!(
        needed.iter().all(|lib| !lib.starts_with("libcudart")),
        "static runtime must not need libcudart: {needed:?}"
    );
}
