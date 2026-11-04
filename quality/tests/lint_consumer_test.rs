use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

struct Row {
    tool: &'static str,
    family: &'static str,
    lib_build: &'static str,
    source: &'static str,
    clean: &'static str,
    dirty: &'static str,
    dirty_report: &'static str,
    broken: Option<(&'static str, &'static str)>,
    tool_flag: &'static str,
    flag_required: bool,
}

const ROWS: &[Row] = &[
    Row {
        tool: "zig_ast_check",
        family: "zig",
        lib_build: r#"load("@rules_dx//zig/rules:defs.bzl", "zig_library")

zig_library(
    name = "math",
    main = "math.zig",
)
"#,
        source: "math.zig",
        clean: "pub fn add(a: i32, b: i32) i32 {\n    return a + b;\n}\n",
        dirty: "pub fn add(a: i32) i32 {\n    const unused = 1;\n    return a;\n}\n",
        dirty_report: "lib/math.zig:2:11: error: unused local constant [zig_ast_check/ast_check]",
        broken: Some((
            "pub fn add(a: i32, b: i32) i32 {\n    return ;a\n}\n",
            "lib/math.zig:2:14: error: expected ';' after statement [zig_ast_check/ast_check]",
        )),
        tool_flag: "--@rules_dx//config:tool_zig_ast_check=@rules_dx//zig/tools:zig",
        flag_required: false,
    },
];

fn row() -> &'static Row {
    let tool =
        std::env::var("DX_LINT_TOOL").unwrap_or_else(|_| panic!("DX_LINT_TOOL must name a tool"));
    ROWS.iter()
        .find(|row| row.tool == tool)
        .unwrap_or_else(|| panic!("no lint row for {tool}"))
}

fn consumer(row: &Row, lib_build: &str, source: &str) -> Case {
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
    write(
        &root.join("BUILD.bazel"),
        &format!(
            r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "family",
    family_id = "{}",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["{}"],
)

workspace_policy(
    name = "policy",
    families = [":family"],
)
"#,
            row.family, row.tool
        ),
    );
    write(&root.join("lib/BUILD.bazel"), lib_build);
    write(&root.join("lib").join(row.source), source);
    root
}

fn dx_with(row: &Row, root: &Case, flag: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![
        "lint",
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ];
    if flag {
        argv.push(row.tool_flag);
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(row: &Row, root: &Case) -> Run {
    dx_with(row, root, row.flag_required)
}

#[test]
fn clean_source_passes() {
    let row = row();
    let root = consumer(row, row.lib_build, row.clean);
    let out = dx(row, &root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("lib").join(row.source)), row.clean);
}

#[test]
fn finding_is_reported_and_the_source_is_left_alone() {
    let row = row();
    let root = consumer(row, row.lib_build, row.dirty);
    let out = dx(row, &root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.stdout.contains(row.dirty_report), "{}", out.combined());
    assert_eq!(read(&root.join("lib").join(row.source)), row.dirty);
}

#[test]
fn unparsable_source_is_never_clean() {
    let row = row();
    let Some((broken, report)) = row.broken else {
        return;
    };
    let root = consumer(row, row.lib_build, broken);
    let out = dx(row, &root);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.stdout.contains(report), "{}", out.combined());
    assert_eq!(read(&root.join("lib").join(row.source)), broken);
}

#[test]
fn tool_flag_is_honored() {
    let row = row();
    let root = consumer(row, row.lib_build, row.dirty);
    let out = dx_with(row, &root, true);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(out.stdout.contains(row.dirty_report), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let row = row();
    if !row.flag_required {
        return;
    }
    let root = consumer(row, row.lib_build, row.clean);
    let out = dx_with(row, &root, false);
    let name = row.tool_flag.split('=').next().unwrap_or_default();
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.combined().contains(name), "{}", out.combined());
}
