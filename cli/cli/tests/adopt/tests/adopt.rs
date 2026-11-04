#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_test_scratch::{scratch, TempDir};
use dx_testing::resolve_runfiles;

struct Out {
    code: i32,
    stderr: String,
}

fn dx(root: &Path, home: &Path, args: &[&str]) -> Out {
    let bin = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must be set"));
    let output = Command::new(bin)
        .args(args)
        .arg("--workspace")
        .arg(root)
        .current_dir(root)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("GIT_CEILING_DIRECTORIES", home.parent().expect("base"))
        .output()
        .unwrap_or_else(|error| panic!("spawn dx: {error}"));
    Out {
        code: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn tree(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            tree(&path, root, out);
        } else {
            let rel = path.strip_prefix(root).expect("under root").to_path_buf();
            out.insert(rel, std::fs::read(&path).expect("read"));
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    tree(root, root, &mut out);
    out
}

fn setup(name: &str) -> (TempDir, PathBuf, PathBuf) {
    let guard = scratch(name);
    let base = guard.path().to_path_buf();
    let home = base.join("home");
    let root = base.join("app");
    std::fs::create_dir_all(&home).expect("home");
    std::fs::create_dir_all(&root).expect("root");
    (guard, root, home)
}

fn init(root: &Path, home: &Path) {
    let out = dx(root, home, &["init", "--apply"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
}

#[test]
fn init_in_an_empty_dir() {
    let (_guard, root, home) = setup("adopt-empty");
    init(&root, &home);
    for file in [
        ".dx/version",
        "tools/dx",
        "tools/dx.cmd",
        "MODULE.bazel",
        ".bazelversion",
        "AGENTS.md",
        ".github/workflows/ci.yml",
    ] {
        assert!(root.join(file).is_file(), "missing {file}");
    }
    assert!(!root.join("dx.toml").exists(), "init wrote dx.toml");
    let check = dx(&root, &home, &["init"]);
    assert_eq!(check.code, 0, "{}", check.stderr);
    let before = snapshot(&root);
    init(&root, &home);
    assert_eq!(before, snapshot(&root));
}

#[test]
fn init_in_an_existing_module() {
    let (_guard, root, home) = setup("adopt-existing");
    let module = "module(name = \"app\")\n";
    write(&root.join("MODULE.bazel"), module);
    write(&root.join("BUILD.bazel"), "");
    init(&root, &home);
    assert_eq!(read(&root.join("MODULE.bazel")), module);
    assert!(!root.join("dx.toml").exists(), "init wrote dx.toml");
    let check = dx(&root, &home, &["init"]);
    assert_eq!(check.code, 0, "{}", check.stderr);
}

#[test]
fn upgrade_moves_an_older_pin_to_this_dx() {
    let (_guard, root, home) = setup("adopt-upgrade");
    init(&root, &home);
    let version_file = root.join(".dx/version");
    let built = read(&version_file).trim().to_string();
    let launchers = [root.join("tools/dx"), root.join("tools/dx.cmd")];
    let fresh: Vec<String> = launchers.iter().map(|path| read(path)).collect();
    write(&version_file, "0.0.1\n");
    for (path, text) in launchers.iter().zip(&fresh) {
        write(path, &text.replace(&built, "0.0.1"));
    }
    let check = dx(&root, &home, &["upgrade", "--to", &built]);
    assert_eq!(check.code, 1, "{}", check.stderr);
    assert!(
        check.stderr.contains("status_pin_mismatch"),
        "{}",
        check.stderr
    );
    let apply = dx(&root, &home, &["upgrade", "--to", &built, "--apply"]);
    assert_eq!(apply.code, 0, "{}", apply.stderr);
    assert_eq!(read(&version_file).trim(), built);
    for (path, text) in launchers.iter().zip(&fresh) {
        assert_eq!(&read(path), text);
    }
}
