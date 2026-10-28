use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "---@param x integer\n---@return integer\nlocal function twice(x)\n  return x * 2\nend\n\nreturn { twice = twice }\n";
const BAD_CALL: &str = "---@param x integer\n---@return integer\nlocal function twice(x)\n  return x * 2\nend\n\nlocal _ = twice(\"a\")\n\nreturn { twice = twice }\n";
const BASE: &str = "---@param x integer\n---@return integer\nlocal function twice(x)\n  return x * 2\nend\n\nreturn { twice = twice }\n";
const CALLER: &str = "local util = require(\"base.util\")\n\nlocal M = {}\n\n---@param x integer\n---@return integer\nfunction M.quad(x)\n  return util.twice(util.twice(x))\nend\n\nreturn M\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "lua_family",
    family_id = "lua",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["luals"],
)

workspace_policy(
    name = "policy",
    families = [":lua_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//lua/rules:defs.bzl", "lua_library")

lua_library(
    name = "calc",
    srcs = ["calc.lua"],
)
"#;

const CALLER_BUILD: &str = r#"load("@rules_dx//lua/rules:defs.bzl", "lua_library")

lua_library(
    name = "calc",
    srcs = ["calc.lua"],
    deps = ["//base:util"],
)
"#;

const BASE_BUILD: &str = r#"load("@rules_dx//lua/rules:defs.bzl", "lua_library")

lua_library(
    name = "util",
    srcs = ["util.lua"],
    visibility = ["//visibility:public"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("lua/rules/defs.bzl");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(source: &str, lib_build: &str) -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), lib_build);
    write(&root.join("lib/calc.lua"), source);
    root
}

fn dx_with(root: &Case, tool: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![
        "typecheck",
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ];
    if tool {
        argv.push("--@rules_dx//config:tool_luals=@rules_dx//lua/tools:luals");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case) -> Run {
    dx_with(root, true)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

#[test]
fn clean_library_passes() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn parameter_type_mismatch_is_reported_at_its_line() {
    let root = consumer(BAD_CALL, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/calc.lua:7") && out.stdout.contains("[luals/param-type-mismatch]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/calc.lua")), BAD_CALL);
}

#[test]
fn dependency_modules_resolve_without_findings() {
    let root = consumer(CALLER, CALLER_BUILD);
    write(&root.join("base/BUILD.bazel"), BASE_BUILD);
    write(&root.join("base/util.lua"), BASE);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_luals"),
        "{}",
        out.combined()
    );
}
