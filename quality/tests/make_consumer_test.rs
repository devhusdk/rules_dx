use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str =
    ".PHONY: all clean test\nall:\n\t@echo all\nclean:\n\t@echo clean\ntest:\n\t@echo test\n";

// Missing .PHONY declarations: the default minphony rule reports it.
const NO_PHONY: &str = "all:\n\t@echo all\nclean:\n\t@echo clean\ntest:\n\t@echo test\n";

const RULES_MK: &str = ".PHONY: sub\nsub:\n\t@echo sub\n";

const UNOWNED: &str = "all:\n\t@echo unowned\n";

const NO_MINPHONY: &str = "[minphony]\ndisabled=true\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//make/rules:defs.bzl", "make_library")
load("@rules_dx//quality:native_config.bzl", "checkmake_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "make_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "make",
    lint = ["checkmake"],
)

workspace_policy(
    name = "policy",
    families = [":make_family"],
)

checkmake_config(
    name = "no_minphony",
    src = "no_minphony.ini",
    visibility = ["//visibility:public"],
)

make_library(
    name = "root",
    srcs = [
        "GNUmakefile",
        "Makefile",
    ],
)
"#;

const SUB_BUILD: &str = r#"load("@rules_dx//make/rules:defs.bzl", "make_library")

make_library(
    name = "sub",
    srcs = ["rules.mk"],
)
"#;

const CONFIGURED_BUILD: &str = r#"load("@rules_dx//make/rules:defs.bzl", "make_library")

make_library(
    name = "configured",
    aspect_hints = HINTS,
    srcs = ["Makefile"],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("configured/BUILD.bazel"),
        &CONFIGURED_BUILD.replace("HINTS", hints),
    );
}

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
    write(&root.join("no_minphony.ini"), NO_MINPHONY);
    write(&root.join("Makefile"), CLEAN);
    write(&root.join("GNUmakefile"), CLEAN);
    write(&root.join("sub/BUILD.bazel"), SUB_BUILD);
    write(&root.join("sub/rules.mk"), RULES_MK);
    write(&root.join("configured/Makefile"), CLEAN);
    set_hints(&root, "[]");
    // No BUILD file in this directory: no make_library owns the file.
    write(&root.join("vendor/Makefile"), UNOWNED);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("make-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn a_clean_tree_passes_and_findings_name_the_owned_file_only() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    write(&root.join("GNUmakefile"), NO_PHONY);
    write(&root.join("sub/rules.mk"), NO_PHONY);
    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("GNUmakefile:") && dirty.combined().contains("rules.mk:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("vendor/"),
        "{}",
        dirty.combined()
    );
}

#[test]
fn apply_is_check_only_and_changes_nothing() {
    let root = consumer();
    let makefile = root.join("Makefile");
    write(&makefile, NO_PHONY);
    let out = dx(&root, "lint", &["--apply"]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert_eq!(read(&makefile), NO_PHONY);
    assert_eq!(read(&root.join("GNUmakefile")), CLEAN);
    assert_eq!(read(&root.join("sub/rules.mk")), RULES_MK);
    assert_eq!(read(&root.join("vendor/Makefile")), UNOWNED);
}

#[test]
fn a_bound_checkmake_config_changes_the_findings() {
    let root = consumer();
    write(&root.join("configured/Makefile"), NO_PHONY);
    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("configured/Makefile"),
        "{}",
        dirty.combined()
    );
    set_hints(&root, "[\"//:no_minphony\"]");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn stray_unbound_configs_next_to_the_sources_are_ignored() {
    let root = consumer();
    write(&root.join("configured/Makefile"), NO_PHONY);
    write(&root.join("configured/checkmake.ini"), NO_MINPHONY);
    write(&root.join("checkmake.ini"), NO_MINPHONY);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("configured/Makefile"),
        "{}",
        out.combined()
    );
}
