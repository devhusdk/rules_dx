use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const TYPES: &str = "module Types\n\nlet prefix = \"hello \"\n";

const GREETER_DIRTY: &str = "module Core\n\nlet greet   name =\n  Types.prefix+name\n";

const GREETER_FORMATTED: &str = "module Core\n\nlet greet name = Types.prefix + name\n";

const LINTABLE: &str = "module Core\n\ntype ExampleInterface =\n    abstract member Print: unit -> unit\n\nlet greet name = Types.prefix + name\n";

const PROGRAM: &str = "module App\n\n[<EntryPoint>]\nlet main _ =\n    Xunit.Assert.NotEmpty(Core.greet \"x\")\n    0\n";

const SYNTAX_ERROR: &str = "module Core\n\nlet greet name = )\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_dotnet", version = "0.22.1")
bazel_dep(name = "rules_jvm_external", version = "7.1")

dotnet = use_extension("@rules_dotnet//dotnet:extensions.bzl", "dotnet")
dotnet.toolchain(dotnet_version = "10.0.201")
use_repo(dotnet, "dotnet_toolchains")

nuget = use_extension("@rules_dx//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
use_repo(nuget, "paket.main")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:32.0.1-jre"],
    known_contributing_modules = ["protobuf", "rules_dx"],
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "fsharplint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "fsharp_family",
    family_id = "fsharp",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["fantomas"],
    lint = ["fsharplint"],
)

workspace_policy(
    name = "policy",
    families = [":fsharp_family"],
)

fsharplint_config(
    name = "relaxed",
    src = "relaxed.fsharplint.json",
    visibility = ["//visibility:public"],
)
"#;

const CORE_BUILD: &str = r#"load("@rules_dx//fsharp/rules:defs.bzl", "fsharp_library")

fsharp_library(
    name = "core",
    aspect_hints = HINTS,
    srcs = [
        "Types.fs",
        "Greeter.fs",
    ],
    visibility = ["//visibility:public"],
    deps = ["@paket.main//fsharp.core"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//fsharp/rules:defs.bzl", "fsharp_binary")

fsharp_binary(
    name = "app",
    srcs = ["Program.fs"],
    deps = [
        "//core",
        "@paket.main//fsharp.core",
        "@paket.main//xunit.v3.assert",
    ],
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
    let marker = resolve_runfiles("quality/tests/fsharp_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
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
    write(
        &root.join("relaxed.fsharplint.json"),
        "{\"interfaceNames\": {\"enabled\": false}}\n",
    );
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[]"),
    );
    write(&root.join("core/Types.fs"), TYPES);
    write(&root.join("core/Greeter.fs"), GREETER_DIRTY);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/Program.fs"), PROGRAM);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with_env(root, command, args, &[])
}

fn dx_with_env(root: &Case, command: &str, args: &[&str], envs: &[(&str, &str)]) -> Run {
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
    root.run(&resolve_runfiles(&rel), &argv, envs)
}

#[test]
fn format_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let greeter = root.join("core/Greeter.fs");
    let program = root.join("app/Program.fs");

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/Greeter.fs:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/Program.fs"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeter), GREETER_DIRTY);
    assert_eq!(read(&program), PROGRAM);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
    assert_eq!(read(&program), PROGRAM);

    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
}

#[test]
fn lint_reports_rule_findings_and_a_bound_config_changes_them() {
    let root = consumer();
    let greeter = root.join("core/Greeter.fs");
    write(&greeter, LINTABLE);

    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/Greeter.fs:"),
        "{}",
        dirty.combined()
    );
    assert!(dirty.combined().contains("FL0036"), "{}", dirty.combined());
    assert_eq!(read(&greeter), LINTABLE);

    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[\"//:relaxed\"]"),
    );
    let relaxed = dx(&root, "lint", &[]);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
}

#[test]
fn a_syntax_error_is_reported_with_its_code_severity_and_line() {
    let root = consumer();
    let greeter = root.join("core/Greeter.fs");
    write(&greeter, SYNTAX_ERROR);

    for command in ["lint", "format"] {
        let out = dx(&root, command, &[]);
        let text = out.combined();
        assert_eq!(out.status.code(), Some(1), "{command}: {text}");
        let line = text
            .lines()
            .find(|line| line.contains("core/Greeter.fs:3:"))
            .unwrap_or_else(|| panic!("{command}: no finding at line 3: {text}"));
        assert!(line.contains("FS0010"), "{command}: {line}");
        assert!(line.to_lowercase().contains("error"), "{command}: {line}");
        assert_eq!(read(&greeter), SYNTAX_ERROR);
    }
}

#[test]
fn a_target_define_turns_on_conditional_code_that_is_then_checked() {
    let root = consumer();
    let greeter = root.join("core/Greeter.fs");
    write(
        &greeter,
        "module Core\n\nlet greet name = Types.prefix + name\n\n#if DX_CHECKED\nlet broken = )\n#endif\n",
    );
    let off = dx(&root, "lint", &[]);
    assert_eq!(off.status.code(), Some(0), "{}", off.combined());

    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD
            .replace("HINTS", "[]")
            .replace("srcs = [", "defines = [\"DX_CHECKED\"],\n    srcs = ["),
    );
    let on = dx(&root, "lint", &[]);
    assert_eq!(on.status.code(), Some(1), "{}", on.combined());
    assert!(
        on.combined().contains("core/Greeter.fs:6:"),
        "{}",
        on.combined()
    );
}

#[test]
fn an_unreadable_config_fails_the_run_and_never_counts_as_clean() {
    let root = consumer();
    write(&root.join("relaxed.fsharplint.json"), "{ not json");
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[\"//:relaxed\"]"),
    );
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().to_lowercase().contains("fsharplint"),
        "{}",
        out.combined()
    );
}

fn path_without_dotnet(path: &str, shadow: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for dir in std::env::split_paths(path) {
        if !dir.join("dotnet").exists() {
            dirs.push(dir);
            continue;
        }
        std::fs::create_dir_all(shadow).unwrap_or_else(|error| panic!("shadow: {error}"));
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("PATH dir: {error}"));
        for entry in entries.flatten() {
            let link = shadow.join(entry.file_name());
            if entry.file_name() != "dotnet" && std::fs::symlink_metadata(&link).is_err() {
                std::os::unix::fs::symlink(entry.path(), &link)
                    .unwrap_or_else(|error| panic!("shadow link: {error}"));
            }
        }
        if !dirs.iter().any(|known| known == shadow) {
            dirs.push(shadow.to_path_buf());
        }
    }
    dirs
}

#[test]
fn tools_run_without_dotnet_on_path_and_install_nothing() {
    let root = consumer();
    let home = mkscratch("fsharp-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let packages = home.join("nuget");
    let path = std::env::var("PATH").unwrap_or_default();
    let clean = path_without_dotnet(&path, &home.join("path"));
    let joined = std::env::join_paths(clean).unwrap_or_else(|error| panic!("PATH: {error}"));
    let joined = joined.to_string_lossy().into_owned();
    let home_text = home.to_string_lossy().into_owned();
    let packages_text = packages.to_string_lossy().into_owned();
    let envs = [
        ("PATH", joined.as_str()),
        ("DOTNET_ROOT", "/nonexistent"),
        ("DOTNET_CLI_HOME", home_text.as_str()),
        ("NUGET_PACKAGES", packages_text.as_str()),
    ];

    let dirty = dx_with_env(&root, "format", &[], &envs);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/Greeter.fs:"),
        "{}",
        dirty.combined()
    );
    write(&root.join("core/Greeter.fs"), LINTABLE);
    let linted = dx_with_env(&root, "lint", &[], &envs);
    assert_eq!(linted.status.code(), Some(1), "{}", linted.combined());
    assert!(
        linted.combined().contains("FL0036"),
        "{}",
        linted.combined()
    );
    assert!(!home.join(".dotnet").exists());
    assert!(!home.join(".dotnet/tools").exists());
    assert!(!packages.exists());
}
