use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str = "defmodule Math do\n  @moduledoc false\n  @spec add(integer, integer) :: integer\n  def add(a, b), do: a + b\nend\n";
const BAD_SPEC: &str = "defmodule Math do\n  @moduledoc false\n  @spec add(integer, integer) :: atom\n  def add(a, b), do: a + b\nend\n";
const STDLIB: &str = "defmodule Math do\n  @moduledoc false\n  require Logger\n\n  @spec total([integer]) :: integer\n  def total(list) do\n    Logger.debug(\"summing\")\n    Enum.sum(Enum.map(list, &(&1 * 2)))\n  end\nend\n";
const WORKER_A: &str = "defmodule A.Worker do\n  @moduledoc false\n  @spec run(integer) :: atom\n  def run(n), do: n + 1\nend\n";
const WORKER_B: &str = "defmodule B.Worker do\n  @moduledoc false\n\n  @spec run(integer) :: atom\n  def run(n), do: n * 2\nend\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "elixir_family",
    family_id = "elixir",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["dialyzer"],
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
    extra_apps = ["logger"],
)
"#;

fn consumer(files: &[(&str, &str)]) -> Case {
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
    for (path, text) in files {
        write(&root.join(path), text);
    }
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
        argv.push("--@rules_dx//config:tool_dialyzer=@rules_dx//erlang/tools:dialyzer");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case) -> Run {
    dx_with(root, true)
}

#[test]
fn clean_spec_passes() {
    let root = consumer(&[("lib/lib/math.ex", CLEAN)]);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn spec_mismatch_reports_the_contract_at_its_location() {
    let root = consumer(&[("lib/lib/math.ex", BAD_SPEC)]);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/lib/math.ex:3:")
            && out.stdout.contains("[dialyzer/invalid_contract]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/lib/math.ex")), BAD_SPEC);
}

#[test]
fn files_sharing_a_name_are_reported_at_their_own_path() {
    let root = consumer(&[
        ("lib/lib/a/worker.ex", WORKER_A),
        ("lib/lib/b/worker.ex", WORKER_B),
    ]);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/lib/a/worker.ex:3:")
            && out.stdout.contains("lib/lib/b/worker.ex:4:"),
        "{}",
        out.combined()
    );
}

#[test]
fn standard_library_calls_resolve_without_unknown_functions() {
    let root = consumer(&[("lib/lib/math.ex", STDLIB)]);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        !out.combined().contains("unknown_function"),
        "{}",
        out.combined()
    );
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(&[("lib/lib/math.ex", CLEAN)]);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_dialyzer"),
        "{}",
        out.combined()
    );
}
