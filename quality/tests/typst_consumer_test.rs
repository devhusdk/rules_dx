use std::path::PathBuf;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const HELPERS: &str = "#let greet(name) = [Hello, #name!]\n";

const HELPERS_DIRTY: &str = "#let   greet(name) = [Hello, #name!]\n";

const MAIN: &str = "#import \"../lib/helpers.typ\": greet\n\n#greet(\"app\")\n";

const MAIN_DIRTY: &str = "#import \"../lib/helpers.typ\": greet\n\n#greet(  \"app\")\n";

const BROKEN: &str = "#let greeting = (\n";

const UNOWNED: &str = "#let   notes = 1\n";

const TYPSTYLE_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_typstyle_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "typst_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "typst",
    format = ["typstyle"],
)

workspace_policy(
    name = "policy",
    families = [":typst_family"],
)
"#;

const UNWIRED_BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "typst_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "typst",
    format = ["jsonnetfmt"],
)

workspace_policy(
    name = "policy",
    families = [":typst_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//typst/rules:defs.bzl", "typst_library")

typst_library(
    name = "lib",
    srcs = ["helpers.typ"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//typst/rules:defs.bzl", "typst_library")

typst_library(
    name = "app",
    srcs = ["main.typ"],
    deps = ["//lib"],
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
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/helpers.typ"), HELPERS);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.typ"), MAIN);
    write(&root.join("app/notes.typ"), UNOWNED);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("typst-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_owned_files_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let helpers = root.join("lib/helpers.typ");
    let main = root.join("app/main.typ");
    let notes = root.join("app/notes.typ");
    write(&helpers, HELPERS_DIRTY);
    write(&main, MAIN_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("lib/helpers.typ:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("app/main.typ:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/notes.typ"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&helpers), HELPERS_DIRTY);
    assert_eq!(read(&main), MAIN_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&helpers), HELPERS);
    assert_eq!(read(&main), MAIN);
    assert_eq!(read(&notes), UNOWNED);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn only_the_dirty_file_of_several_is_reported() {
    let root = consumer();
    write(&root.join("lib/helpers.typ"), HELPERS_DIRTY);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lib/helpers.typ:"),
        "{}",
        out.combined()
    );
    assert!(
        !out.combined().contains("app/main.typ"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let main = root.join("app/main.typ");
    write(&main, BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/main.typ"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&main), BROKEN);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_policy_selecting_an_unwired_tool_fails_with_the_policy_message() {
    let root = consumer();
    write(&root.join("BUILD.bazel"), UNWIRED_BUILD);
    let out = dx(&root, "format", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("selects format tool 'jsonnetfmt' with no wired executable"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_declared_typstyle_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("typst-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("lib/helpers.typ"), HELPERS_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("lib/helpers.typ:"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", TYPSTYLE_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        TYPSTYLE_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined()
            .contains("typstyle-x86_64-unknown-linux-gnu"),
        "{}",
        miss.combined()
    );
}

#[test]
fn the_declared_typstyle_is_the_pinned_version() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let typstyle = PathBuf::from(base.stdout.trim()).join(
        "external/rules_dx++dx_tools+dx_typstyle_linux_x86_64/typstyle-x86_64-unknown-linux-gnu",
    );
    let version = std::process::Command::new(&typstyle)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("{typstyle:?}: {error}"));
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("0.15.2"),
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
}
