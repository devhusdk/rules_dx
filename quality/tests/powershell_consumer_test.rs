use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const GREETING_PSM1: &str = "<#\n.SYNOPSIS\nBuilds a greeting.\n#>\nfunction Get-Greeting {\n    [CmdletBinding()]\n    [OutputType([string])]\n    param(\n        [string]$Name = 'powershell'\n    )\n    return \"hello $Name\"\n}\n\nExport-ModuleMember -Function Get-Greeting\n";

const GREETING_PSD1: &str = "@{\n    RootModule = 'Greeting.psm1'\n    ModuleVersion = '0.1.0'\n    GUID = '6f1f5a0e-3b0e-4c53-9d57-5f1d2f7c9a10'\n    FunctionsToExport = @('Get-Greeting')\n}\n";

const FORMAT_PSM1: &str = "<#\n.SYNOPSIS\nWraps text in a banner.\n#>\nfunction Format-Banner {\n    [CmdletBinding()]\n    [OutputType([string])]\n    param(\n        [Parameter(Mandatory)]\n        [string]$Text\n    )\n    return \"== $Text ==\"\n}\n\nExport-ModuleMember -Function Format-Banner\n";

const FORMAT_PSD1: &str = "@{\n    RootModule = 'Banner.psm1'\n    ModuleVersion = '0.1.0'\n    GUID = '0b1b8d8a-52a3-44f4-93d4-9a3f2f2f2d11'\n    FunctionsToExport = @('Format-Banner')\n    RequiredModules = @('Greeting')\n}\n";

const MAIN_PS1: &str = "using module Greeting\nImport-Module Banner\nWrite-Output (Format-Banner -Text (Get-Greeting -Name 'dx'))\n";

const TESTS_PS1: &str = "Import-Module Pester -RequiredVersion 5.7.1\nImport-Module Greeting\n\n$config = New-PesterConfiguration\n$config.Run.Path = Join-Path $PSScriptRoot 'Greeting.Describe.ps1'\n$config.Run.PassThru = $true\n$result = Invoke-Pester -Configuration $config\nif ($result.TotalCount -lt 1 -or $result.FailedCount -gt 0) { exit 1 }\nexit 0\n";

const DESCRIBE_PS1: &str = "Describe 'Get-Greeting' {\n    It 'greets by name' {\n        Get-Greeting -Name 'dx' | Should -Be 'hello dx'\n    }\n}\n";

const GALLERY_BUILD: &str = r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_binary")

pwsh_binary(
    name = "gallery",
    srcs = ["Gallery.ps1"],
    deps = ["@rules_dx//powershell/gallery:pester"],
)
"#;

const GALLERY_PS1: &str = "using module Pester\nWrite-Output 'gallery'\n";

const BAD_PS1: &str = "Write-Host 'hello'\n";

const BROKEN_PS1: &str = "function Broken {\n    Write-Output 'open'\n";

const OVERRIDE_PSD1: &str = "@{\n    ExcludeRules = @('PSAvoidUsingWriteHost')\n}\n";

const UNKNOWN_RULE_PSD1: &str = "@{\n    ExcludeRules = @('PSNoSuchRuleAnywhere')\n}\n";

const CLOSURE_REPOS: [&str; 3] = [
    "--repo=@@rules_dx++psgallery_module+psgallery_pester",
    "--repo=@@rules_dx++psgallery_module+psgallery_psscriptanalyzer",
    "--repo=@@rules_powershell++powershell+dx_powershell_toolchains__linux_x64",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "psscriptanalyzer_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

psscriptanalyzer_config(
    name = "lint_override",
    src = "PSScriptAnalyzerSettings.psd1",
    visibility = ["//visibility:public"],
)

quality_family(
    name = "powershell_family",
    family_id = "powershell",
    lint = ["psscriptanalyzer"],
)

workspace_policy(
    name = "policy",
    families = [":powershell_family"],
)
"#;

const GREETING_BUILD: &str = r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_library", "pwsh_test")

pwsh_library(
    name = "greeting",
    srcs = [
        "Greeting.psd1",
        "Greeting.psm1",
    ],
    visibility = ["//visibility:public"],
)

pwsh_test(
    name = "greeting_test",
    srcs = ["Greeting.Tests.ps1"],
    data = ["Greeting.Describe.ps1"],
    deps = [
        ":greeting",
        "@rules_dx//powershell/gallery:pester",
    ],
)
"#;

const BANNER_BUILD: &str = r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_library")

pwsh_library(
    name = "banner",
    srcs = [
        "Banner.psd1",
        "Banner.psm1",
    ],
    visibility = ["//visibility:public"],
    deps = ["//Greeting:greeting"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_binary")

pwsh_binary(
    name = "app",
    srcs = ["Main.ps1"],
    deps = [
        "//Banner:banner",
        "//Greeting:greeting",
    ],
)
"#;

const BAD_BUILD: &str = r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_binary")

pwsh_binary(
    name = "bad",
    srcs = ["bad.ps1"],
    aspect_hints = HINTS,
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
    let marker = resolve_runfiles("quality/tests/powershell_consumer_test.rs");
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
    write(&root.join("PSScriptAnalyzerSettings.psd1"), OVERRIDE_PSD1);
    write(&root.join("Greeting/BUILD.bazel"), GREETING_BUILD);
    write(&root.join("Greeting/Greeting.psd1"), GREETING_PSD1);
    write(&root.join("Greeting/Greeting.psm1"), GREETING_PSM1);
    write(&root.join("Greeting/Greeting.Tests.ps1"), TESTS_PS1);
    write(&root.join("Greeting/Greeting.Describe.ps1"), DESCRIBE_PS1);
    write(&root.join("Banner/BUILD.bazel"), BANNER_BUILD);
    write(&root.join("Banner/Banner.psd1"), FORMAT_PSD1);
    write(&root.join("Banner/Banner.psm1"), FORMAT_PSM1);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/Main.ps1"), MAIN_PS1);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("powershell-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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

fn add_bad(root: &Case, text: &str, hint: &str) {
    write(
        &root.join("bad/BUILD.bazel"),
        &BAD_BUILD.replace("HINTS", hint),
    );
    write(&root.join("bad/bad.ps1"), text);
}

#[test]
fn clean_modules_pass_and_the_application_and_pester_tests_run() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    let ran = root.bazel(&["run", "//app:app"]);
    assert!(ran.status.success(), "{}", ran.combined());
    assert!(
        ran.combined().contains("== hello dx =="),
        "{}",
        ran.combined()
    );
    let tested = root.bazel(&["test", "//Greeting:greeting_test"]);
    assert!(tested.status.success(), "{}", tested.combined());
}

#[test]
fn using_module_of_an_external_dependency_resolves_in_lint() {
    let root = consumer();
    write(&root.join("gallery/BUILD.bazel"), GALLERY_BUILD);
    write(&root.join("gallery/Gallery.ps1"), GALLERY_PS1);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_dirty_script_is_reported_with_rule_and_position() {
    let root = consumer();
    add_bad(&root, BAD_PS1, "[]");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("bad/bad.ps1:1:1"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("PSAvoidUsingWriteHost"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_psscriptanalyzer_config_override_replaces_the_default_rules() {
    let root = consumer();
    add_bad(&root, BAD_PS1, "[\"//:lint_override\"]");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_script_that_does_not_parse_is_reported_and_never_clean() {
    let root = consumer();
    add_bad(&root, BROKEN_PS1, "[]");
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("bad/bad.ps1:"),
        "{}",
        out.combined()
    );
}

#[test]
fn an_invalid_config_is_reported_and_never_clean() {
    let root = consumer();
    write(
        &root.join("PSScriptAnalyzerSettings.psd1"),
        UNKNOWN_RULE_PSD1,
    );
    add_bad(&root, BAD_PS1, "[\"//:lint_override\"]");
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("PSNoSuchRuleAnywhere"),
        "{}",
        out.combined()
    );
}

#[test]
fn lint_apply_never_rewrites_a_script() {
    let root = consumer();
    add_bad(&root, BAD_PS1, "[]");
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("bad/bad.ps1")), BAD_PS1);
}

#[test]
fn the_declared_psscriptanalyzer_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch =
        mkscratch("psscriptanalyzer-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    add_bad(&root, BAD_PS1, "[]");
    let linted = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(linted.status.code(), Some(1), "{}", linted.combined());
    assert!(
        linted.combined().contains("PSAvoidUsingWriteHost"),
        "{}",
        linted.combined()
    );
    let tested = root.bazel(&["test", "//Greeting:greeting_test", denied_flag.as_str()]);
    assert!(tested.status.success(), "{}", tested.combined());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut fetch: Vec<&str> = vec!["fetch"];
    fetch.extend(CLOSURE_REPOS);
    fetch.push(denied_flag.as_str());
    let offline = root.bazel(&fetch);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut cold: Vec<&str> = vec!["fetch"];
    cold.extend(CLOSURE_REPOS);
    cold.push(cache_flag.as_str());
    cold.push(denied_flag.as_str());
    let miss = root.bazel(&cold);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("Cache miss"),
        "{}",
        miss.combined()
    );
}
