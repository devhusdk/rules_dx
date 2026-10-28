use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const DIRTY: &str = "defmodule Math do\n  def add(a, b), do: IO.inspect(a + b)\nend\n";
const CLEAN: &str = "defmodule Math do\n  @moduledoc false\n  def add(a, b), do: a + b\nend\n";
const BROKEN: &str = "defmodule Math do\n  def add(a, b), do: a + * b\nend\n";
/// A stray config above the workspace that would hide every finding if Credo read it.
const STRAY: &str = "%{configs: [%{name: \"default\", files: %{included: [\"nothing/**\"]}}]}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "elixir_family",
    family_id = "elixir",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["credo"],
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
        argv.push("--@rules_dx//config:tool_credo=@rules_dx//elixir/tools:credo");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, true)
}

#[test]
fn findings_are_reported_and_the_source_is_left_alone() {
    let root = consumer(DIRTY);
    let source = root.join("lib/lib/math.ex");

    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked
            .stdout
            .contains("lib/lib/math.ex:2:22: warning: There should be no calls to `IO.inspect/1`. [credo/Warning.IoInspect]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&source), DIRTY);

    let applied = dx(&root, &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&source), DIRTY);
}

#[test]
fn a_clean_file_passes() {
    let root = consumer(CLEAN);
    let checked = dx(&root, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    assert_eq!(read(&root.join("lib/lib/math.ex")), CLEAN);
}

#[test]
fn a_file_credo_cannot_parse_is_never_clean() {
    let root = consumer(BROKEN);
    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked.stdout.contains("lib/lib/math.ex:2:") && checked.stdout.contains("[credo/syntax]"),
        "{}",
        checked.combined()
    );
    assert_eq!(read(&root.join("lib/lib/math.ex")), BROKEN);
}

#[test]
fn a_stray_credo_config_in_a_parent_directory_is_ignored() {
    let root = consumer(DIRTY);
    write(&root.join(".credo.exs"), STRAY);
    write(&root.join("lib/.credo.exs"), STRAY);
    let checked = dx(&root, &[]);
    assert_ne!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        checked.stdout.contains("[credo/Warning.IoInspect]"),
        "{}",
        checked.combined()
    );
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(DIRTY);
    let out = dx_with(&root, &[], false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_credo"),
        "{}",
        out.combined()
    );
}
