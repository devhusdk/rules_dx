use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str = "pub fn add(a: i32, b: i32) i32 {\n    return a + b;\n}\n";
const UNUSED: &str = "pub fn add(a: i32) i32 {\n    const unused = 1;\n    return a;\n}\n";
const BROKEN: &str = "pub fn add(a: i32, b: i32) i32 {\n    return ;a\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "zig_family",
    family_id = "zig",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["zig_ast_check"],
)

workspace_policy(
    name = "policy",
    families = [":zig_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//zig/rules:defs.bzl", "zig_library")

zig_library(
    name = "math",
    main = "math.zig",
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
    write(&root.join("lib/math.zig"), source);
    root
}

fn dx_with(root: &Case, args: &[&str], tool: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["lint"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if tool {
        argv.push("--@rules_dx//config:tool_zig_ast_check=@rules_dx//zig/tools:zig");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, false)
}

#[test]
fn clean_file_passes() {
    let root = consumer(CLEAN);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn unused_local_is_reported_at_its_location() {
    let root = consumer(UNUSED);
    let out = dx(&root, &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.stdout
            .contains("lib/math.zig:2:11: error: unused local constant [zig_ast_check/ast_check]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/math.zig")), UNUSED);
}

#[test]
fn syntax_error_is_reported_at_its_location() {
    let root = consumer(BROKEN);
    let out = dx(&root, &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.stdout.contains(
            "lib/math.zig:2:14: error: expected ';' after statement [zig_ast_check/ast_check]"
        ),
        "{}",
        out.combined()
    );
}

#[test]
fn explicit_tool_flag_still_lints() {
    let root = consumer(UNUSED);
    let out = dx_with(&root, &[], true);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.stdout
            .contains("lib/math.zig:2:11: error: unused local constant"),
        "{}",
        out.combined()
    );
}
