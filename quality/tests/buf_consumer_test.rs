use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const ITEM_DIRTY: &str = "syntax = \"proto3\";\npackage item.v1;\nmessage Item {string name=1;}\n";

const ITEM_FORMATTED: &str =
    "syntax = \"proto3\";\npackage item.v1;\nmessage Item {\n  string name = 1;\n}\n";

const ORDER: &str = "syntax = \"proto3\";\n\npackage order.v1;\n\nimport \"item/v1/item.proto\";\n\nmessage Order {\n  item.v1.Item item = 1;\n}\n";

const MISPLACED: &str =
    "syntax = \"proto3\";\n\npackage bad.v1;\n\nmessage Bad {\n  string id = 1;\n}\n";

const SYNTAX_ERROR: &str = "syntax = \"proto3\";\n\npackage item.v1;\n\nmessage Broken {{{\n";

const BAD_BUILD: &str = r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "bad",
    srcs = ["bad.proto"],
)
"#;

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_proto", version = "7.1.0")
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

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

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

const ORDER_BUILD: &str = r#"load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "order",
    srcs = ["order.proto"],
    deps = ["//item/v1:item"],
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
    write(&root.join("item/v1/BUILD.bazel"), ITEM_BUILD);
    write(&root.join("item/v1/item.proto"), ITEM_FORMATTED);
    write(&root.join("order/v1/BUILD.bazel"), ORDER_BUILD);
    write(&root.join("order/v1/order.proto"), ORDER);
    root
}

fn dx(root: &Path, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    run(&resolve_runfiles(&rel), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
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

    write(&root.join("bad/BUILD.bazel"), BAD_BUILD);
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
