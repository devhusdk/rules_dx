use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dotnet", version = "0.22.2")

single_version_override(
    module_name = "rules_dotnet",
    patch_strip = 1,
    patches = ["//patches/rules_dotnet:sdk_versions.patch"],
    version = "0.22.2",
)

dotnet = use_extension("@rules_dotnet//dotnet:extensions.bzl", "dotnet")
dotnet.toolchain(dotnet_version = "10.0.403")
use_repo(dotnet, "dotnet_toolchains")
"#;

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "dotnet"
ecosystem = "nuget"
manifests = ["third_party/dotnet/paket.dependencies"]
locks = [
  "third_party/dotnet/paket.lock",
  "third_party/dotnet/deps/paket.main.bzl",
  "third_party/dotnet/deps/paket.main_extension.bzl",
]
scopes = ["third_party/dotnet"]
"#;

const DEPENDENCIES: &str = "source https://api.nuget.org/v3/index.json\nframework: net10.0\n\nnuget Newtonsoft.Json 13.0.3\n";

const LOCK: &str = "STORAGE: NONE\nRESTRICTION: == net10.0\nNUGET\n  remote: https://api.nuget.org/v3/index.json\n    Newtonsoft.Json (13.0.3)\n";

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/nuget_sets/tests/nuget_sets.rs");
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

/// Seeds the SDK patch the way `dx init` does; overrides only apply in the root module.
fn stage_dotnet_patch(root: &Path, rules: &Path) {
    let patch = std::fs::read_to_string(rules.join("patches/rules_dotnet/sdk_versions.patch"))
        .unwrap_or_else(|error| panic!("sdk_versions.patch: {error}"));
    write(
        &root.join("patches/rules_dotnet/sdk_versions.patch"),
        &patch,
    );
    write(
        &root.join("patches/rules_dotnet/BUILD.bazel"),
        "exports_files([\"sdk_versions.patch\"])\n",
    );
}

fn consumer() -> Case {
    let root = nested::case("nuget-sets");
    let version = std::fs::read_to_string(rules_dx_root().join(".bazelversion"))
        .unwrap_or_else(|error| panic!(".bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(&root.join("MODULE.bazel"), MODULE);
    stage_dotnet_patch(&root, &rules_dx_root());
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join(".bazelrc"),
        "build:dx_dev --compilation_mode=fastbuild\n",
    );
    write(&root.join(".dx/version"), "0.0.0\n");
    write(&root.join("dx.toml"), DX_TOML);
    write(
        &root.join("third_party/dotnet/paket.dependencies"),
        DEPENDENCIES,
    );
    write(&root.join("third_party/dotnet/paket.lock"), LOCK);
    write(
        &root.join("third_party/dotnet/deps/paket.main.bzl"),
        "stale\n",
    );
    write(
        &root.join("third_party/dotnet/deps/paket.main_extension.bzl"),
        "stale\n",
    );
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
fn nuget_set_regenerates_only_its_declared_bzl_files() {
    let root = consumer();
    let before = snapshot(&root);

    let run = dx_in(&root, &["update", "--check"]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(!run.stdout.contains("lockfile current"), "{}", report(&run));
    let checked = snapshot(&root);
    let touched: Vec<&PathBuf> = checked
        .keys()
        .chain(before.keys())
        .filter(|path| checked.get(*path) != before.get(*path))
        .collect();
    assert!(touched.is_empty(), "--check wrote {touched:?}");

    let run = dx_in(&root, &["update", "--apply"]);
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
            Path::new("third_party/dotnet/deps/paket.main.bzl"),
            Path::new("third_party/dotnet/deps/paket.main_extension.bzl"),
        ],
        "{after:?}"
    );
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    let bzl = String::from_utf8(after[Path::new("third_party/dotnet/deps/paket.main.bzl")].clone())
        .expect("utf-8");
    assert!(bzl.contains("Newtonsoft.Json"), "{bzl}");

    let run = dx_in(&root, &["update", "--check"]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        run.stdout.contains("dotnet lockfile current"),
        "{}",
        report(&run)
    );

    let run = dx_in(&root, &["update", "--check", "--offline"]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        format!("{}{}", run.stdout, run.stderr).contains("offline_required"),
        "{}",
        report(&run)
    );

    write(
        &root.join("third_party/dotnet/paket.lock"),
        "not a paket lock\n",
    );
    let run = dx_in(&root, &["update", "--check"]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(!run.stdout.contains("lockfile current"), "{}", report(&run));
}
