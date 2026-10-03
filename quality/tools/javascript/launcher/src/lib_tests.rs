//! Tests for reading a generated launcher's runfiles.

use std::path::{Path, PathBuf};

use super::{bin_dir, entry_point, read_manifest};

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(&path, body).expect("write");
    path
}

#[test]
fn a_manifest_reads_into_key_and_path() {
    let dir = temp("manifest");
    let path = write(
        &dir,
        "MANIFEST",
        "_main/a/b.txt C:/out/a/b.txt\n_main/c C:/out/c\n",
    );
    let read = read_manifest(&path).expect("manifest");
    assert_eq!(read.len(), 2);
    assert_eq!(read["_main/a/b.txt"], PathBuf::from("C:/out/a/b.txt"));
    cleanup(&dir);
}

#[test]
fn an_indented_line_is_not_a_manifest_entry() {
    let dir = temp("indented");
    let path = write(&dir, "MANIFEST", "_main/a C:/a\n  indented\n");
    let read = read_manifest(&path).expect("manifest");
    assert_eq!(read.len(), 1);
    cleanup(&dir);
}

#[test]
fn the_entry_point_comes_from_the_named_line() {
    let dir = temp("entry");
    let body = "head\n    entry_point=$(resolve_execroot_bin_path \"pkg/bin/tool.cjs\")\ntail\n";
    let path = write(&dir, "gen", body);
    assert_eq!(entry_point(&path).expect("entry"), "pkg/bin/tool.cjs");
    cleanup(&dir);
}

#[test]
fn a_script_without_an_entry_point_says_so() {
    let dir = temp("missing");
    let path = write(&dir, "gen", "head\nnothing here\n");
    assert!(entry_point(&path).is_err());
    cleanup(&dir);
}

#[test]
fn the_bin_directory_sits_above_the_generated_directories() {
    let script = Path::new("/out/bin/pkg/bin/tool");
    assert_eq!(bin_dir(script), Some(PathBuf::from("/out/bin")));
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dx-js-launcher-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
