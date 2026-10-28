use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const SETTINGS: &str = "retries = 3\ntimeout = \"30s\"\n";

const SETTINGS_DIRTY: &str = "retries   =   3\ntimeout = \"30s\"\n";

const MAIN: &str = "import \"../lib/settings.pkl\"\n\nname = \"app\"\nretries = settings.retries\ntimeout = settings.timeout\n";

const VALUES: &str = "limits {\n  retries = 3\n}\n";

const VALUES_BROKEN: &str = "limits {\n  retries = = 3\n}\n";

const PKL_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_pkl_linux_x86_64";

const FAKE_PKL: &str = "#!/bin/sh\necho fake-pkl-ran >&2\nexit 2\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "pkl_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "pkl",
    format = ["pkl"],
)

workspace_policy(
    name = "policy",
    families = [":pkl_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//pkl/rules:defs.bzl", "pkl_library")

pkl_library(
    name = "lib",
    srcs = ["settings.pkl"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//pkl/rules:defs.bzl", "pkl_library")

pkl_library(
    name = "app",
    srcs = [
        "main.pkl",
        "values.pkl",
    ],
    deps = ["//lib"],
)
"#;

const TOOLS_BUILD: &str = "exports_files([\"override_pkl.sh\"])\n";

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
    write(&root.join("lib/settings.pkl"), SETTINGS);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.pkl"), MAIN);
    write(&root.join("app/values.pkl"), VALUES);
    write(&root.join("tools/BUILD.bazel"), TOOLS_BUILD);
    write(&root.join("tools/override_pkl.sh"), FAKE_PKL);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("pkl-home");
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
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let settings = root.join("lib/settings.pkl");
    write(&settings, SETTINGS_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("lib/settings.pkl:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/main.pkl"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&settings), SETTINGS_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&settings), SETTINGS);
    assert_eq!(read(&root.join("app/main.pkl")), MAIN);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let values = root.join("app/values.pkl");
    write(&values, VALUES_BROKEN);
    write(&root.join("lib/settings.pkl"), SETTINGS_DIRTY);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/values.pkl:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("lib/settings.pkl:"),
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
fn the_declared_pkl_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("pkl-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("lib/settings.pkl"), SETTINGS_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("lib/settings.pkl:"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", PKL_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&["fetch", PKL_REPO, cache_flag.as_str(), denied_flag.as_str()]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("pkl-alpine-linux-amd64"),
        "{}",
        miss.combined()
    );
}

fn pkl_binary(root: &Case) -> PathBuf {
    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    PathBuf::from(base.stdout.trim())
        .join("external/rules_dx++dx_tools+dx_pkl_linux_x86_64/pkl-alpine-linux-amd64")
}

fn evaluate(pkl: &Path, entry: &Path) -> std::process::Output {
    std::process::Command::new(pkl)
        .arg("eval")
        .arg(entry)
        .output()
        .unwrap_or_else(|error| panic!("{pkl:?}: {error}"))
}

#[test]
fn the_pinned_pkl_formats_and_evaluates_the_module_with_its_import() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let pkl = pkl_binary(&root);
    let version = std::process::Command::new(&pkl)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("{pkl:?}: {error}"));
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("Pkl 0.32.1"),
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );

    let entry = root.join("app/main.pkl");
    let evaluated = evaluate(&pkl, &entry);
    assert!(
        evaluated.status.success(),
        "{}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&evaluated.stdout),
        "name = \"app\"\nretries = 3\ntimeout = \"30s\"\n"
    );

    write(
        &root.join("lib/settings.pkl"),
        "retries = 5\ntimeout = \"30s\"\n",
    );
    let changed = evaluate(&pkl, &entry);
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    assert!(
        String::from_utf8_lossy(&changed.stdout).contains("retries = 5"),
        "{}",
        String::from_utf8_lossy(&changed.stdout)
    );
    let still_clean = dx(&root, "format", &[]);
    assert_eq!(
        still_clean.status.code(),
        Some(0),
        "{}",
        still_clean.combined()
    );

    write(&root.join("lib/settings.pkl"), "timeout = \"30s\"\n");
    let missing = evaluate(&pkl, &entry);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("retries"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
}

#[test]
fn a_declared_pkl_executable_replaces_the_pin_and_its_failure_is_never_clean() {
    let root = consumer();
    let fake = "--@rules_dx//config:tool_pkl=//tools:override_pkl.sh";
    let out = dx_with(&root, "format", &[], &[fake]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("fake-pkl-ran"),
        "{}",
        out.combined()
    );

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_pkl_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        fake,
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //app:app)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    assert!(plan.stdout.contains("override_pkl.sh"), "{}", plan.stdout);
    assert!(
        !plan.stdout.contains("dx_pkl_linux_x86_64"),
        "{}",
        plan.stdout
    );
}
