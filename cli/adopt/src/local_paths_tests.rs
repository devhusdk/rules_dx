use std::path::{Path, PathBuf};

const ENTRIES: [&str; 17] = [
    "setups",
    "current",
    "environments",
    "generated",
    "bin",
    "leases",
    "results.json",
    "explain",
    "docs",
    "profile",
    "timings",
    "bench",
    "advisory",
    "fuzz",
    "ide",
    "launcher",
    ".commit.lock",
];

fn src_dir() -> PathBuf {
    match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(root) => Path::new(&root).join("src"),
        Err(_) => PathBuf::from("src"),
    }
}

#[test]
fn machine_local_paths_live_under_dx_local() {
    let dir = src_dir();
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("src dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read source");
        checked += 1;
        for (number, line) in text.lines().enumerate() {
            for name in ENTRIES {
                let old = format!(".dx/{name}");
                assert!(
                    !line.contains(&old),
                    "{}:{} names {old}; machine-local state lives under .dx/local",
                    path.display(),
                    number + 1
                );
            }
        }
    }
    assert!(
        checked > 10,
        "found only {checked} sources in {}",
        dir.display()
    );
}
