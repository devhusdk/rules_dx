use std::path::PathBuf;

fn read(name: &str) -> String {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

/// Reads `<Name> = '<ver>'` lines of the psd1, skipping comments.
fn psd1_entries(psd1: &str) -> Vec<(&str, &str)> {
    psd1.lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once(" = '")?;
            let ident =
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            let version = rest.strip_suffix('\'')?;
            ident.then_some((name, version))
        })
        .collect()
}

/// Module names are the keys of the lock's `"modules"` object.
fn lock_modules(lock: &str) -> Vec<&str> {
    let start = lock.find("\"modules\"").expect("lock has no modules");
    lock[start..]
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (key, value) = line.trim().split_once("\": ")?;
            value.starts_with('{').then(|| key.trim_start_matches('"'))
        })
        .collect()
}

#[test]
fn the_requirements_file_and_the_lock_list_the_same_modules() {
    let lock = read("DX_GALLERY_LOCK");
    let psd1 = read("DX_GALLERY_REQUIREMENTS");
    let mut from_lock = lock_modules(&lock);
    let mut from_psd1: Vec<&str> = psd1_entries(&psd1).into_iter().map(|(n, _)| n).collect();
    from_lock.sort_unstable();
    from_psd1.sort_unstable();
    assert_eq!(from_psd1, from_lock, "psd1 and lock module sets differ");
}
