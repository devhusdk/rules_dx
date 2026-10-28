use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const DIRTY: &str = "-module(math).\n-export([add/2]).\n\nadd(A, B) ->\n    io:format(\"~p~n\", [A]),\n    A + B.\n";
const FORMATTED: &str = "-module(math).\n-export([add/2, table/0]).\n\nadd(A, B) -> A + B.\n\ntable() ->\n    #{\n        one => 1,\n        two => [A || A <- [1, 2, 3], A > 1]\n    }.\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "erlang_family",
    family_id = "erlang",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["elvis"],
)

workspace_policy(
    name = "policy",
    families = [":erlang_family"],
)
"#;

const CONFIGURED_BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "elvis_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

elvis_config(
    name = "elvis_config",
    src = "elvis.config",
)

quality_family(
    name = "erlang_family",
    family_id = "erlang",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["elvis"],
)

workspace_policy(
    name = "policy",
    families = [":erlang_family"],
)
"#;

const CONFIGURED_LIB_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "math",
    app_version = "1.0.0",
    aspect_hints = ["//:elvis_config"],
)
"#;

const NO_DEBUG_CALL: &str = "[{config, [#{files => [\"**/*.erl\"], ruleset => erl_files, rules => [{elvis_style, no_debug_call, disable}]}]}].\n";

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

fn consumer(source: &str, build: &str) -> Case {
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
    write(&root.join("BUILD.bazel"), build);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/src/sub/math.erl"), source);
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
        argv.push("--@rules_dx//config:tool_elvis=@rules_dx//erlang/tools:elvis");
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
fn dirty_module_reports_elvis_findings_at_their_location() {
    let root = consumer(DIRTY, BUILD);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains(
            "lib/src/sub/math.erl:5:5: warning: an unexpected debug call to 'io:format/2' was found. [elvis/no_debug_call]"
        ),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/src/sub/math.erl")), DIRTY);
}

#[test]
fn erlfmt_formatted_module_is_clean_under_the_default_config() {
    let root = consumer(FORMATTED, BUILD);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn bound_elvis_config_replaces_the_default() {
    let root = consumer(DIRTY, CONFIGURED_BUILD);
    write(&root.join("elvis.config"), NO_DEBUG_CALL);
    write(&root.join("lib/BUILD.bazel"), CONFIGURED_LIB_BUILD);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(DIRTY, BUILD);
    let out = dx_with(&root, &[], false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_elvis"),
        "{}",
        out.combined()
    );
}
