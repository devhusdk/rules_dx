use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const THRIFT_REPO: &str = "--repo=@@rules_dx++http_archive+dx_thrift";

const COMMON: &str = "namespace cpp money\n\nstruct Money {\n  1: i64 cents,\n}\n";

const ITEM: &str = "namespace cpp shop\n\ninclude \"money/money.thrift\"\n\nstruct Item {\n  1: money.Money price,\n  2: string name,\n}\n";

const ORDER: &str = "namespace cpp shop\n\ninclude \"item.thrift\"\n\nstruct Order {\n  1: list<item.Item> items,\n}\n";

const UNDECLARED_INCLUDE: &str =
    "namespace cpp lost\n\ninclude \"hidden.thrift\"\n\nstruct Lost {\n  1: hidden.Hidden hidden,\n}\n";

const HIDDEN: &str = "namespace cpp lost\n\nstruct Hidden {\n  1: i32 id,\n}\n";

const ABSOLUTE_INCLUDE: &str = "include \"/etc/passwd\"\n\nstruct Escape {\n  1: i32 id,\n}\n";

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
    name = "thrift_family",
    family_id = "thrift",
    lint = ["thrift"],
)

workspace_policy(
    name = "policy",
    families = [":thrift_family"],
)
"#;

const MONEY_BUILD: &str = r#"load("@rules_dx//thrift/rules:defs.bzl", "thrift_library")

thrift_library(
    name = "money",
    srcs = ["money.thrift"],
    visibility = ["//visibility:public"],
)
"#;

const SHOP_BUILD: &str = r#"load("@rules_dx//thrift/rules:defs.bzl", "thrift_library")

thrift_library(
    name = "item",
    srcs = ["item.thrift"],
    deps = ["//money"],
    visibility = ["//visibility:public"],
)

thrift_library(
    name = "order",
    srcs = ["order.thrift"],
    deps = [":item"],
)
"#;

const LOST_BUILD: &str = r#"load("@rules_dx//thrift/rules:defs.bzl", "thrift_library")

thrift_library(
    name = "lost",
    srcs = ["lost.thrift"],
)
"#;

fn workspace(with_schemas: bool) -> Case {
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
    if with_schemas {
        write(&root.join("money/BUILD.bazel"), MONEY_BUILD);
        write(&root.join("money/money.thrift"), COMMON);
        write(&root.join("shop/BUILD.bazel"), SHOP_BUILD);
        write(&root.join("shop/item.thrift"), ITEM);
        write(&root.join("shop/order.thrift"), ORDER);
    }
    root
}

fn consumer() -> Case {
    workspace(true)
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("thrift-home");
    let home = home_dir.path().to_string_lossy().into_owned();
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
    assert_eq!(read(&root.join("money/money.thrift")), COMMON);
    assert_eq!(read(&root.join("shop/item.thrift")), ITEM);
    assert_eq!(read(&root.join("shop/order.thrift")), ORDER);
    assert!(
        !root.join("shop/gen-json").exists() && !root.join("gen-json").exists(),
        "lint must not write generated code"
    );
}

#[test]
fn an_include_that_is_not_a_declared_input_fails() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.thrift"), UNDECLARED_INCLUDE);
    write(&root.join("lost/hidden.thrift"), HIDDEN);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lost/lost.thrift:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined()
            .contains("Could not find include file hidden.thrift"),
        "{}",
        out.combined()
    );
}

#[test]
fn an_absolute_include_outside_the_declared_inputs_fails() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.thrift"), ABSOLUTE_INCLUDE);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("is outside the declared inputs"),
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
        &["--@rules_dx//config:tool_thrift=//:no_tool"],
    );
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("set --@rules_dx//config:tool_thrift to a thrift binary"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_repo_without_thrift_files_never_fetches_the_compiler() {
    let root = workspace(false);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let external = std::path::Path::new(base.stdout.trim()).join("external");
    let fetched = std::fs::read_dir(&external)
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().contains("dx_thrift"))
        })
        .unwrap_or(false);
    assert!(!fetched, "dx_thrift was fetched without .thrift files");
}

#[test]
fn the_pinned_thrift_source_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("thrift-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", THRIFT_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        THRIFT_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("thrift-0.25.0.tar.gz"),
        "{}",
        miss.combined()
    );
}
