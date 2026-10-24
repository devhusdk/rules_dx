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

fn check(lock: &str, module: &str, name: &str, repo: &str) {
    let entry_start = lock
        .find(&format!("\"{name}\": {{"))
        .unwrap_or_else(|| panic!("lock has no {name}"));
    let entry = &lock[entry_start..];
    let entry = &entry[..entry.find('}').unwrap_or(entry.len())];
    let version = quoted_after(entry, "\"version\":");
    let sha256 = quoted_after(entry, "\"sha256\":");
    let block = repo_block(module, repo);
    assert_eq!(
        quoted_after(block, "sha256 ="),
        sha256,
        "{name} sha256 differs between MODULE.bazel and PSGallery.lock.json"
    );
    assert!(
        block.contains(&format!("/{version}\"")),
        "{name} version {version} is not the one MODULE.bazel downloads"
    );
}

#[test]
fn the_gallery_lock_names_the_bytes_that_module_bazel_fetches() {
    let lock = read("DX_GALLERY_LOCK");
    let module = read("DX_MODULE_BAZEL");
    check(&lock, &module, "Pester", "psgallery_pester");
    check(
        &lock,
        &module,
        "PSScriptAnalyzer",
        "psgallery_psscriptanalyzer",
    );
}
