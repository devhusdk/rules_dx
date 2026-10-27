use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_rust", version = "0.74.0")

crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(
    name = "crates",
    cargo_lockfile = "//apps/rs:Cargo.lock",
    lockfile = "//apps/rs:cargo-bazel-lock.json",
    manifests = ["//apps/rs:Cargo.toml"],
)
crate.from_cargo(
    name = "tool_crates",
    cargo_lockfile = "//tools/rs:Cargo.lock",
    lockfile = "//tools/rs:cargo-bazel-lock.json",
    manifests = ["//tools/rs:Cargo.toml"],
)
use_repo(crate, "crates", "tool_crates")
"#;

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "crates"
ecosystem = "cargo"
manifests = ["apps/rs/Cargo.toml"]
locks = ["apps/rs/Cargo.lock", "apps/rs/cargo-bazel-lock.json"]
scopes = ["apps/rs"]
repository = "crates"

[[dependency_set]]
name = "tools"
ecosystem = "cargo"
manifests = ["tools/rs/Cargo.toml"]
locks = ["tools/rs/Cargo.lock", "tools/rs/cargo-bazel-lock.json"]
scopes = ["tools/rs"]
repository = "tool_crates"
"#;

const SET_BUILD: &str =
    "exports_files([\"Cargo.toml\", \"Cargo.lock\", \"cargo-bazel-lock.json\"])\n";

fn cargo_toml(name: &str, deps: &[(&str, &str)]) -> String {
    let deps: Vec<String> = deps
        .iter()
        .map(|(dep, version)| format!("{dep} = \"={version}\""))
        .collect();
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"lib.rs\"\n\n[dependencies]\n{}\n",
        deps.join("\n")
    )
}

fn cargo_lock(name: &str) -> String {
    format!("version = 4\n\n[[package]]\nname = \"{name}\"\nversion = \"0.1.0\"\n")
}

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/cargo_sets/tests/cargo_sets.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(6)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn consumer() -> Case {
    let root = nested::case("cargo-sets");
    let version = std::fs::read_to_string(rules_dx_root().join(".bazelversion"))
        .unwrap_or_else(|error| panic!(".bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(&root.join("MODULE.bazel"), MODULE);
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join(".bazelrc"),
        "build:dx_dev --compilation_mode=fastbuild\n",
    );
    write(&root.join(".dx/version"), "0.0.0\n");
    write(&root.join("dx.toml"), DX_TOML);
    for (dir, name, dep) in [
        ("apps/rs", "app", ("itoa", "1.0.11")),
        ("tools/rs", "tool", ("itoa", "1.0.11")),
    ] {
        write(&root.join(dir).join("BUILD.bazel"), SET_BUILD);
        write(&root.join(dir).join("lib.rs"), "");
        write(
            &root.join(dir).join("Cargo.toml"),
            &cargo_toml(name, &[dep]),
        );
        write(&root.join(dir).join("Cargo.lock"), &cargo_lock(name));
        write(&root.join(dir).join("cargo-bazel-lock.json"), "{}\n");
    }
    root
}

fn dx_in(root: &Case, args: &[&str], env: &[(&str, &str)]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    root.run(&dx(), &argv, env)
}

fn report(run: &Run) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        run.stdout,
        run.stderr
    )
}

#[test]
fn cargo_sets_check_and_repin_one_crate_universe_hub_at_a_time() {
    let root = consumer();
    let app_lock = root.join("apps/rs/Cargo.lock");
    let app_json = root.join("apps/rs/cargo-bazel-lock.json");
    let tool_lock = root.join("tools/rs/Cargo.lock");
    let tool_json = root.join("tools/rs/cargo-bazel-lock.json");

    let first = dx_in(&root, &["update", "--apply"], &[]);
    let run = dx_in(&root, &["update", "--apply"], &[]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}\n{}",
        report(&first),
        report(&run)
    );
    for lock in [&app_lock, &tool_lock] {
        let text = String::from_utf8(read(lock)).expect("utf-8 lock");
        assert!(text.contains("name = \"itoa\""), "{lock:?}: {text}");
    }

    let before: Vec<Vec<u8>> = [&app_lock, &app_json, &tool_lock, &tool_json]
        .iter()
        .map(|path| read(path))
        .collect();
    let run = dx_in(&root, &["update", "--check"], &[("REPIN", "1")]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        run.stdout.contains("crates lockfile current"),
        "{}",
        report(&run)
    );
    assert!(
        run.stdout.contains("tools lockfile current"),
        "{}",
        report(&run)
    );
    let after: Vec<Vec<u8>> = [&app_lock, &app_json, &tool_lock, &tool_json]
        .iter()
        .map(|path| read(path))
        .collect();
    assert_eq!(before, after, "--check must not write");

    write(
        &root.join("apps/rs/Cargo.toml"),
        &cargo_toml("app", &[("itoa", "1.0.11"), ("ryu", "1.0.18")]),
    );
    let run = dx_in(&root, &["update", "--check", "crates"], &[]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        !run.stdout.contains("crates lockfile current"),
        "{}",
        report(&run)
    );
    assert_eq!(read(&app_lock), before[0], "--check must not write");
    assert_eq!(read(&app_json), before[1], "--check must not write");

    let run = dx_in(&root, &["update", "--apply", "crates"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let app_after = String::from_utf8(read(&app_lock)).expect("utf-8 lock");
    assert!(app_after.contains("name = \"ryu\""), "{app_after}");
    assert_ne!(read(&app_json), before[1], "cargo-bazel lock not repinned");
    assert_eq!(read(&tool_lock), before[2], "tools Cargo.lock changed");
    assert_eq!(
        read(&tool_json),
        before[3],
        "tools cargo-bazel lock changed"
    );

    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
}

fn locked_version(lock: &Path, name: &str) -> String {
    let text = String::from_utf8(read(lock)).expect("utf-8 lock");
    let needle = format!("name = \"{name}\"\nversion = \"");
    let start = text.find(&needle).unwrap_or_else(|| panic!("{name} in {text}")) + needle.len();
    text[start..].split('"').next().expect("version").to_owned()
}

#[test]
fn cargo_set_package_repin_moves_one_crate() {
    let root = consumer();
    let app_lock = root.join("apps/rs/Cargo.lock");
    let app_json = root.join("apps/rs/cargo-bazel-lock.json");
    let tool_lock = root.join("tools/rs/Cargo.lock");

    write(
        &root.join("apps/rs/Cargo.toml"),
        &cargo_toml("app", &[("itoa", "1.0.10"), ("ryu", "1.0.17")]),
    );
    let run = dx_in(&root, &["update", "--apply", "crates"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert_eq!(locked_version(&app_lock, "itoa"), "1.0.10");
    assert_eq!(locked_version(&app_lock, "ryu"), "1.0.17");

    let relaxed = cargo_toml("app", &[("itoa", "1.0.10"), ("ryu", "1.0.17")]).replace("\"=", "\"");
    write(&root.join("apps/rs/Cargo.toml"), &relaxed);
    let json_before = read(&app_json);
    let tool_before = read(&tool_lock);

    let run = dx_in(&root, &["update", "--apply", "crates:itoa"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert_ne!(locked_version(&app_lock, "itoa"), "1.0.10");
    assert_eq!(locked_version(&app_lock, "ryu"), "1.0.17");
    assert_ne!(read(&app_json), json_before, "cargo-bazel lock not repinned");
    assert_eq!(read(&tool_lock), tool_before, "tools Cargo.lock changed");
    let run = dx_in(&root, &["update", "--check", "crates"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));

    let lock_before = read(&app_lock);
    let json_before = read(&app_json);
    let run = dx_in(&root, &["update", "--apply", "crates:nonexistent_crate"], &[]);
    assert_eq!(run.status.code(), Some(1), "{}", report(&run));
    assert_eq!(read(&app_lock), lock_before, "failed repin wrote Cargo.lock");
    assert_eq!(read(&app_json), json_before, "failed repin wrote cargo-bazel lock");
}
