use std::path::PathBuf;

fn read(name: &str) -> String {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn quoted_after<'a>(text: &'a str, key: &str) -> &'a str {
    let start = text.find(key).unwrap_or_else(|| panic!("missing {key:?}")) + key.len();
    let rest = &text[start..];
    let open = rest
        .find('"')
        .unwrap_or_else(|| panic!("no value after {key:?}"))
        + 1;
    let close = rest[open..]
        .find('"')
        .unwrap_or_else(|| panic!("unterminated value after {key:?}"));
    &rest[open..open + close]
}

fn repo_block<'a>(module: &'a str, repo: &str) -> &'a str {
    let marker = format!("name = \"{repo}\"");
    let start = module
        .find(&marker)
        .unwrap_or_else(|| panic!("MODULE.bazel has no {repo}"));
    let end = module[start..]
        .find("\n)")
        .unwrap_or_else(|| panic!("unterminated {repo}"));
    &module[start..start + end]
}

fn urls(block: &str) -> Vec<&str> {
    let start = block.find("urls = [").expect("block has no urls list") + "urls = [".len();
    let end = block[start..].find(']').expect("unterminated urls list");
    block[start..start + end]
        .split('"')
        .skip(1)
        .step_by(2)
        .collect()
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

fn requirements_pin<'a>(psd1: &'a str, name: &str) -> &'a str {
    psd1_entries(psd1)
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("psd1 has no {name}"))
        .1
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

fn check(lock: &str, module: &str, psd1: &str, name: &str) {
    let repo = format!("psgallery_{}", name.to_lowercase());
    let entry_start = lock
        .find(&format!("\"{name}\": {{"))
        .unwrap_or_else(|| panic!("lock has no {name}"));
    let entry = &lock[entry_start..];
    let entry = &entry[..entry.find('}').unwrap_or(entry.len())];
    let version = quoted_after(entry, "\"version\":");
    let sha256 = quoted_after(entry, "\"sha256\":");
    let block = repo_block(module, &repo);
    assert_eq!(
        quoted_after(block, "sha256 ="),
        sha256,
        "{name} sha256 differs between MODULE.bazel and PSGallery.lock.json"
    );
    let urls = urls(block);
    assert!(!urls.is_empty(), "{name} has no urls in MODULE.bazel");
    for url in urls {
        assert!(
            url.contains(&format!("/{version}")),
            "{name} url {url} is not version {version} from the lock"
        );
    }
    assert_eq!(
        requirements_pin(psd1, name),
        version,
        "{name} version differs between PSGallery.requirements.psd1 and PSGallery.lock.json"
    );
}

#[test]
fn the_gallery_lock_names_the_bytes_that_module_bazel_fetches() {
    let lock = read("DX_GALLERY_LOCK");
    let module = read("DX_MODULE_BAZEL");
    let psd1 = read("DX_GALLERY_REQUIREMENTS");
    let names = lock_modules(&lock);
    assert!(!names.is_empty(), "lock lists no modules");
    for name in names {
        check(&lock, &module, &psd1, name);
    }
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
