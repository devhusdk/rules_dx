use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "module Calc\n\ntwice(x::Int) = x * 2\n\nend\n";
const BAD: &str = "module Calc\n\ntotal(x::Int) = x + missing_global\n\nend\n";
const PROJECT: &str =
    "name = \"Calc\"\nuuid = \"5f1c4b6e-3c3e-4b3a-9f0e-2a7d6a1c9b11\"\nversion = \"0.1.0\"\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "julia_family",
    family_id = "julia",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["jet"],
)

workspace_policy(
    name = "policy",
    families = [":julia_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//julia/rules:defs.bzl", "julia_library")

julia_library(
    name = "Calc",
    srcs = ["src/Calc.jl"],
    data = ["Project.toml"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("julia/rules/defs.bzl");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(source: &str) -> Case {
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
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/Project.toml"), PROJECT);
    write(&root.join("lib/src/Calc.jl"), source);
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
        argv.push("--@rules_dx//config:tool_jet=@rules_dx//julia/tools/jet:jet");
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
    let root = consumer(CLEAN);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn undefined_global_is_reported_at_its_line() {
    let root = consumer(BAD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/src/Calc.jl:3") && out.stdout.contains("[jet/UndefVar"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/src/Calc.jl")), BAD);
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(CLEAN);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_jet"),
        "{}",
        out.combined()
    );
}
