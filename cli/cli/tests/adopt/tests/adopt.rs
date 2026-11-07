#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_test_scratch::{TempDir, scratch};
use dx_testing::resolve_runfiles;

struct Out {
    code: i32,
    stderr: String,
}

fn dx(root: &Path, home: &Path, args: &[&str]) -> Out {
    dx_with_env(root, home, args, &[])
}

fn dx_with_env(root: &Path, home: &Path, args: &[&str], env: &[(&str, &str)]) -> Out {
    let bin = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must be set"));
    let output = Command::new(bin)
        .args(args)
        .arg("--workspace")
        .arg(root)
        .current_dir(root)
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("GIT_CEILING_DIRECTORIES", home.parent().expect("base"))
        .envs(env.iter().copied())
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
        ".dx/pins",
        "tools/dx",
        "tools/dx.cmd",
        "MODULE.bazel",
        ".bazelversion",
        "AGENTS.md",
        ".github/workflows/ci.yml",
    ] {
        assert!(root.join(file).is_file(), "missing {file}");
    }
    for file in [
        "dx.toml",
        "security.toml",
        "licenses.toml",
        "ruff.toml",
        "biome.json",
        "rustfmt.toml",
        "eslint.config.js",
    ] {
        assert!(!root.join(file).exists(), "init wrote {file}");
    }
    let check = dx(&root, &home, &["init"]);
    assert_eq!(check.code, 0, "{}", check.stderr);
    let before = snapshot(&root);
    init(&root, &home);
    assert_eq!(before, snapshot(&root));
}

#[test]
fn init_rejects_profile() {
    let (_guard, root, home) = setup("adopt-profile");
    let out = dx(&root, &home, &["init", "--apply", "--profile=strict"]);
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("--profile"), "{}", out.stderr);
    assert!(!root.join("MODULE.bazel").exists());
}

#[test]
fn init_apply_works_with_the_agent_json_default() {
    let (_guard, root, home) = setup("adopt-agent");
    let agent = [("CLAUDECODE", "1")];
    let out = dx_with_env(&root, &home, &["init", "--apply"], &agent);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(root.join("MODULE.bazel").is_file());
    let json = dx_with_env(&root, &home, &["init", "--apply", "--output=json"], &agent);
    assert_eq!(json.code, 2, "{}", json.stderr);
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
    let version_file = root.join(".dx/pins");
    let built = read(&version_file).trim().to_string();
    let launchers = [root.join("tools/dx"), root.join("tools/dx.cmd")];
    let fresh: Vec<String> = launchers.iter().map(|path| read(path)).collect();
    write(&version_file, "0.0.1\n");
    let module_file = root.join("MODULE.bazel");
    write(&module_file, &read(&module_file).replace(&built, "0.0.1"));
    for (path, text) in launchers.iter().zip(&fresh) {
        write(path, &text.replace(&built, "0.0.1"));
    }
    let check = dx(&root, &home, &["upgrade", "--to", &built]);
    assert_eq!(check.code, 1, "{}", check.stderr);
    assert!(check.stderr.contains("upgrade_pending"), "{}", check.stderr);
    let kept = || {
        let mut files = BTreeMap::new();
        tree(&root.join(".dx/bazelrc"), &root, &mut files);
        for rel in [".bazelrc", ".gitignore"] {
            files.insert(
                PathBuf::from(rel),
                std::fs::read(root.join(rel)).expect(rel),
            );
        }
        files
    };
    let before = kept();
    assert!(before.len() >= 5, "{:?}", before.keys());
    let apply = dx(&root, &home, &["upgrade", "--to", &built, "--apply"]);
    assert_eq!(apply.code, 0, "{}", apply.stderr);
    assert_eq!(read(&version_file).trim(), built);
    assert_eq!(before, kept());
    for (path, text) in launchers.iter().zip(&fresh) {
        assert_eq!(&read(path), text);
    }
}

#[test]
fn fresh_clone_regenerates_dx_on_first_command() {
    let (_guard, root, home) = setup("adopt-fresh-clone");
    init(&root, &home);
    let dot_dx = root.join(".dx");
    let generated = || {
        let mut files = BTreeMap::new();
        tree(&dot_dx, &root, &mut files);
        files.retain(|path, _| !path.starts_with(".dx/local") && path != Path::new(".dx/pins"));
        files
    };
    let before = generated();
    assert!(
        before.contains_key(Path::new(".dx/BUILD.bazel")),
        "{:?}",
        before.keys()
    );
    for entry in std::fs::read_dir(&dot_dx).expect("read .dx") {
        let path = entry.expect("entry").path();
        if path.file_name().is_some_and(|name| name == "pins") {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path).expect("remove dir");
        } else {
            std::fs::remove_file(&path).expect("remove file");
        }
    }
    let out = dx(&root, &home, &["completion", "bash"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(before, generated());
}

#[test]
fn init_leaves_polyglot_locks_untouched_and_is_idempotent() {
    let (_guard, root, home) = setup("adopt-polyglot");
    let names = [
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "pnpm-lock.yaml",
        "pyproject.toml",
        "uv.lock",
    ];
    let mut originals = BTreeMap::new();
    for name in names {
        let rel = std::env::var(format!(
            "DX_POLYGLOT_{}",
            name.replace(['.', '-'], "_").to_uppercase()
        ))
        .unwrap_or_else(|_| panic!("missing path for {name}"));
        let text = std::fs::read(resolve_runfiles(&rel)).expect("read fixture");
        std::fs::write(root.join(name), &text).expect("stage fixture");
        originals.insert(name, text);
    }
    init(&root, &home);
    for (name, text) in &originals {
        assert_eq!(
            &std::fs::read(root.join(name)).expect("read"),
            text,
            "init changed {name}"
        );
    }
    let before = snapshot(&root);
    init(&root, &home);
    assert_eq!(before, snapshot(&root));
    let check = dx(&root, &home, &["init"]);
    assert_eq!(check.code, 0, "{}", check.stderr);
}
