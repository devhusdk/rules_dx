use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const ITEM_DIRTY: &str = "syntax = \"proto3\";\npackage item.v1;\nmessage Item {string name=1;}\n";

const ITEM_FORMATTED: &str =
    "syntax = \"proto3\";\npackage item.v1;\nmessage Item {\n  string name = 1;\n}\n";

const ORDER: &str = "syntax = \"proto3\";\n\npackage order.v1;\n\nimport \"item/v1/item.proto\";\n\nmessage Order {\n  item.v1.Item item = 1;\n}\n";

const MISPLACED: &str =
    "syntax = \"proto3\";\n\npackage bad.v1;\n\nmessage Bad {\n  string id = 1;\n}\n";

const FORMAT_SYNTAX_ERROR: &str = "syntax = \"proto3\";\n\npackage item.v1;\n\nmessage {\n}\n";

const SYNTAX_ERROR: &str = "syntax = \"proto3\";\n\npackage item.v1;\n\nmessage Broken {{{\n";

const BAD_BUILD: &str = r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "bad",
    srcs = ["bad.proto"],
    aspect_hints = HINTS,
)
"#;

const BUF_YAML: &str =
    "version: v2\nlint:\n  use:\n    - STANDARD\n  except:\n    - PACKAGE_DIRECTORY_MATCH\n";

const UNRESOLVED: &str = "syntax = \"proto3\";\n\npackage lost.v1;\n\nimport \"missing/v1/none.proto\";\n\nmessage Lost {\n  string id = 1;\n}\n";

const LOST_BUILD: &str = r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "lost",
    srcs = ["lost.proto"],
)
"#;

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_proto", version = "7.1.0")
bazel_dep(name = "protobuf", version = "34.0.bcr.1")
bazel_dep(name = "rules_cc", version = "0.2.27")
bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:32.0.1-jre"],
    known_contributing_modules = ["protobuf", "rules_dx"],
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "buf_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

buf_config(
    name = "lint_override",
    src = "buf.yaml",
    visibility = ["//visibility:public"],
)

quality_family(
    name = "protobuf_family",
    family_id = "protobuf",
    format = ["buf"],
    lint = ["buf"],
)

workspace_policy(
    name = "policy",
    families = [":protobuf_family"],
)
"#;

const ITEM_BUILD: &str = r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "item",
    srcs = ["item.proto"],
    visibility = ["//visibility:public"],
)
"#;

const ORDER_BUILD: &str = r#"load("@protobuf//bazel:cc_proto_library.bzl", "cc_proto_library")
load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "order",
    srcs = ["order.proto"],
    deps = ["//item/v1:item"],
)

cc_proto_library(
    name = "order_cc_proto",
    tags = ["manual"],
    deps = [":order"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("quality/tests/buf_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch("buf-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("buf.yaml"), BUF_YAML);
    write(&root.join("item/v1/BUILD.bazel"), ITEM_BUILD);
    write(&root.join("item/v1/item.proto"), ITEM_FORMATTED);
    write(&root.join("order/v1/BUILD.bazel"), ORDER_BUILD);
    write(&root.join("order/v1/order.proto"), ORDER);
    root
}

fn dx_with(
    root: &Path,
    command: &str,
    args: &[&str],
    startup: &[&str],
    flags: &[&str],
    envs: &[(&str, &str)],
) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(startup);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    run(&resolve_runfiles(&rel), &argv, envs).unwrap_or_else(|error| panic!("run dx: {error}"))
}

fn dx(root: &Path, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[], &[], &[])
}

fn bazel_build(root: &Path, target: &str) -> std::process::Output {
    std::process::Command::new("bazel")
        .current_dir(root)
        .args(["build", target])
        .output()
        .unwrap_or_else(|error| panic!("run bazel: {error}"))
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let item = root.join("item/v1/item.proto");
    let order = root.join("order/v1/order.proto");
    write(&item, ITEM_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("item/v1/item.proto:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("order/v1/order.proto"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&item), ITEM_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&item), ITEM_FORMATTED);
    assert_eq!(read(&order), ORDER);

    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
}

#[test]
fn lint_resolves_imports_and_reports_rule_and_position() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    write(
        &root.join("bad/BUILD.bazel"),
        &BAD_BUILD.replace("HINTS", "[]"),
    );
    write(&root.join("bad/bad.proto"), MISPLACED);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("bad/bad.proto:3:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("PACKAGE_DIRECTORY_MATCH"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let item = root.join("item/v1/item.proto");
    write(&item, SYNTAX_ERROR);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("item/v1/item.proto:5:"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&item), SYNTAX_ERROR);
}

#[test]
fn a_buf_config_override_replaces_the_default_rules() {
    let root = consumer();
    write(
        &root.join("bad/BUILD.bazel"),
        &BAD_BUILD.replace("HINTS", "[\"//:lint_override\"]"),
    );
    write(&root.join("bad/bad.proto"), MISPLACED);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn an_unresolved_import_is_reported_and_never_clean() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.proto"), UNRESOLVED);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("lost/lost.proto:5:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("imported file does not exist"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_format_time_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let item = root.join("item/v1/item.proto");
    write(&item, FORMAT_SYNTAX_ERROR);
    let out = dx(&root, "format", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("invalid tool output for buf"),
        "{}",
        out.combined()
    );
    dx(&root, "format", &["--apply"]);
    assert_eq!(read(&item), FORMAT_SYNTAX_ERROR);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn the_generation_consumer_builds_before_and_after_apply() {
    let root = consumer();
    let item = root.join("item/v1/item.proto");
    write(&item, ITEM_DIRTY);
    let before = bazel_build(&root, "//order/v1:order_cc_proto");
    assert!(
        before.status.success(),
        "{}",
        String::from_utf8_lossy(&before.stderr)
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let after = bazel_build(&root, "//order/v1:order_cc_proto");
    assert!(
        after.status.success(),
        "{}",
        String::from_utf8_lossy(&after.stderr)
    );
}

#[test]
fn a_warm_cache_runs_with_the_network_denied_and_a_cold_cache_fails() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());
    let denied = [
        ("HTTP_PROXY", "http://127.0.0.1:9"),
        ("HTTPS_PROXY", "http://127.0.0.1:9"),
        ("http_proxy", "http://127.0.0.1:9"),
        ("https_proxy", "http://127.0.0.1:9"),
    ];
    let offline = dx_with(&root, "lint", &[], &[], &[], &denied);
    assert_eq!(offline.status.code(), Some(0), "{}", offline.combined());

    let cold = consumer();
    let base = mkscratch("buf-cold-base").unwrap_or_else(|error| panic!("scratch: {error}"));
    let cache = mkscratch("buf-cold-cache").unwrap_or_else(|error| panic!("scratch: {error}"));
    let startup = format!("--bazel-startup-option=--output_base={}", base.display());
    let cache_flag = format!("--repository_cache={}", cache.display());
    let miss = dx_with(
        &cold,
        "lint",
        &[],
        &[startup.as_str()],
        &[cache_flag.as_str()],
        &denied,
    );
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
}

#[test]
fn imports_resolve_through_strip_import_prefix() {
    let root = consumer();
    write(
        &root.join("lib/BUILD.bazel"),
        r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "lib",
    srcs = ["proto/lib/v1/lib.proto"],
    aspect_hints = ["//:lint_override"],
    strip_import_prefix = "proto",
    visibility = ["//visibility:public"],
)
"#,
    );
    write(
        &root.join("lib/proto/lib/v1/lib.proto"),
        "syntax = \"proto3\";\n\npackage lib.v1;\n\nmessage Lib {\n  string id = 1;\n}\n",
    );
    write(
        &root.join("user/BUILD.bazel"),
        r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "user",
    srcs = ["user.proto"],
    aspect_hints = ["//:lint_override"],
    deps = ["//lib"],
)
"#,
    );
    write(
        &root.join("user/user.proto"),
        "syntax = \"proto3\";\n\npackage user.v1;\n\nimport \"lib/v1/lib.proto\";\n\nmessage User {\n  lib.v1.Lib lib = 1;\n}\n",
    );
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}
