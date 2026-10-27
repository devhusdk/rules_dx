use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const FLATBUFFERS_REPO: &str = "--repo=@@rules_dx++http_archive+dx_flatbuffers";

const COMMON: &str = "namespace shop;\n\ntable Money {\n  cents:long;\n}\n";

const ITEM: &str = "include \"money/money.fbs\";\n\nnamespace shop;\n\ntable Item {\n  price:Money;\n  name:string;\n}\n";

const ORDER: &str = "include \"item.fbs\";\n\nnamespace shop;\n\ntable Order {\n  items:[Item];\n}\n\nroot_type Order;\n";

const UNDECLARED_INCLUDE: &str =
    "include \"hidden.fbs\";\n\nnamespace shop;\n\ntable Lost {\n  hidden:Hidden;\n}\n";

const HIDDEN: &str = "namespace shop;\n\ntable Hidden {\n  id:int;\n}\n";

const WARNING: &str = "namespace shop;\n\ntable Loud {\n  camelCase:int;\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

filegroup(
    name = "no_tool",
    srcs = [],
)

quality_family(
    name = "flatbuffers_family",
    family_id = "flatbuffers",
    lint = ["flatc"],
)

workspace_policy(
    name = "policy",
    families = [":flatbuffers_family"],
)
"#;

const MONEY_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "money",
    srcs = ["money.fbs"],
    visibility = ["//visibility:public"],
)
"#;

const SHOP_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "item",
    srcs = ["item.fbs"],
    deps = ["//money"],
    visibility = ["//visibility:public"],
)

flatbuffers_library(
    name = "order",
    srcs = ["order.fbs"],
    deps = [":item"],
)
"#;

const LOST_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "lost",
    srcs = ["lost.fbs"],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
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
    write(&root.join("money/BUILD.bazel"), MONEY_BUILD);
    write(&root.join("money/money.fbs"), COMMON);
    write(&root.join("shop/BUILD.bazel"), SHOP_BUILD);
    write(&root.join("shop/item.fbs"), ITEM);
    write(&root.join("shop/order.fbs"), ORDER);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("flatbuffers-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn a_multi_file_library_with_deps_passes_and_check_keeps_bytes() {
    let root = consumer();
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("money/money.fbs")), COMMON);
    assert_eq!(read(&root.join("shop/item.fbs")), ITEM);
    assert_eq!(read(&root.join("shop/order.fbs")), ORDER);
    assert!(
        !root.join("shop/item_generated.h").exists(),
        "lint must not write generated code"
    );
}

#[test]
fn an_include_that_is_not_a_declared_input_fails() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.fbs"), UNDECLARED_INCLUDE);
    write(&root.join("lost/hidden.fbs"), HIDDEN);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lost/lost.fbs:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("unable to load include file"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_warning_fails_the_lint() {
    let root = consumer();
    write(&root.join("money/money.fbs"), WARNING);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("money/money.fbs:"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_tool_override_without_an_executable_names_the_flag() {
    let root = consumer();
    let out = dx_with(
        &root,
        "lint",
        &[],
        &["--@rules_dx//config:tool_flatc=//:no_tool"],
    );
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("set --@rules_dx//config:tool_flatc to a flatc binary"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_pinned_flatc_source_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch =
        mkscratch("flatbuffers-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", FLATBUFFERS_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        FLATBUFFERS_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("v25.12.19.tar.gz"),
        "{}",
        miss.combined()
    );
}
