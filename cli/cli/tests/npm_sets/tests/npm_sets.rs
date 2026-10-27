use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "aspect_rules_js", version = "3.5.2")

pnpm = use_extension("@aspect_rules_js//npm:extensions.bzl", "pnpm")
use_repo(pnpm, "pnpm")
"#;

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "npm"
manifests = ["apps/web/package.json"]
locks = ["apps/web/pnpm-lock.yaml"]
scopes = ["apps/web"]

[[dependency_set]]
name = "worker"
ecosystem = "npm"
manifests = ["services/worker/package.json"]
locks = ["services/worker/pnpm-lock.yaml"]
scopes = ["services/worker"]
"#;

const LIB: &str = "{\n  \"name\": \"shared-lib\",\n  \"version\": \"1.0.0\"\n}\n";

const OTHER: &str = "{\n  \"name\": \"other-lib\",\n  \"version\": \"2.0.0\"\n}\n";

fn package_json(name: &str, deps: &[(&str, &str)]) -> String {
    let deps: Vec<String> = deps
        .iter()
        .map(|(dep, path)| format!("    \"{dep}\": \"file:{path}\""))
        .collect();
    format!(
        "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"dependencies\": {{\n{}\n  }}\n}}\n",
        deps.join(",\n")
    )
}

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/npm_sets/tests/npm_sets.rs");
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
    let root = nested::case("npm-sets");
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
    write(&root.join("libs/shared/package.json"), LIB);
    write(&root.join("libs/other/package.json"), OTHER);
    write(
        &root.join("apps/web/package.json"),
        &package_json("web", &[("shared-lib", "../../libs/shared")]),
    );
    write(
        &root.join("services/worker/package.json"),
        &package_json("worker", &[("other-lib", "../../libs/other")]),
    );
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
fn npm_sets_lock_check_and_update_one_set_at_a_time() {
    let root = consumer();
    let web_lock = root.join("apps/web/pnpm-lock.yaml");
    let worker_lock = root.join("services/worker/pnpm-lock.yaml");

    let run = dx_in(&root, &["update", "--apply"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    for lock in [&web_lock, &worker_lock] {
        assert!(lock.is_file(), "{lock:?} missing\n{}", report(&run));
    }
    let web_text = String::from_utf8(read(&web_lock)).expect("utf-8 lock");
    assert!(web_text.contains("shared-lib"), "{web_text}");
    for dir in ["apps/web", "services/worker"] {
        assert!(!root.join(dir).join("node_modules").exists(), "{dir}");
    }

    let web_before = read(&web_lock);
    let worker_before = read(&worker_lock);
    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert_eq!(read(&web_lock), web_before);
    assert_eq!(read(&worker_lock), worker_before);

    let stale = package_json(
        "web",
        &[
            ("shared-lib", "../../libs/shared"),
            ("other-lib", "../../libs/other"),
        ],
    );
    write(&root.join("apps/web/package.json"), &stale);
    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert_eq!(read(&web_lock), web_before, "--check must not write");

    let home = root.join("hostile-home");
    write(
        &home.join(".npmrc"),
        "lockfile=false\nregistry=http://127.0.0.1:9/\n",
    );
    let home = home.to_string_lossy().into_owned();
    let run = dx_in(
        &root,
        &["update", "--apply", "frontend"],
        &[("HOME", home.as_str())],
    );
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let web_after = String::from_utf8(read(&web_lock)).expect("utf-8 lock");
    assert!(web_after.contains("other-lib"), "{web_after}");
    assert_eq!(read(&worker_lock), worker_before, "worker lock changed");
}
