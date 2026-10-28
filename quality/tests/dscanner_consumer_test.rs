use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str =
    "module math;\n\n/// Adds two numbers.\nint add(int a, int b)\n{\n    return a + b;\n}\n";
const UNUSED: &str = "module math;\n\n/// Adds two numbers.\nint add(int a, int b)\n{\n    int unused;\n    return a + b;\n}\n";
const BROKEN: &str =
    "module math;\n\n/// Adds two numbers.\nint add(int a, int b)\n{\n    return ;a\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "d_family",
    family_id = "d",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["dscanner"],
)

workspace_policy(
    name = "policy",
    families = [":d_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//d/rules:defs.bzl", "d_library")

d_library(
    name = "math",
    srcs = ["math.d"],
)
"#;

fn consumer(source: &str) -> Case {
    let rules = rules_dx_source();
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
    write(&root.join("lib/math.d"), source);
    root
}

fn dx_with(root: &Case, tool: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["lint"];
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if tool {
        argv.push("--@rules_dx//config:tool_dscanner=@rules_dx//d/tools:dscanner");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

#[test]
fn clean_source_passes_in_a_consumer_workspace() {
    let root = consumer(CLEAN);
    let out = dx_with(&root, false);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn unused_variable_is_reported_at_its_location() {
    let root = consumer(UNUSED);
    let out = dx_with(&root, false);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains(
            "lib/math.d:6:9: warning: Variable unused is never used. [dscanner/unused_variable_check]"
        ),
        "{}",
        out.combined()
    );
}

#[test]
fn syntax_error_is_reported_as_an_error() {
    let root = consumer(BROKEN);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.stdout
            .contains("lib/math.d:7:1: error: Expected `;` instead of `}` [dscanner/syntax]"),
        "{}",
        out.combined()
    );
}

#[test]
fn explicit_tool_flag_still_lints() {
    let root = consumer(UNUSED);
    let out = dx_with(&root, true);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("[dscanner/unused_variable_check]"),
        "{}",
        out.combined()
    );
}
