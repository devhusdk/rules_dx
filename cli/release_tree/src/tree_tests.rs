use super::tree::run;
use std::fs;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let base = std::env::var("TEST_TMPDIR").unwrap_or_default();
    let dir = PathBuf::from(base).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap_or_default();
    dir
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn copies_files_and_merges_directories() {
    let root = scratch("copies");
    fs::create_dir_all(root.join("beams")).unwrap_or_default();
    fs::write(root.join("beams/a.beam"), "a").unwrap_or_default();
    fs::write(root.join("lib.so"), "so").unwrap_or_default();
    let out = root.join("out");
    let result = run(&args(&[
        out.to_str().unwrap_or_default(),
        "--dir",
        "lib/app/priv",
        "--copy",
        root.join("beams").to_str().unwrap_or_default(),
        "lib/app/ebin",
        "--copy",
        root.join("lib.so").to_str().unwrap_or_default(),
        "lib/app/priv/nested/lib.so",
    ]));
    assert_eq!(result, Ok(()));
    assert_eq!(fs::read_to_string(out.join("lib/app/ebin/a.beam")).unwrap_or_default(), "a");
    assert_eq!(fs::read_to_string(out.join("lib/app/priv/nested/lib.so")).unwrap_or_default(), "so");
}

#[test]
fn rejects_paths_outside_the_output() {
    let root = scratch("escape");
    let out = root.join("out");
    let result = run(&args(&[out.to_str().unwrap_or_default(), "--dir", "../x"]));
    assert!(result.is_err());
}

#[test]
fn rejects_unknown_steps() {
    let root = scratch("unknown");
    let out = root.join("out");
    assert!(run(&args(&[out.to_str().unwrap_or_default(), "--bogus"])).is_err());
}
