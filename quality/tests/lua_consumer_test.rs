use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const GREET: &str =
    "local M = {}\n\nfunction M.greet(name)\n\treturn \"hello \" .. name\nend\n\nreturn M\n";

const GREET_DIRTY: &str =
    "local M = {}\n\nfunction M.greet( name )\n\treturn   \"hello \" .. name\nend\n\nreturn M\n";

const UTIL: &str = "local M = {}\n\nM.answer = 42\n\nreturn M\n";

const BROKEN: &str = "local = = 1\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "stylua_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "lua_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "lua",
    format = ["stylua"],
)

workspace_policy(
    name = "policy",
    families = [":lua_family"],
)

stylua_config(
    name = "spaces",
    src = "spaces.toml",
    visibility = ["//visibility:public"],
)
"#;

const SPACES: &str = "indent_type = \"Spaces\"\nindent_width = 2\n";

const APP_BUILD: &str = r#"load("@rules_dx//lua/rules:defs.bzl", "lua_library")

lua_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "greet.lua",
        "util.lua",
    ],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints),
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
    write(&root.join("spaces.toml"), SPACES);
    set_hints(&root, "[]");
    write(&root.join("app/greet.lua"), GREET);
    write(&root.join("app/util.lua"), UTIL);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("lua-home");
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
fn a_clean_tree_passes_a_dirty_file_is_reported_and_apply_repairs_it() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let greet = root.join("app/greet.lua");
    write(&greet, GREET_DIRTY);
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/greet.lua:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/util.lua"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greet), GREET_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greet), GREET);
    assert_eq!(read(&root.join("app/util.lua")), UTIL);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_located_and_never_clean() {
    let root = consumer();
    let util = root.join("app/util.lua");
    write(&util, BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/util.lua:1:"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&util), BROKEN);
}

#[test]
fn a_bound_stylua_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:spaces\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/greet.lua"),
        "{}",
        dirty.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(
        read(&root.join("app/greet.lua")).contains("\n  return \"hello \" .. name\n"),
        "{}",
        read(&root.join("app/greet.lua"))
    );
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn stray_unbound_configs_next_to_the_sources_are_ignored() {
    let root = consumer();
    write(&root.join("app/stylua.toml"), SPACES);
    write(&root.join("app/.stylua.toml"), SPACES);
    write(&root.join("stylua.toml"), SPACES);
    write(
        &root.join("app/.editorconfig"),
        "root = true\n\n[*.lua]\nindent_style = space\nindent_size = 8\n",
    );
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("app/greet.lua")), GREET);
}
