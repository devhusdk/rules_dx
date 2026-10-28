use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "-module(math).\n-export([add/2]).\n\n-spec add(integer(), integer()) -> integer().\nadd(A, B) -> A + B.\n";
const BAD_SPEC: &str = "-module(math).\n-export([add/2]).\n\n-spec add(integer(), integer()) -> atom().\nadd(A, B) -> A + B.\n";
const CRYPTO: &str = "-module(math).\n-export([digest/1]).\n\n-spec digest(binary()) -> binary().\ndigest(Data) -> crypto:hash(sha256, Data).\n";
const BASE: &str = "-module(base).\n-export([double/1]).\n\n-spec double(integer()) -> integer().\ndouble(X) -> X * 2.\n";
const CALLER: &str = "-module(math).\n-export([quad/1]).\n\n-spec quad(integer()) -> integer().\nquad(X) -> base:double(base:double(X)).\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "erlang_family",
    family_id = "erlang",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["dialyzer"],
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

const CRYPTO_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "math",
    app_version = "1.0.0",
    extra_apps = ["crypto"],
)
"#;

const CALLER_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "math",
    app_version = "1.0.0",
    deps = ["//base:erlang_app"],
)
"#;

const BASE_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "base",
    app_version = "1.0.0",
    visibility = ["//visibility:public"],
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

fn consumer(source: &str, lib_build: &str) -> Case {
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
    write(&root.join("lib/BUILD.bazel"), lib_build);
    write(&root.join("lib/src/sub/math.erl"), source);
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

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

#[test]
fn clean_module_passes() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn spec_mismatch_reports_the_contract_at_its_location() {
    let root = consumer(BAD_SPEC, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/src/sub/math.erl:4:")
            && out.stdout.contains("[dialyzer/invalid_contract]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/src/sub/math.erl")), BAD_SPEC);
}

#[test]
fn dependency_apps_resolve_without_unknown_functions() {
    let root = consumer(CALLER, CALLER_BUILD);
    write(&root.join("base/BUILD.bazel"), BASE_BUILD);
    write(&root.join("base/src/base.erl"), BASE);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn extra_otp_apps_resolve() {
    let root = consumer(CRYPTO, CRYPTO_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_dialyzer"),
        "{}",
        out.combined()
    );
}
