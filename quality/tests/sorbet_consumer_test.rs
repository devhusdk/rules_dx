use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "# typed: true\n\nputs 1\n";
const BAD_CALL: &str =
    "# typed: true\n\nclass Calc\n  def twice(x)\n    x * 2\n  end\nend\n\nCalc.new.thrice(1)\n";
const BASE: &str = "# typed: true\n\nmodule Base\n  LIMIT = 3\nend\n";
const CALLER: &str = "# typed: true\n\nputs Base::LIMIT\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "ruby_family",
    family_id = "ruby",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["sorbet"],
)

workspace_policy(
    name = "policy",
    families = [":ruby_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_library")

ruby_library(
    name = "calc",
    srcs = ["calc.rb"],
)
"#;

const CALLER_BUILD: &str = r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_library")

ruby_library(
    name = "calc",
    srcs = ["calc.rb"],
    deps = ["//base:limits"],
)
"#;

const BASE_BUILD: &str = r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_library")

ruby_library(
    name = "limits",
    srcs = ["limits.rb"],
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
    let sample = resolve_runfiles("ruby/rules/defs.bzl");
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
    write(&root.join("lib/calc.rb"), source);
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
        argv.push("--@rules_dx//config:tool_sorbet=@rules_dx//ruby/tools:sorbet");
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
fn unknown_method_is_reported_at_its_line() {
    let root = consumer(BAD_CALL, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/calc.rb:9") && out.stdout.contains("[sorbet/7003]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/calc.rb")), BAD_CALL);
}

#[test]
fn dependency_constants_resolve_without_findings() {
    let root = consumer(CALLER, CALLER_BUILD);
    write(&root.join("base/BUILD.bazel"), BASE_BUILD);
    write(&root.join("base/limits.rb"), BASE);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_sorbet"),
        "{}",
        out.combined()
    );
}
