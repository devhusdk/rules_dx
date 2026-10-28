use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const APP: &str =
    "{ pkgs }:\nlet\n  name = \"dx\";\nin\n{\n  inherit name;\n  shell = pkgs.mkShell { };\n}\n";

const BASE: &str = "{\n  base = 1;\n}\n";

const EMPTY_LET: &str = "let\n  a = let in 1;\nin\na\n";

const EMPTY_LET_FIXED: &str = "let\n  a = 1;\nin\na\n";

const UNUSED: &str = "let\n  a = 1;\n  b = 2;\nin\na\n";

const UNUSED_FIXED: &str = "let\n  a = 1;\nin\na\n";

// nixfmt rewrites this to `{ a = 1; }`.
const SQUASHED: &str = "{a=1;}\n";

const SQUASHED_FIXED: &str = "{ a = 1; }\n";

const BROKEN: &str = "{ a = 1;\n";

// deadnix leaves this file alone, so only statix has an opinion about it.
const MANUAL_INHERIT: &str = "let\n  a = 1;\nin\n{ a = a; }\n";

const RELAXED: &str = "disabled = [\"manual_inherit\"]\n";

const HOST_RELAXED: &str = "disabled = [\"empty_let_in\"]\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "statix_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "nix_family",
    family_id = "nix",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["nixfmt"],
    lint = [
        "deadnix",
        "statix",
    ],
)

workspace_policy(
    name = "policy",
    families = [":nix_family"],
)

statix_config(
    name = "relaxed",
    src = "relaxed.toml",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//nix/rules:defs.bzl", "nix_library")

nix_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "default.nix",
        "extra.nix",
    ],
    deps = ["//lib:base"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//nix/rules:defs.bzl", "nix_library")

nix_library(
    name = "base",
    srcs = ["base.nix"],
    visibility = ["//visibility:public"],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints),
    );
}

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
    write(&root.join("relaxed.toml"), RELAXED);
    set_hints(&root, "[]");
    write(&root.join("app/default.nix"), APP);
    write(&root.join("app/extra.nix"), BASE);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.nix"), BASE);
    write(&root.join("docs/unowned.nix"), UNUSED);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("nix-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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
fn clean_packages_pass_both_tools() {
    let root = consumer();
    let lint = dx(&root, "lint", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    assert_eq!(read(&root.join("app/default.nix")), APP);
}

#[test]
fn lint_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let extra = root.join("app/extra.nix");
    let base = root.join("lib/base.nix");
    write(&extra, EMPTY_LET);
    write(&base, UNUSED);

    let dirty = dx(&root, "lint", &[]);
    let text = dirty.combined();
    assert_eq!(dirty.status.code(), Some(1), "{text}");
    assert!(text.contains("app/extra.nix:"), "{text}");
    assert!(text.contains("lib/base.nix:"), "{text}");
    assert!(text.contains("W02"), "{text}");
    assert!(text.contains("unused_let_binding"), "{text}");
    assert!(!text.contains("docs/unowned.nix"), "{text}");
    assert_eq!(read(&extra), EMPTY_LET);
    assert_eq!(read(&base), UNUSED);

    let applied = dx(&root, "lint", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), EMPTY_LET_FIXED);
    assert_eq!(read(&base), UNUSED_FIXED);
    assert_eq!(read(&root.join("app/default.nix")), APP);
    assert_eq!(read(&root.join("docs/unowned.nix")), UNUSED);

    let again = dx(&root, "lint", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_bound_statix_config_disables_a_lint() {
    let root = consumer();
    let extra = root.join("app/extra.nix");
    write(&extra, MANUAL_INHERIT);
    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());

    set_hints(&root, "[\"//:relaxed\"]");
    let relaxed = dx(&root, "lint", &[]);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
    assert_eq!(read(&extra), MANUAL_INHERIT);
}

#[test]
fn lint_without_a_bound_config_ignores_host_configs() {
    let root = consumer();
    write(&root.join("statix.toml"), HOST_RELAXED);
    write(&root.join("app/statix.toml"), HOST_RELAXED);
    let extra = root.join("app/extra.nix");
    write(&extra, EMPTY_LET);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.combined().contains("W02"), "{}", out.combined());
}

#[test]
fn a_parse_error_is_reported_and_never_clean() {
    let root = consumer();
    let extra = root.join("app/extra.nix");
    write(&extra, BROKEN);
    let out = dx(&root, "lint", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("app/extra.nix"), "{text}");
    assert_eq!(read(&extra), BROKEN);

    let applied = dx(&root, "lint", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), BROKEN);
}

#[test]
fn clean_packages_pass_a_multi_file_format_check() {
    let root = consumer();
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("app/default.nix")), APP);
    assert_eq!(read(&root.join("app/extra.nix")), BASE);
    assert_eq!(read(&root.join("lib/base.nix")), BASE);
}

#[test]
fn format_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let extra = root.join("app/extra.nix");
    let unowned = root.join("docs/unowned.nix");
    write(&extra, SQUASHED);
    write(&unowned, SQUASHED);

    let dirty = dx(&root, "format", &[]);
    let text = dirty.combined();
    assert_eq!(dirty.status.code(), Some(1), "{text}");
    assert!(text.contains("app/extra.nix"), "{text}");
    assert!(!text.contains("docs/unowned.nix"), "{text}");
    assert_eq!(read(&extra), SQUASHED);
    assert_eq!(read(&unowned), SQUASHED);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), SQUASHED_FIXED);
    assert_eq!(read(&unowned), SQUASHED);
    assert_eq!(read(&root.join("app/default.nix")), APP);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_format_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let extra = root.join("app/extra.nix");
    write(&extra, BROKEN);
    let out = dx(&root, "format", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("app/extra.nix"), "{text}");
    assert_eq!(read(&extra), BROKEN);

    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), BROKEN);
}

#[test]
fn lint_and_format_run_together_on_the_same_tree() {
    let root = consumer();
    let lint = dx(&root, "lint", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    let format = dx(&root, "format", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
}
