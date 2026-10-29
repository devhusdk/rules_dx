mod common;

use common::{dx, error_codes, events, finished, of_kind, read, rules_dx_root, write, Tamper};
use dx_nested::{self as nested, Case};

const CLEAN: &str = "def add(a: int, b: int) -> int:\n    return a + b\n";
const DIRTY: &str = "import os\n\n\ndef add(a: int, b: int) -> int:\n    return a + b\n";

const MODULE: &str = r#"module(name = "quality_results_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "my_dx")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.10")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

const BUILD: &str = r#"load("@my_dx//quality:native_config.bzl", "ruff_config")
load("@my_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "python_family",
    family_id = "python",
    disabled = ["audit"],
    lint = ["ruff"],
)

workspace_policy(
    name = "policy",
    families = [":python_family"],
)

ruff_config(
    name = "ruff",
    src = "ruff.toml",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@my_dx//python/rules:defs.bzl", "python_library")

python_library(
    name = "clean",
    srcs = ["clean.py"],
    aspect_hints = ["//:ruff"],
)

python_library(
    name = "dirty",
    srcs = ["dirty.py"],
    aspect_hints = ["//:ruff"],
)
"#;

/// Two python libraries under ruff; `dirty.py` has an unused import.
fn quality_consumer() -> Case {
    let rules = rules_dx_root();
    let case = nested::case("quality");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("ruff.toml"), "line-length = 88\n");
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/clean.py"), CLEAN);
    write(&root.join("app/dirty.py"), DIRTY);
    case
}

const LINT: [&str; 4] = [
    "//app/...",
    "--output=json",
    "--",
    "--@my_dx//config:workspace=//:policy",
];

fn lint(mode: &str) -> Vec<&str> {
    let mut args = vec!["lint", mode];
    args.extend_from_slice(&LINT);
    args
}

#[test]
fn clean_check_run_is_complete() {
    let case = quality_consumer();
    let tamper = Tamper::new();
    let run = dx(&case, &lint("--check"), &[], &tamper);
    let all = events(&run);
    // The unused import is a finding (exit 1) but the evidence is complete.
    assert!(
        !error_codes(&all).contains(&"incomplete_results".to_owned()),
        "{}",
        run.combined()
    );
    assert_eq!(finished(&all)["results_complete"], true, "{}", run.stdout);
    assert!(tamper.changed().is_empty());
}

#[test]
fn tampered_check_run_is_incomplete() {
    let case = quality_consumer();
    let tamper = Tamper::new();
    let run = dx(
        &case,
        &lint("--check"),
        &[("DX_TAMPER_FILE", r"/dirty-real-lint[^/]*\.pb$")],
        &tamper,
    );
    assert!(!tamper.changed().is_empty(), "{}", run.combined());
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    // A result file that BEP names but that is gone is reported as invalid_bep, not incomplete_results.
    assert!(
        error_codes(&all).contains(&"invalid_bep".to_owned()),
        "{}",
        run.stdout
    );
    assert_eq!(finished(&all)["results_complete"], false, "{}", run.stdout);
    assert_eq!(of_kind(&all, "command_finished").len(), 1);
}

#[test]
fn tampered_apply_leaves_the_dirty_file_unchanged() {
    let case = quality_consumer();
    let tamper = Tamper::new();
    let path = case.root().join("app/dirty.py");
    let run = dx(
        &case,
        &lint("--apply"),
        &[("DX_TAMPER_FILE", r"/dirty-real-lint[^/]*\.pb$")],
        &tamper,
    );
    assert!(!tamper.changed().is_empty(), "{}", run.combined());
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(
        read(&path),
        DIRTY,
        "missing evidence never authorizes an edit"
    );
    assert_eq!(read(&case.root().join("app/clean.py")), CLEAN);
}
