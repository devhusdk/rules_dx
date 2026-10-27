use std::path::PathBuf;

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const GREETING: &str =
    "package lib\n\n#Greeting: {\n\tname:    string\n\tmessage: \"hello \\(name)\"\n}\n";

const GREETING_DIRTY: &str =
    "package lib\n\n#Greeting: {\n\tname: string\n\tmessage:   \"hello \\(name)\"\n}\n";

const CUE_MODULE: &str = "module: \"example.com/app\"\nlanguage: version: \"v0.17.0\"\n";

const MAIN: &str = "package app\n\nimport \"example.com/app/lib\"\n\ngreeting: lib.#Greeting & {\n\tname: \"world\"\n}\n";

const VALUES: &str = "package app\n\nlimits: {\n\tretries: 3\n}\n";

const VALUES_BROKEN: &str = "package app\n\nlimits: {\n\tretries: 3\n";

const CUE_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_cue_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//cue/rules:defs.bzl", "cue_library")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

cue_library(
    name = "module",
    srcs = ["cue.mod/module.cue"],
    visibility = ["//visibility:public"],
)

quality_family(
    name = "cue_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "cue",
    format = ["cue"],
)

workspace_policy(
    name = "policy",
    families = [":cue_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//cue/rules:defs.bzl", "cue_library")

cue_library(
    name = "lib",
    srcs = ["greeting.cue"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//cue/rules:defs.bzl", "cue_library")

cue_library(
    name = "app",
    srcs = [
        "main.cue",
        "values.cue",
    ],
    deps = [
        "//:module",
        "//lib",
    ],
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
    write(&root.join("cue.mod/module.cue"), CUE_MODULE);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/greeting.cue"), GREETING);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.cue"), MAIN);
    write(&root.join("app/values.cue"), VALUES);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("cue-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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

    let greeting = root.join("lib/greeting.cue");
    write(&greeting, GREETING_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("lib/greeting.cue:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/main.cue"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeting), GREETING_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeting), GREETING);
    assert_eq!(read(&root.join("app/main.cue")), MAIN);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let values = root.join("app/values.cue");
    write(&values, VALUES_BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/values.cue:"),
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
fn the_declared_cue_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("cue-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("lib/greeting.cue"), GREETING_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("lib/greeting.cue:"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", CUE_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&["fetch", CUE_REPO, cache_flag.as_str(), denied_flag.as_str()]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("cue_v0.17.1_linux_amd64"),
        "{}",
        miss.combined()
    );
}

#[test]
fn the_declared_module_is_real_and_its_imports_reach_the_format_action() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let cue = PathBuf::from(base.stdout.trim())
        .join("external/rules_dx++dx_tools+dx_cue_linux_x86_64/cue");
    let vet = std::process::Command::new(&cue)
        .args(["vet", "./..."])
        .current_dir(&*root)
        .output()
        .unwrap_or_else(|error| panic!("{cue:?}: {error}"));
    assert!(
        vet.status.success(),
        "{}{}",
        String::from_utf8_lossy(&vet.stdout),
        String::from_utf8_lossy(&vet.stderr)
    );

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_cue_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //app:app)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    for staged in ["cue.mod/module.cue", "lib/greeting.cue", "app/main.cue"] {
        assert!(
            plan.stdout.contains(staged),
            "missing {staged}: {}",
            plan.stdout
        );
    }
}
