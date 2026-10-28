use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_go", version = "0.64.2")

go_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")
go_sdk.download(version = "1.24.0")
"#;

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "gomods"
ecosystem = "go"
manifests = ["third_party/go/go.mod"]
locks = ["third_party/go/go.sum"]
scopes = ["third_party/go"]
"#;

const GO_MOD: &str =
    "module example.com/consumer\n\ngo 1.24\n\nrequire github.com/google/go-cmp v0.7.0\n";

/// Only the `go.mod` hash: the module zip line is what `go mod download all` adds.
const STALE_SUM: &str =
    "github.com/google/go-cmp v0.7.0/go.mod h1:pXiqmnSA92OHEEa9HXL2W4E7lf9JzCmGVUdgjX3N/iU=\n";

const FULL_SUM: &str = "github.com/google/go-cmp v0.7.0 h1:wk8382ETsv4JYUZwIsn6YpYiWiBsYLSJiTsyBybVuN8=\ngithub.com/google/go-cmp v0.7.0/go.mod h1:pXiqmnSA92OHEEa9HXL2W4E7lf9JzCmGVUdgjX3N/iU=\n";

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/go_sets/tests/go_sets.rs");
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

fn consumer() -> Case {
    let root = nested::case("go-sets");
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
    write(&root.join("third_party/go/go.mod"), GO_MOD);
    write(&root.join("third_party/go/go.sum"), STALE_SUM);
    root
}

/// Every file below `root` outside Bazel's and dx's own state, with its bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("{dir:?}: {error}")) {
            let entry = entry.expect("entry");
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            // Bazel itself may write its module lockfile; dx never does.
            if name.starts_with("bazel-") || name == ".dx" || name == "MODULE.bazel.lock" {
                continue;
            }
            let kind = entry.file_type().expect("type");
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                let rel = path.strip_prefix(root).expect("below root").to_path_buf();
                out.insert(rel, std::fs::read(&path).expect("read"));
            }
        }
    }
    out
}

fn dx_in(root: &Case, args: &[&str]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    root.run(&dx(), &argv, &[])
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
fn go_set_adds_only_the_missing_go_sum_lines() {
    let root = consumer();
    let before = snapshot(&root);

    let run = dx_in(&root, &["update", "--check", "gomods"]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(!run.stdout.contains("lockfile current"), "{}", report(&run));
    assert!(
        format!("{}{}", run.stdout, run.stderr).contains("stale generated locks"),
        "{}",
        report(&run)
    );
    assert_eq!(snapshot(&root), before, "--check wrote");

    let run = dx_in(&root, &["update", "--apply", "gomods"]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let after = snapshot(&root);
    let changed: Vec<&PathBuf> = after
        .iter()
        .filter(|(path, bytes)| before.get(*path) != Some(*bytes))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(changed, [Path::new("third_party/go/go.sum")], "{after:?}");
    assert_eq!(
        String::from_utf8_lossy(&after[Path::new("third_party/go/go.sum")]),
        FULL_SUM
    );
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );

    let run = dx_in(&root, &["update", "--check", "gomods"]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        run.stdout.contains("gomods lockfile current"),
        "{}",
        report(&run)
    );

    let run = dx_in(&root, &["update", "--check", "--offline", "gomods"]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        format!("{}{}", run.stdout, run.stderr).contains("offline_required"),
        "{}",
        report(&run)
    );

    // A real bump: an older require with stale sums is widened and go.sum is refreshed.
    write(
        &root.join("third_party/go/go.mod"),
        &GO_MOD.replace("v0.7.0", "v0.6.0"),
    );
    write(&root.join("third_party/go/go.sum"), STALE_SUM);
    let before = snapshot(&root);
    let run = dx_in(
        &root,
        &[
            "bump",
            "gomods:github.com/google/go-cmp",
            "v0.7.0",
            "--apply",
        ],
    );
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let after = snapshot(&root);
    let changed: Vec<&PathBuf> = after
        .iter()
        .filter(|(path, bytes)| before.get(*path) != Some(*bytes))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        changed,
        [
            Path::new("third_party/go/go.mod"),
            Path::new("third_party/go/go.sum")
        ],
        "{after:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&after[Path::new("third_party/go/go.mod")]),
        GO_MOD
    );
    assert_eq!(
        String::from_utf8_lossy(&after[Path::new("third_party/go/go.sum")]),
        FULL_SUM
    );
}
