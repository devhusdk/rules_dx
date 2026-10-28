use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const ROOT_LISTS: &str =
    "cmake_minimum_required(VERSION 3.20)\nproject(app)\nadd_subdirectory(sub)\n";

const ROOT_LISTS_DIRTY: &str =
    "cmake_minimum_required( VERSION 3.20 )\nproject(  app )\nadd_subdirectory(sub)\n";

const SUB_LISTS: &str = "add_library(sub sub.c)\n";

const HELPERS: &str =
    "function(greet name)\n    message(STATUS \"hello ${name}\")\nendfunction()\n";

const HELPERS_TWO_SPACES: &str =
    "function(greet name)\n  message(STATUS \"hello ${name}\")\nendfunction()\n";

const UNOWNED: &str = "set(  VENDORED   1 )\n";

const BROKEN: &str = "add_library(sub\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//cmake/rules:defs.bzl", "cmake_library")
load("@rules_dx//quality:native_config.bzl", "gersemi_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "cmake_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "cmake",
    format = ["gersemi"],
)

workspace_policy(
    name = "policy",
    families = [":cmake_family"],
)

gersemi_config(
    name = "two_spaces",
    src = "two_spaces.yaml",
    visibility = ["//visibility:public"],
)

cmake_library(
    name = "root",
    srcs = ["CMakeLists.txt"],
)
"#;

const TWO_SPACES: &str = "indent: 2\nline_length: 100\n";

const SUB_BUILD: &str = r#"load("@rules_dx//cmake/rules:defs.bzl", "cmake_library")

cmake_library(
    name = "sub",
    srcs = ["CMakeLists.txt"],
)
"#;

const CMAKE_BUILD: &str = r#"load("@rules_dx//cmake/rules:defs.bzl", "cmake_library")

cmake_library(
    name = "helpers",
    aspect_hints = HINTS,
    srcs = ["helpers.cmake"],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("cmake/BUILD.bazel"),
        &CMAKE_BUILD.replace("HINTS", hints),
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
    write(&root.join("two_spaces.yaml"), TWO_SPACES);
    write(&root.join("CMakeLists.txt"), ROOT_LISTS);
    write(&root.join("sub/BUILD.bazel"), SUB_BUILD);
    write(&root.join("sub/CMakeLists.txt"), SUB_LISTS);
    set_hints(&root, "[]");
    write(&root.join("cmake/helpers.cmake"), HELPERS);
    // No BUILD file in this directory: no cmake_library owns the file.
    write(&root.join("vendor/unowned.cmake"), UNOWNED);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

fn dx_with(root: &Case, command: &str, args: &[&str], bazel_args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("cmake-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    argv.extend_from_slice(bazel_args);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn a_clean_tree_passes_a_dirty_file_is_reported_and_apply_repairs_only_owned_files() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let lists = root.join("CMakeLists.txt");
    write(&lists, ROOT_LISTS_DIRTY);
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("CMakeLists.txt:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("helpers.cmake") && !dirty.combined().contains("unowned"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&lists), ROOT_LISTS_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&lists), ROOT_LISTS);
    assert_eq!(read(&root.join("sub/CMakeLists.txt")), SUB_LISTS);
    assert_eq!(read(&root.join("cmake/helpers.cmake")), HELPERS);
    assert_eq!(read(&root.join("vendor/unowned.cmake")), UNOWNED);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_located_and_apply_leaves_the_file_unchanged() {
    let root = consumer();
    let sub = root.join("sub/CMakeLists.txt");
    write(&sub, BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("sub/CMakeLists.txt:"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&sub), BROKEN);
}

#[test]
fn a_bound_gersemi_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:two_spaces\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("cmake/helpers.cmake"),
        "{}",
        dirty.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("cmake/helpers.cmake")), HELPERS_TWO_SPACES);
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn stray_unbound_configs_next_to_the_sources_are_ignored() {
    let root = consumer();
    write(&root.join("cmake/.gersemirc"), TWO_SPACES);
    write(&root.join(".gersemirc"), TWO_SPACES);
    write(&root.join("cmake/gersemi.yaml"), TWO_SPACES);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("cmake/helpers.cmake")), HELPERS);
}

#[test]
fn a_tool_override_that_is_not_one_executable_fails_naming_gersemi() {
    let root = consumer();
    write(
        &root.join("tools/BUILD.bazel"),
        "filegroup(name = \"two_files\", srcs = [\"a.txt\", \"b.txt\"], visibility = [\"//visibility:public\"])\n",
    );
    write(&root.join("tools/a.txt"), "a\n");
    write(&root.join("tools/b.txt"), "b\n");
    let rules = ["--@rules_dx//config:tool_gersemi=//tools:two_files"];
    let out = dx_with(&root, "format", &[], &rules);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().to_lowercase().contains("gersemi"),
        "{}",
        out.combined()
    );
}
