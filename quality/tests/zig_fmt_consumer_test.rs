use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const DIRTY: &str = "pub fn add(a: i32, b: i32) i32 {\n    return a+b;\n}\n";
const FORMATTED: &str = "pub fn add(a: i32, b: i32) i32 {\n    return a + b;\n}\n";
const BROKEN: &str = "pub fn add(a: i32, b: i32) i32 {\n    return ;a\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "zig_family",
    family_id = "zig",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["zig_fmt"],
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
    let mut argv: Vec<&str> = vec!["format"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if tool {
        argv.push("--@rules_dx//config:tool_zig_fmt=@rules_dx//zig/tools:zig");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, false)
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let root = consumer(DIRTY);
    let source = root.join("lib/math.zig");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("lib/math.zig:1:1: warning: file is not formatted [zig_fmt]"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&source), DIRTY);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), FORMATTED);

    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&source), FORMATTED);
}

#[test]
fn syntax_error_is_reported_at_its_location_and_left_untouched() {
    let root = consumer(BROKEN);
    let source = root.join("lib/math.zig");

    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked
            .stdout
            .contains("lib/math.zig:2:14: error: expected ';' after statement [zig_fmt/syntax]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), BROKEN);

    let applied = dx(&root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), BROKEN);
}

#[test]
fn explicit_tool_flag_still_formats() {
    let root = consumer(DIRTY);
    let out = dx_with(&root, &[], true);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout
            .contains("lib/math.zig:1:1: warning: file is not formatted [zig_fmt]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/math.zig")), DIRTY);
}
