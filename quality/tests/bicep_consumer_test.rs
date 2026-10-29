use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const MAIN: &str = "param name string\nvar settings = loadJsonContent('settings.json')\n\nmodule storage '../lib/storage.bicep' = {\n  name: 'storage'\n  params: {\n    name: name\n  }\n}\n\noutput value string = '${storage.outputs.value}-${settings.suffix}'\n";

const MAIN_DIRTY: &str = "param   name   string\nvar settings = loadJsonContent('settings.json')\n\nmodule storage '../lib/storage.bicep' = {\n  name: 'storage'\n  params: {\n    name: name\n  }\n}\n\noutput value string = '${storage.outputs.value}-${settings.suffix}'\n";

const MAIN_UNUSED: &str = "param name string\nparam unused string = 'x'\nvar settings = loadJsonContent('settings.json')\n\nmodule storage '../lib/storage.bicep' = {\n  name: 'storage'\n  params: {\n    name: name\n  }\n}\n\noutput value string = '${storage.outputs.value}-${settings.suffix}'\n";

const MAIN_BROKEN: &str = "param name string =\n";

const MAIN_REGISTRY: &str = "param name string\n\nmodule remote 'br:mcr.microsoft.com/bicep/avm/res/foo:1.0' = {\n  name: 'remote'\n}\n";

const PARAMS: &str = "using './main.bicep'\n\nparam name = 'dx'\n";

const SETTINGS: &str = "{\"suffix\":\"prod\"}\n";

const STORAGE: &str = "param name string\noutput value string = name\n";

const STORAGE_DIRTY: &str = "param   name   string\noutput value string   =   name\n";

const LEVEL_ERROR: &str = "{\n  \"analyzers\": {\n    \"core\": {\n      \"rules\": {\n        \"no-unused-params\": {\n          \"level\": \"error\"\n        }\n      }\n    }\n  }\n}\n";

const LEVEL_OFF: &str = "{\n  \"analyzers\": {\n    \"core\": {\n      \"rules\": {\n        \"no-unused-params\": {\n          \"level\": \"off\"\n        }\n      }\n    }\n  }\n}\n";

const BICEP_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_bicep_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
include("//third_party/rules_dx:rules_dx.MODULE.bazel")

# rules_dx imports repositories only rules_kotlin 2.4.21 generates.
bazel_dep(name = "rules_kotlin", version = "2.4.21")
bazel_dep(name = "rules_rust", version = "0.75.0")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "bicep_family",
    family_id = "bicep",
    disabled = ["typecheck"],
    format = ["bicep"],
    lint = ["bicep"],
)

workspace_policy(
    name = "policy",
    families = [":bicep_family"],
)
"#;

const CONFIG_BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "bicep_config")

bicep_config(
    name = "cfg",
    src = "bicepconfig.json",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//bicep/rules:defs.bzl", "bicep_module")

bicep_module(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "main.bicep",
        "main.bicepparam",
    ],
    data = ["settings.json"],
    deps = DEPS,
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//bicep/rules:defs.bzl", "bicep_module")

bicep_module(
    name = "mods",
    srcs = ["storage.bicep"],
    visibility = ["//visibility:public"],
)
"#;

fn set_app(root: &Case, hints: &str, deps: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints).replace("DEPS", deps),
    );
}

/// Writes the `third_party/rules_dx` include `dx init` gives every consumer: rules_dx is a
/// non-root module, so its patched-dependency overrides only apply through that include.
fn adopt_overrides(root: &Case) {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let home_dir = scratch("init-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let workspace = root.to_string_lossy().into_owned();
    let init = root.run(
        &resolve_runfiles(&rel),
        &[
            "init",
            "--apply",
            "--output=text",
            "--workspace",
            workspace.as_str(),
        ],
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    );
    assert_eq!(init.status.code(), Some(0), "{}", init.combined());
}

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    adopt_overrides(&root);
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("strict/BUILD.bazel"), CONFIG_BUILD);
    write(&root.join("strict/bicepconfig.json"), LEVEL_ERROR);
    write(&root.join("quiet/BUILD.bazel"), CONFIG_BUILD);
    write(&root.join("quiet/bicepconfig.json"), LEVEL_OFF);
    set_app(&root, "[]", "[\"//lib:mods\"]");
    write(&root.join("app/main.bicep"), MAIN);
    write(&root.join("app/main.bicepparam"), PARAMS);
    write(&root.join("app/settings.json"), SETTINGS);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/storage.bicep"), STORAGE);
    root
}

fn dx_in(
    root: &Case,
    command: &str,
    args: &[&str],
    target: &str,
    flags: &[&str],
    home: &str,
) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        target,
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let home_dir = scratch("bicep-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    dx_in(root, command, args, "//...", flags, home.as_str())
}

fn format(root: &Case) -> Run {
    dx_with(root, "format", &[], &[])
}

fn lint(root: &Case) -> Run {
    dx_with(root, "lint", &[], &[])
}

#[test]
fn modules_params_and_loaded_data_format_and_lint_clean() {
    let root = consumer();
    let formatted = format(&root);
    assert_eq!(formatted.status.code(), Some(0), "{}", formatted.combined());
    let linted = lint(&root);
    assert_eq!(linted.status.code(), Some(0), "{}", linted.combined());
}

#[test]
fn format_check_keeps_bytes_and_apply_rewrites_only_the_owned_file() {
    let root = consumer();
    write(&root.join("app/main.bicep"), MAIN_DIRTY);
    write(&root.join("lib/storage.bicep"), STORAGE_DIRTY);
    let home_dir = scratch("bicep-home");
    let home = home_dir.path().to_string_lossy().into_owned();

    let dirty = dx_in(&root, "format", &[], "//app/...", &[], home.as_str());
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/main.bicep"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&root.join("app/main.bicep")), MAIN_DIRTY);

    let applied = dx_in(
        &root,
        "format",
        &["--apply"],
        "//app/...",
        &[],
        home.as_str(),
    );
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("app/main.bicep")), MAIN);
    assert_eq!(read(&root.join("app/main.bicepparam")), PARAMS);
    assert_eq!(read(&root.join("app/settings.json")), SETTINGS);
    assert_eq!(read(&root.join("lib/storage.bicep")), STORAGE_DIRTY);

    let again = dx_in(&root, "format", &[], "//app/...", &[], home.as_str());
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn an_unused_param_is_located_and_a_bound_config_changes_the_result() {
    let root = consumer();
    write(&root.join("app/main.bicep"), MAIN_UNUSED);
    let default = lint(&root);
    let out = default.combined();
    assert!(out.contains("no-unused-params"), "{out}");
    assert!(out.contains("app/main.bicep:"), "{out}");
    assert_eq!(read(&root.join("app/main.bicep")), MAIN_UNUSED);

    set_app(&root, "[\"//strict:cfg\"]", "[\"//lib:mods\"]");
    let strict = lint(&root);
    assert_eq!(strict.status.code(), Some(1), "{}", strict.combined());
    assert!(
        strict.combined().contains("no-unused-params"),
        "{}",
        strict.combined()
    );

    set_app(&root, "[\"//quiet:cfg\"]", "[\"//lib:mods\"]");
    let quiet = lint(&root);
    assert_eq!(quiet.status.code(), Some(0), "{}", quiet.combined());
    assert!(
        !quiet.combined().contains("no-unused-params"),
        "{}",
        quiet.combined()
    );
}

// `bicep format` exits 0 and echoes a file that does not parse, so only lint can report it.
#[test]
fn a_syntax_error_is_never_clean_in_lint() {
    let root = consumer();
    write(&root.join("app/main.bicep"), MAIN_BROKEN);
    let linted = lint(&root);
    assert_ne!(linted.status.code(), Some(0), "{}", linted.combined());
    assert!(
        linted.combined().contains("app/main.bicep:"),
        "{}",
        linted.combined()
    );
    assert_eq!(read(&root.join("app/main.bicep")), MAIN_BROKEN);
}

#[test]
fn a_module_missing_from_deps_is_an_error_not_clean() {
    let root = consumer();
    set_app(&root, "[]", "[]");
    let missing = lint(&root);
    assert_ne!(missing.status.code(), Some(0), "{}", missing.combined());
    assert!(
        missing.combined().contains("app/main.bicep"),
        "{}",
        missing.combined()
    );
}

#[test]
fn a_registry_module_is_refused_even_when_the_host_has_it_cached() {
    let root = consumer();
    write(&root.join("app/main.bicep"), MAIN_REGISTRY);
    let home_dir = scratch("bicep-home");
    let home = home_dir.path();
    let cached = home.join(".bicep/br/mcr.microsoft.com/bicep$avm$res$foo/1.0$");
    write(&cached.join("main.json"), "{\"resources\":[]}\n");
    write(&cached.join("lock"), "");
    let home = home.to_string_lossy().into_owned();
    let refused = dx_in(&root, "lint", &[], "//...", &[], home.as_str());
    assert_ne!(refused.status.code(), Some(0), "{}", refused.combined());
    assert!(
        refused
            .combined()
            .contains("registry modules are not supported"),
        "{}",
        refused.combined()
    );
}

#[test]
fn the_declared_bicep_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = format(&root);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("bicep-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(
        &denied,
        "rewrite github.com/Azure/bicep/(.*) bicep-blocked.invalid/$1\nallow *\n",
    );
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    write(&root.join("app/main.bicep"), MAIN_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/main.bicep"),
        "{}",
        built.combined()
    );

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        BICEP_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("bicep-linux-x64"),
        "{}",
        miss.combined()
    );
}
