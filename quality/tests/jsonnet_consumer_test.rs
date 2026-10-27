use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const SETTINGS: &str = "{\n  retries: 3,\n  timeout: '30s',\n}\n";

const SETTINGS_DIRTY: &str = "{\n  retries:   3,\n  timeout: '30s',\n}\n";

const MAIN: &str = "local settings = import '../lib/settings.libsonnet';\n\n{\n  name: 'app',\n  retries: settings.retries,\n}\n";

const VALUES: &str = "{\n  limits: { retries: 3 },\n}\n";

const VALUES_BROKEN: &str = "{\n  limits: [1, 2,\n";

const JSONNETFMT_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_jsonnetfmt_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "jsonnet_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "jsonnet",
    format = ["jsonnetfmt"],
)

workspace_policy(
    name = "policy",
    families = [":jsonnet_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//jsonnet/rules:defs.bzl", "jsonnet_library")

jsonnet_library(
    name = "lib",
    srcs = ["settings.libsonnet"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//jsonnet/rules:defs.bzl", "jsonnet_library")

jsonnet_library(
    name = "app",
    srcs = [
        "main.jsonnet",
        "values.jsonnet",
    ],
    deps = ["//lib"],
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
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/settings.libsonnet"), SETTINGS);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.jsonnet"), MAIN);
    write(&root.join("app/values.jsonnet"), VALUES);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("jsonnet-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let settings = root.join("lib/settings.libsonnet");
    write(&settings, SETTINGS_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("lib/settings.libsonnet:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/main.jsonnet"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&settings), SETTINGS_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&settings), SETTINGS);
    assert_eq!(read(&root.join("app/main.jsonnet")), MAIN);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let values = root.join("app/values.jsonnet");
    write(&values, VALUES_BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/values.jsonnet:"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&values), VALUES_BROKEN);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn the_declared_jsonnetfmt_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("jsonnet-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("lib/settings.libsonnet"), SETTINGS_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("lib/settings.libsonnet:"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", JSONNETFMT_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        JSONNETFMT_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("go-jsonnet_0.22.0_linux_amd64"),
        "{}",
        miss.combined()
    );
}

#[test]
fn the_library_import_reaches_the_format_action() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let jsonnetfmt = PathBuf::from(base.stdout.trim())
        .join("external/rules_dx++dx_tools+dx_jsonnetfmt_linux_x86_64/jsonnetfmt");
    let version = std::process::Command::new(&jsonnetfmt)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("{jsonnetfmt:?}: {error}"));
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("v0.22.0"),
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_jsonnet_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //app:app)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    for staged in ["lib/settings.libsonnet", "app/main.jsonnet"] {
        assert!(
            plan.stdout.contains(staged),
            "missing {staged}: {}",
            plan.stdout
        );
    }
}

fn evaluate(root: &Case, library_dir: &Path, entry: &Path, out: &Path) -> std::process::Output {
    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let jsonnet = PathBuf::from(base.stdout.trim())
        .join("external/rules_dx++dx_tools+dx_jsonnetfmt_linux_x86_64/dx_extracted/jsonnet");
    std::process::Command::new(&jsonnet)
        .arg("-J")
        .arg(library_dir)
        .arg("-o")
        .arg(out)
        .arg(entry)
        .output()
        .unwrap_or_else(|error| panic!("{jsonnet:?}: {error}"))
}

#[test]
fn the_entry_evaluates_with_its_library_to_the_checked_configuration() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("jsonnet-eval").unwrap_or_else(|error| panic!("scratch: {error}"));
    let out = scratch.join("app.json");
    let evaluated = evaluate(
        &root,
        &root.join("lib"),
        &root.join("app/main.jsonnet"),
        &out,
    );
    assert!(
        evaluated.status.success(),
        "{}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    assert_eq!(
        read(&out),
        "{\n   \"name\": \"app\",\n   \"retries\": 3\n}\n"
    );

    write(
        &root.join("lib/settings.libsonnet"),
        "{\n  retries: 5,\n  timeout: '30s',\n}\n",
    );
    let changed = evaluate(
        &root,
        &root.join("lib"),
        &root.join("app/main.jsonnet"),
        &out,
    );
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    assert_eq!(
        read(&out),
        "{\n   \"name\": \"app\",\n   \"retries\": 5\n}\n"
    );

    write(
        &root.join("app/main.jsonnet"),
        "import 'missing.libsonnet'\n",
    );
    let missing = evaluate(
        &root,
        &root.join("lib"),
        &root.join("app/main.jsonnet"),
        &out,
    );
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("missing.libsonnet"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
}
