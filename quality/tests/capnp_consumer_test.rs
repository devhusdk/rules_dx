use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CAPNP_REPO: &str = "--repo=@@rules_dx++http_archive+dx_capnproto";

const COMMON: &str = "@0xdbb9ad1f14bf0b36;\n\nstruct Money {\n  cents @0 :Int64;\n}\n";

const ITEM: &str = "@0xd0b9ad1f14bf0b37;\n\nusing Cxx = import \"/capnp/c++.capnp\";\n$Cxx.namespace(\"shop\");\n\nusing Money = import \"/money/money.capnp\";\n\nstruct Item {\n  price @0 :Money.Money;\n  name @1 :Text;\n}\n";

const ORDER: &str = "@0xd1b9ad1f14bf0b38;\n\nusing Item = import \"item.capnp\";\n\nstruct Order {\n  items @0 :List(Item.Item);\n}\n";

const UNDECLARED_IMPORT: &str =
    "@0xd2b9ad1f14bf0b39;\n\nusing Hidden = import \"hidden.capnp\";\n\nstruct Lost {\n  hidden @0 :Hidden.Hidden;\n}\n";

const HIDDEN: &str = "@0xd3b9ad1f14bf0b3a;\n\nstruct Hidden {\n  id @0 :Int32;\n}\n";

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
    name = "capnp_family",
    family_id = "capnp",
    lint = ["capnp"],
)

workspace_policy(
    name = "policy",
    families = [":capnp_family"],
)
"#;

const MONEY_BUILD: &str = r#"load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "money",
    srcs = ["money.capnp"],
    visibility = ["//visibility:public"],
)
"#;

const SHOP_BUILD: &str = r#"load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "item",
    srcs = ["item.capnp"],
    deps = ["//money"],
    visibility = ["//visibility:public"],
)

capnp_library(
    name = "order",
    srcs = ["order.capnp"],
    deps = [":item"],
)
"#;

const LOST_BUILD: &str = r#"load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "lost",
    srcs = ["lost.capnp"],
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
        write(&root.join("money/money.capnp"), COMMON);
        write(&root.join("shop/BUILD.bazel"), SHOP_BUILD);
        write(&root.join("shop/item.capnp"), ITEM);
        write(&root.join("shop/order.capnp"), ORDER);
    }
    root
}

fn consumer() -> Case {
    workspace(true)
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("capnp-home");
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
    assert_eq!(read(&root.join("money/money.capnp")), COMMON);
    assert_eq!(read(&root.join("shop/item.capnp")), ITEM);
    assert_eq!(read(&root.join("shop/order.capnp")), ORDER);
    assert!(
        !root.join("shop/item.capnp.h").exists(),
        "lint must not write generated code"
    );
}

#[test]
fn an_import_that_is_not_a_declared_input_fails() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.capnp"), UNDECLARED_IMPORT);
    write(&root.join("lost/hidden.capnp"), HIDDEN);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lost/lost.capnp:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("Import failed: hidden.capnp"),
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
        &["--@rules_dx//config:tool_capnp=//:no_tool"],
    );
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("set --@rules_dx//config:tool_capnp to a capnp binary"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_repo_without_capnp_files_never_fetches_the_compiler() {
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
                .any(|entry| entry.file_name().to_string_lossy().contains("dx_capnproto"))
        })
        .unwrap_or(false);
    assert!(!fetched, "dx_capnproto was fetched without .capnp files");
}

#[test]
fn the_pinned_capnp_source_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("capnp-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", CAPNP_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        CAPNP_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("v1.5.0.tar.gz"),
        "{}",
        miss.combined()
    );
}
