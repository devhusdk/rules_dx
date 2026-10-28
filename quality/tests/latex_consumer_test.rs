use std::path::PathBuf;

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const HELPERS: &str = "\\newcommand{\\greet}[1]{Hello, #1!}\n";

const HELPERS_DIRTY: &str = "\\begin{center}\n\\greet{x}\n\\end{center}\n";

const HELPERS_FIXED: &str = "\\begin{center}\n\t\\greet{x}\n\\end{center}\n";

const MAIN: &str = "\\documentclass{article}\n\\begin{document}\n\\begin{itemize}\n\t\\item one\n\\end{itemize}\n\\end{document}\n";

const MAIN_DIRTY: &str = "\\documentclass{article}\n\\begin{document}\n\\begin{itemize}\n\\item one\n\\end{itemize}\n\\end{document}\n";

const MAIN_SPACES: &str = "\\documentclass{article}\n\\begin{document}\n\\begin{itemize}\n  \\item one\n\\end{itemize}\n\\end{document}\n";

const UNOWNED: &str = "\\begin{center}\n\\item notes\n\\end{center}\n";

const SPACES: &str = "defaultIndent: \"  \"\n";

const LATEXINDENT_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_latexindent_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "latexindent_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "latex_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "latex",
    format = ["latexindent"],
)

workspace_policy(
    name = "policy",
    families = [":latex_family"],
)

latexindent_config(
    name = "spaces",
    src = "spaces.yaml",
    visibility = ["//visibility:public"],
)
"#;

const CHKTEX_BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "latex_family",
    disabled = ["typecheck"],
    family_id = "latex",
    format = ["latexindent"],
    lint = ["chktex"],
)

workspace_policy(
    name = "policy",
    families = [":latex_family"],
)
"#;

const UNWIRED_BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "latex_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "latex",
    format = ["modfmt"],
)

workspace_policy(
    name = "policy",
    families = [":latex_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//latex/rules:defs.bzl", "latex_library")

latex_library(
    name = "lib",
    srcs = ["helpers.sty"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//latex/rules:defs.bzl", "latex_library")

latex_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = ["main.tex"],
    deps = ["//lib"],
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
    write(&root.join("spaces.yaml"), SPACES);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/helpers.sty"), HELPERS);
    set_hints(&root, "[]");
    write(&root.join("app/main.tex"), MAIN);
    write(&root.join("app/notes.tex"), UNOWNED);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("latex-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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

/// Every file under the workspace, as workspace-relative paths, without the Bazel output links.
fn tree(root: &Case) -> Vec<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("bazel-") || name == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("."), &root.join("."), &mut files);
    files.sort();
    files
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_owned_files_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let helpers = root.join("lib/helpers.sty");
    let main = root.join("app/main.tex");
    let notes = root.join("app/notes.tex");
    write(&helpers, HELPERS_DIRTY);
    write(&main, MAIN_DIRTY);
    let before = tree(&root);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("lib/helpers.sty:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("app/main.tex:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/notes.tex"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&helpers), HELPERS_DIRTY);
    assert_eq!(read(&main), MAIN_DIRTY);
    assert_eq!(tree(&root), before, "check must not add files");

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&helpers), HELPERS_FIXED);
    assert_eq!(read(&main), MAIN);
    assert_eq!(read(&notes), UNOWNED);
    assert_eq!(
        tree(&root),
        before,
        "apply must leave no log or backup files"
    );

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn only_the_dirty_file_of_several_is_reported() {
    let root = consumer();
    write(&root.join("app/main.tex"), MAIN_DIRTY);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/main.tex:"),
        "{}",
        out.combined()
    );
    assert!(
        !out.combined().contains("lib/helpers.sty"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_bound_latexindent_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:spaces\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/main.tex"),
        "{}",
        dirty.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("app/main.tex")), MAIN_SPACES);
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn stray_unbound_settings_next_to_the_sources_are_ignored() {
    let root = consumer();
    for name in [
        "app/localSettings.yaml",
        "app/.localSettings.yaml",
        "app/latexindent.yaml",
        "app/.latexindent.yaml",
        "localSettings.yaml",
    ] {
        write(&root.join(name), SPACES);
    }
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("app/main.tex")), MAIN);
}

#[test]
fn a_policy_selecting_an_unwired_tool_fails_with_the_policy_message() {
    let root = consumer();
    write(&root.join("BUILD.bazel"), UNWIRED_BUILD);
    let out = dx(&root, "format", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("selects format tool 'modfmt' with no wired executable"),
        "{}",
        out.combined()
    );
}

#[test]
fn a_policy_selecting_chktex_names_the_missing_build() {
    let root = consumer();
    write(&root.join("BUILD.bazel"), CHKTEX_BUILD);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains(
            "policy: family 'latex' selects lint tool 'chktex', which has no pinned build on linux_arm64, linux_x86_64, macos_arm64, windows_x86_64; disable 'lint'"
        ),
        "{}",
        out.combined()
    );
}

#[test]
fn the_declared_latexindent_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("latex-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("app/main.tex"), MAIN_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/main.tex:"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", LATEXINDENT_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        LATEXINDENT_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("latexindent-linux"),
        "{}",
        miss.combined()
    );
}

#[test]
fn the_declared_latexindent_is_the_pinned_version() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let base = root.bazel(&["info", "output_base"]);
    assert!(base.status.success(), "{}", base.combined());
    let latexindent = PathBuf::from(base.stdout.trim())
        .join("external/rules_dx++dx_tools+dx_latexindent_linux_x86_64/latexindent-linux");
    let version = std::process::Command::new(&latexindent)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("{latexindent:?}: {error}"));
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("4.0.3"),
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
}
