use dx_testing::fatbin::{self, Entry, Kind};

fn runfile(path: &str) -> Vec<u8> {
    let resolved = dx_testing::resolve_runfiles(path);
    std::fs::read(&resolved).unwrap_or_else(|e| panic!("read {}: {e}", resolved.display()))
}

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
    for path in archives.split_whitespace().filter(|p| p.ends_with(".a")) {
        let entries = fatbin::device_code(&runfile(path)).unwrap();
        assert_eq!(sorted(entries), sorted(expected()), "{path}");
        checked += 1;
    }
    assert!(checked > 0, "no static archive in {archives}");
}

#[test]
fn binary_holds_device_code_and_links_cublas_without_libcudart() {
    let binary = runfile(&std::env::var("DX_CUDA_BINARY").unwrap());
    assert_eq!(
        sorted(fatbin::device_code(&binary).unwrap()),
        sorted(expected())
    );
    let needed = fatbin::needed_libraries(&binary).unwrap();
    assert!(!needed.is_empty(), "the binary links libc dynamically");
    assert!(
        needed.iter().all(|lib| !lib.starts_with("libcudart")),
        "static runtime must not need libcudart: {needed:?}"
    );
    for soname in ["libcublas.so.13", "libcublasLt.so.13"] {
        assert!(
            needed.iter().any(|lib| lib == soname),
            "{soname} must be linked from the pinned component: {needed:?}"
        );
    }
}
