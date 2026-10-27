use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const DIRTY: &str = "defmodule Math do\n  def add( a,b ),do: a+b\nend\n";
const FORMATTED: &str = "defmodule Math do\n  def add(a, b), do: a + b\nend\n";
const BROKEN: &str = "defmodule Math do\n  def add( a,b ),do: a + * b\nend\n";
const UNICODE: &str = "defmodule Math do\n  @moduledoc \"\"\"\n  Søren's café sums 1 + 1.\n  \"\"\"\n\n  def label, do: \"π ≈ 3.14\"\nend\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "elixir_family",
    family_id = "elixir",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["mix_format"],
)

workspace_policy(
    name = "policy",
    families = [":elixir_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//elixir/rules:defs.bzl", "elixir_app")

elixir_app(
    app_name = "math",
    app_version = "1.0.0",
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
    write(&root.join("lib/lib/math.ex"), source);
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
        argv.push("--@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, true)
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let root = consumer(DIRTY);
    let source = root.join("lib/lib/math.ex");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("lib/lib/math.ex:1:1: warning: file is not formatted [mix_format]"),
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
    let source = root.join("lib/lib/math.ex");

    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked
            .stdout
            .contains("lib/lib/math.ex:2:26: error: syntax error before: '*' [mix_format/syntax]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), BROKEN);

    let applied = dx(&root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), BROKEN);
}

#[test]
fn unicode_source_that_is_already_formatted_passes_untouched() {
    let root = consumer(UNICODE);
    let checked = dx(&root, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    assert_eq!(read(&root.join("lib/lib/math.ex")), UNICODE);
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(DIRTY);
    let out = dx_with(&root, &[], false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("--@rules_dx//config:tool_mix_format"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/lib/math.ex")), DIRTY);
}
