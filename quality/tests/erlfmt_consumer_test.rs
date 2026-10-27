use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const DIRTY: &str = "-module(math).\n-export([add/2]).\n\nadd(A,B)->A+B.\n";
const FORMATTED: &str = "-module(math).\n-export([add/2]).\n\nadd(A, B) -> A + B.\n";
const BROKEN: &str = "-module(math).\n-export([add/2]).\n\nadd(A,B)->A+B.\nbroken(X) -> X +.\n";
const IGNORED: &str = "-module(math).\n-export([add/2, table/0]).\n\nadd(A, B) -> A + B.\n\n%% erlfmt-ignore\ntable() -> [ 1,  2,\n            30, 40 ].\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "erlang_family",
    family_id = "erlang",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["erlfmt"],
)

workspace_policy(
    name = "policy",
    families = [":erlang_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "math",
    app_version = "1.0.0",
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("erlang/tests/runtime/src/dx_runtime_probe.erl");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(5)
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
    write(&root.join("lib/src/math.erl"), source);
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
        argv.push("--@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, true)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let root = consumer(DIRTY);
    let source = root.join("lib/src/math.erl");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("lib/src/math.erl:1:1: warning: file is not formatted [erlfmt]"),
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
    let source = root.join("lib/src/math.erl");

    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked
            .stdout
            .contains("lib/src/math.erl:5:17: error: syntax error before: '.' [erlfmt/syntax]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), BROKEN);

    let applied = dx(&root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), BROKEN);
}

#[test]
fn erlfmt_ignore_comment_keeps_a_form_as_written() {
    let root = consumer(IGNORED);
    let checked = dx(&root, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    assert_eq!(read(&root.join("lib/src/math.erl")), IGNORED);
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(DIRTY);
    let out = dx_with(&root, &[], false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_erlfmt"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/src/math.erl")), DIRTY);
}
