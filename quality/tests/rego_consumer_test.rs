use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const BASE: &str = "package lib.base\n\nanswer := 42\n";

const BASE_DIRTY: &str = "package lib.base\n\nanswer   :=   42\n";

const APP: &str = "package app\n\nimport data.lib.base\n\nvalue := base.answer\n";

const APP_DIRTY: &str = "package app\n\nimport data.lib.base\n\nvalue   :=   base.answer\n";

const APP_UNDEFINED: &str = "package app\n\nimport data.lib.base\n\nvalue := base.missing(1)\n";

const BROKEN: &str = "package app\n\nvalue := := 1\n";

const DATA: &str = "{\"limit\": 1}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "rego_family",
    family_id = "rego",
    disabled = [
        "audit",
        "lint",
    ],
    format = ["opa"],
    typecheck = ["opa"],
)

workspace_policy(
    name = "policy",
    families = [":rego_family"],
)

filegroup(
    name = "two_files",
    srcs = [
        "BUILD.bazel",
        "MODULE.bazel",
    ],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//rego/rules:defs.bzl", "rego_library")

rego_library(
    name = "app",
    srcs = ["main.rego"],
    data = ["data.json"],
    deps = ["//lib:base"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//rego/rules:defs.bzl", "rego_library")

rego_library(
    name = "base",
    srcs = ["base.rego"],
    visibility = ["//visibility:public"],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
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
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.rego"), APP);
    write(&root.join("app/data.json"), DATA);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.rego"), BASE);
    write(&root.join("docs/unowned.rego"), APP_DIRTY);
    root
}

fn dx(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("rego-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn clean_packages_pass_format_and_typecheck() {
    let root = consumer();
    for command in ["format", "typecheck"] {
        let out = dx(&root, command, &[], &[]);
        assert_eq!(out.status.code(), Some(0), "{command}: {}", out.combined());
    }
    assert_eq!(read(&root.join("app/main.rego")), APP);
    assert_eq!(read(&root.join("lib/base.rego")), BASE);
    assert_eq!(read(&root.join("app/data.json")), DATA);
}

#[test]
fn format_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let app = root.join("app/main.rego");
    let base = root.join("lib/base.rego");
    write(&app, APP_DIRTY);
    write(&base, BASE_DIRTY);

    let dirty = dx(&root, "format", &[], &[]);
    let text = dirty.combined();
    assert_eq!(dirty.status.code(), Some(1), "{text}");
    assert!(text.contains("app/main.rego:"), "{text}");
    assert!(text.contains("lib/base.rego:"), "{text}");
    assert!(!text.contains("docs/unowned.rego"), "{text}");
    assert_eq!(read(&app), APP_DIRTY);
    assert_eq!(read(&base), BASE_DIRTY);

    let applied = dx(&root, "format", &["--apply"], &[]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&app), APP);
    assert_eq!(read(&base), BASE);
    assert_eq!(read(&root.join("docs/unowned.rego")), APP_DIRTY);
    assert_eq!(read(&root.join("app/data.json")), DATA);

    let again = dx(&root, "format", &[], &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn an_undefined_function_is_reported_with_file_and_line() {
    let root = consumer();
    let app = root.join("app/main.rego");
    write(&app, APP_UNDEFINED);
    let out = dx(&root, "typecheck", &[], &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("app/main.rego:5"), "{text}");
    assert!(text.contains("rego_type_error"), "{text}");
    assert_eq!(read(&app), APP_UNDEFINED);
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let app = root.join("app/main.rego");
    write(&app, BROKEN);
    for command in ["format", "typecheck"] {
        let out = dx(&root, command, &[], &[]);
        let text = out.combined();
        assert_eq!(out.status.code(), Some(1), "{command}: {text}");
        assert!(text.contains("app/main.rego"), "{command}: {text}");
    }
    let applied = dx(&root, "format", &["--apply"], &[]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&app), BROKEN);
}

#[test]
fn a_tool_override_that_is_not_a_file_fails_and_names_the_tool() {
    let root = consumer();
    let out = dx(
        &root,
        "format",
        &[],
        &["--@rules_dx//config:tool_opa=//:two_files"],
    );
    let text = out.combined();
    assert_ne!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("opa"), "{text}");
}
