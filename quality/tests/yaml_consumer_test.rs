use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const APP: &str = "defaults: &defaults\n  retries: 3\n  region: eu\nservice:\n  settings: *defaults\n  name: api\n---\nsecond: 2\n";

const EXTRA: &str = "list:\n  - a\n  - b\n";

const DIRTY: &str = "list:   [a,b]\nname: api\n";

const FORMATTED_DIRTY: &str = "list: [a, b]\nname: api\n";

const UNOWNED: &str = "key:    value\nother: [1,2]\n";

const TRAILING: &str = "name: api   \n";

const BROKEN: &str = "key: [unclosed\n";

const INDENT4: &str = "formatter:\n  indent: 4\n";

const LINT_DEFAULT: &str = "extends: default\nrules:\n  document-start: disable\n";

const LINT_RELAXED: &str =
    "extends: default\nrules:\n  document-start: disable\n  trailing-spaces: disable\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.6")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "yamlfmt_config", "yamllint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "yaml_family",
    family_id = "yaml",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["yamlfmt"],
    lint = ["yamllint"],
)

workspace_policy(
    name = "policy",
    families = [":yaml_family"],
)

yamlfmt_config(
    name = "indent4",
    src = "indent4.yamlfmt",
    visibility = ["//visibility:public"],
)

yamllint_config(
    name = "lint_default",
    src = "default.yamllint",
    visibility = ["//visibility:public"],
)

yamllint_config(
    name = "lint_relaxed",
    src = "relaxed.yamllint",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//yaml/rules:defs.bzl", "yaml_library")

yaml_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "app.yaml",
        "extra.yml",
    ],
    deps = ["//lib:base"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//yaml/rules:defs.bzl", "yaml_library")

yaml_library(
    name = "base",
    aspect_hints = ["//:lint_default"],
    srcs = ["base.yaml"],
    visibility = ["//visibility:public"],
)
"#;

const BASE: &str = "base: 1\n";

const HOST_CONFIG: &str =
    "extends: default\nrules:\n  document-start: disable\n  trailing-spaces: disable\n";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("quality/tests/yaml_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints),
    );
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
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
    write(&root.join("indent4.yamlfmt"), INDENT4);
    write(&root.join("default.yamllint"), LINT_DEFAULT);
    write(&root.join("relaxed.yamllint"), LINT_RELAXED);
    set_hints(&root, "[\"//:lint_default\"]");
    write(&root.join("app/app.yaml"), APP);
    write(&root.join("app/extra.yml"), EXTRA);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.yaml"), BASE);
    write(&root.join("docs/unowned.yaml"), UNOWNED);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("yaml-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn format_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let extra = root.join("app/extra.yml");
    write(&extra, DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/extra.yml"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("docs/unowned.yaml"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&extra), DIRTY);
    assert_eq!(read(&root.join("app/app.yaml")), APP);
    assert_eq!(read(&root.join("docs/unowned.yaml")), UNOWNED);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), FORMATTED_DIRTY);
    assert_eq!(read(&root.join("app/app.yaml")), APP);
    assert_eq!(read(&root.join("docs/unowned.yaml")), UNOWNED);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn multiple_documents_and_anchors_pass_both_tools() {
    let root = consumer();
    let format = dx(&root, "format", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
    let lint = dx(&root, "lint", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    assert_eq!(read(&root.join("app/app.yaml")), APP);
}

#[test]
fn a_bound_yamlfmt_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:lint_default\", \"//:indent4\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/app.yaml"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&root.join("app/app.yaml")), APP);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(
        read(&root.join("app/app.yaml")).contains("\n    retries: 3\n"),
        "{}",
        read(&root.join("app/app.yaml"))
    );
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn lint_reports_rules_and_a_bound_config_changes_them() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);

    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/extra.yml:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("trailing-spaces"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&extra), TRAILING);

    set_hints(&root, "[\"//:lint_relaxed\"]");
    let relaxed = dx(&root, "lint", &[]);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("trailing-spaces") || out.combined().contains("check-only"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&extra), TRAILING);
}

#[test]
fn a_parse_error_is_reported_by_both_tools_and_never_clean() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, BROKEN);

    for command in ["format", "lint"] {
        let out = dx(&root, command, &[]);
        let text = out.combined();
        assert_eq!(out.status.code(), Some(1), "{command}: {text}");
        assert!(text.contains("app/extra.yml"), "{command}: {text}");
        assert_eq!(read(&extra), BROKEN);
    }
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), BROKEN);
}

#[test]
fn lint_includes_dependency_libraries() {
    let root = consumer();
    let base = root.join("lib/base.yaml");
    write(&base, "base: 1   \n");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lib/base.yaml:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("trailing-spaces"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&base), "base: 1   \n");
}

#[test]
fn lint_without_a_bound_config_ignores_host_configs() {
    let root = consumer();
    set_hints(&root, "[]");
    write(&root.join(".yamllint"), HOST_CONFIG);
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("trailing-spaces"),
        "{}",
        out.combined()
    );
}
