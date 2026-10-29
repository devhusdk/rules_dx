use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const VALE_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_vale_linux_x86_64";

const CLEAN_RST: &str = "Guide\n=====\n\nA clean paragraph.\n";
const CLEAN_ADOC: &str = "= Guide\n\nA clean paragraph.\n";
const DIRTY_RST: &str = "Guide\n=====\n\nTODO write this paragraph.\n";
const DIRTY_ADOC: &str = "= Guide\n\nTODO write this paragraph.\n";
const DRAFT_RST: &str = "Guide\n=====\n\nDRAFT paragraph.\n";
const DRAFT_ADOC: &str = "= Guide\n\nDRAFT paragraph.\n";

const TYPST: &str = "= Title\n\nBody.\n";
const LATEX: &str = "\\documentclass{article}\n\\begin{document}\nHello.\n\\end{document}\n";

const MARKERS: &str =
    "extends: existence\nmessage: \"Remove '%s' markers.\"\nlevel: error\ntokens:\n  - TODO\n";
const DRAFTS: &str =
    "extends: existence\nmessage: \"Remove '%s' drafts.\"\nlevel: error\ntokens:\n  - DRAFT\n";
const MARKERS_INI: &str =
    "StylesPath = styles\nMinAlertLevel = suggestion\n\n[*]\nBasedOnStyles = Dx\n";
const DRAFTS_INI: &str =
    "StylesPath = styles_draft\nMinAlertLevel = suggestion\n\n[*]\nBasedOnStyles = Dx\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "vale_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "rst_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "rst",
    lint = ["vale"],
)

quality_family(
    name = "asciidoc_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "asciidoc",
    lint = ["vale"],
)

workspace_policy(
    name = "policy",
    families = [
        ":asciidoc_family",
        ":rst_family",
    ],
)

vale_config(
    name = "markers",
    src = "markers.ini",
    data = ["styles/Dx/Markers.yml"],
    visibility = ["//visibility:public"],
)

vale_config(
    name = "drafts",
    src = "drafts.ini",
    data = ["styles_draft/Dx/Markers.yml"],
    visibility = ["//visibility:public"],
)
"#;

const UNWIRED_BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "rst_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "rst",
    lint = ["modlint"],
)

workspace_policy(
    name = "policy",
    families = [":rst_family"],
)
"#;

const MIXED_BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "vale_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "rst_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "rst",
    lint = ["vale"],
)

quality_family(
    name = "asciidoc_family",
    disabled = [
        "format",
        "typecheck",
    ],
    family_id = "asciidoc",
    lint = ["vale"],
)

quality_family(
    name = "typst_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "typst",
    format = ["typstyle"],
)

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
    families = [
        ":asciidoc_family",
        ":latex_family",
        ":rst_family",
        ":typst_family",
    ],
)

vale_config(
    name = "markers",
    src = "markers.ini",
    data = ["styles/Dx/Markers.yml"],
    visibility = ["//visibility:public"],
)
"#;

const MIXED_PKG: &str = r#"load("@rules_dx//asciidoc/rules:defs.bzl", "asciidoc_library")
load("@rules_dx//latex/rules:defs.bzl", "latex_library")
load("@rules_dx//rst/rules:defs.bzl", "rst_library")
load("@rules_dx//typst/rules:defs.bzl", "typst_library")

rst_library(
    name = "rst",
    srcs = ["guide.rst"],
    aspect_hints = ["//:markers"],
)

asciidoc_library(
    name = "adoc",
    srcs = ["guide.adoc"],
    aspect_hints = ["//:markers"],
)

typst_library(
    name = "typst",
    srcs = ["main.typ"],
)

latex_library(
    name = "latex",
    srcs = ["main.tex"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//asciidoc/rules:defs.bzl", "asciidoc_library")
load("@rules_dx//rst/rules:defs.bzl", "rst_library")

rst_library(
    name = "lib",
    srcs = ["base.rst"],
    aspect_hints = ["//:markers"],
    visibility = ["//visibility:public"],
)

asciidoc_library(
    name = "lib_adoc",
    srcs = ["base.adoc"],
    aspect_hints = ["//:markers"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//asciidoc/rules:defs.bzl", "asciidoc_library")
load("@rules_dx//rst/rules:defs.bzl", "rst_library")

rst_library(
    name = "app",
    srcs = [
        "guide.rst",
        "usage.rst",
    ],
    aspect_hints = ["//:CONFIG"],
    deps = ["//lib"],
)

asciidoc_library(
    name = "app_adoc",
    srcs = [
        "guide.adoc",
        "usage.adoc",
    ],
    aspect_hints = ["//:CONFIG"],
    deps = ["//lib:lib_adoc"],
)
"#;

fn set_config(root: &Case, config: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("CONFIG", config),
    );
}

fn base(name: &str, build: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case(name);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), build);
    write(&root.join("markers.ini"), MARKERS_INI);
    write(&root.join("drafts.ini"), DRAFTS_INI);
    write(&root.join("styles/Dx/Markers.yml"), MARKERS);
    write(&root.join("styles_draft/Dx/Markers.yml"), DRAFTS);
    root
}

fn consumer() -> Case {
    let root = base("consumer", BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.rst"), CLEAN_RST);
    write(&root.join("lib/base.adoc"), CLEAN_ADOC);
    set_config(&root, "markers");
    for name in ["guide", "usage"] {
        write(&root.join(format!("app/{name}.rst")), CLEAN_RST);
        write(&root.join(format!("app/{name}.adoc")), CLEAN_ADOC);
    }
    write(&root.join("app/notes.rst"), DIRTY_RST);
    write(&root.join("app/notes.adoc"), DIRTY_ADOC);
    root
}

fn mixed() -> Case {
    let root = base("mixed", MIXED_BUILD);
    write(&root.join("docs/BUILD.bazel"), MIXED_PKG);
    write(&root.join("docs/guide.rst"), CLEAN_RST);
    write(&root.join("docs/guide.adoc"), CLEAN_ADOC);
    write(&root.join("docs/main.typ"), TYPST);
    write(&root.join("docs/main.tex"), LATEX);
    root
}

fn dx_with(root: &Case, command: &str, flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("prose-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str) -> Run {
    dx_with(root, command, &[])
}

/// Every file under the workspace, as workspace-relative paths, without the Bazel output links.
fn tree(root: &Case) -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<(String, String)>) {
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
                let bytes = std::fs::read(&path).unwrap_or_default();
                out.push((
                    rel.to_string_lossy().into_owned(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("."), &root.join("."), &mut files);
    files.sort();
    files
}

#[test]
fn a_clean_multi_file_tree_passes_and_check_never_edits() {
    let root = consumer();
    let before = tree(&root);
    let out = dx(&root, "lint");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(tree(&root), before, "lint must not change the tree");
}

#[test]
fn only_the_dirty_rst_and_adoc_of_several_files_are_reported() {
    let root = consumer();
    write(&root.join("app/guide.rst"), DIRTY_RST);
    write(&root.join("app/usage.adoc"), DIRTY_ADOC);
    let before = tree(&root);
    let out = dx(&root, "lint");
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    let text = out.combined();
    assert!(text.contains("app/guide.rst"), "{text}");
    assert!(text.contains("app/usage.adoc"), "{text}");
    assert!(text.contains("Remove 'TODO' markers."), "{text}");
    for clean in [
        "app/usage.rst",
        "app/guide.adoc",
        "lib/base.rst",
        "lib/base.adoc",
    ] {
        assert!(!text.contains(clean), "{clean} reported: {text}");
    }
    assert_eq!(tree(&root), before, "lint must not change the tree");
}

#[test]
fn a_bound_nondefault_vale_config_changes_the_findings() {
    let root = consumer();
    write(&root.join("app/guide.rst"), DRAFT_RST);
    write(&root.join("app/guide.adoc"), DRAFT_ADOC);
    let markers = dx(&root, "lint");
    assert_eq!(markers.status.code(), Some(0), "{}", markers.combined());

    set_config(&root, "drafts");
    let drafts = dx(&root, "lint");
    assert_eq!(drafts.status.code(), Some(1), "{}", drafts.combined());
    let text = drafts.combined();
    assert!(text.contains("app/guide.rst"), "{text}");
    assert!(text.contains("app/guide.adoc"), "{text}");
    assert!(text.contains("Remove 'DRAFT' drafts."), "{text}");
}

#[test]
fn stray_unbound_vale_settings_next_to_the_sources_are_ignored() {
    let root = consumer();
    write(&root.join("app/guide.rst"), DRAFT_RST);
    write(&root.join("app/guide.adoc"), DRAFT_ADOC);
    for name in ["app/.vale.ini", "app/_vale.ini", ".vale.ini", "_vale.ini"] {
        write(&root.join(name), DRAFTS_INI);
    }
    let out = dx(&root, "lint");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_file_no_library_owns_is_not_linted() {
    let root = consumer();
    let out = dx(&root, "lint");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(!out.combined().contains("notes."), "{}", out.combined());
}

#[test]
fn a_policy_selecting_an_unwired_tool_fails_with_the_policy_message() {
    let root = consumer();
    write(&root.join("BUILD.bazel"), UNWIRED_BUILD);
    let out = dx(&root, "lint");
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("selects lint tool 'modlint' with no wired executable"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_declared_tools_are_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint");
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("prose-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("app/guide.rst"), DIRTY_RST);
    let built = dx_with(&root, "lint", &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/guide.rst"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", VALE_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());
    for target in [
        "@rules_dx//quality/tools/python:rst2html",
        "@rules_dx//quality/tools/javascript/bin:asciidoctor",
    ] {
        let tool = root.bazel(&["build", target, denied_flag.as_str()]);
        assert!(tool.status.success(), "{target}: {}", tool.combined());
    }

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        VALE_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(miss.combined().contains("vale"), "{}", miss.combined());
}

#[test]
fn one_workspace_with_all_four_document_families_passes_lint_and_format() {
    let root = mixed();
    let lint = dx(&root, "lint");
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    let format = dx(&root, "format");
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());

    write(&root.join("docs/guide.rst"), DIRTY_RST);
    let dirty = dx(&root, "lint");
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("docs/guide.rst"),
        "{}",
        dirty.combined()
    );
}

const EXAMPLE_MODULE: &str = r#"
bazel_dep(name = "rules_shell", version = "0.8.0")
bazel_dep(name = "rules_rust", version = "0.75.0")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)
"#;

/// The version pins and patches the root module applies, which a dependent module never inherits.
fn root_overrides(module: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for line in module.lines() {
        if line.starts_with("single_version_override(") {
            open = true;
        }
        if open {
            out.push_str(line);
            out.push('\n');
            if line == ")" {
                open = false;
                out.push('\n');
            }
        }
    }
    out
}

const EXAMPLE_POLICY: &str = "--@rules_dx//config:workspace=//examples/prose-docs:policy";

/// The example BUILD text as a consumer sees it, without the repo-only fixture targets.
fn consumer_build(text: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    for line in text.lines() {
        let drop = depth > 0
            || line.starts_with("exports_files(")
            || line.starts_with("real_source_target(")
            || (line.starts_with("load(") && line.contains("//quality:fixtures.bzl"));
        if drop {
            depth += line.matches('(').count() as i32 - line.matches(')').count() as i32;
            continue;
        }
        out.push(line);
    }
    out.join("\n") + "\n"
}

fn copy_tree(from: &std::path::Path, to: &Case, rel: &str) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let target = format!("{rel}/{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            copy_tree(&path, to, &target);
        } else {
            write(&to.join(&target), &read(&path));
        }
    }
}

fn copy_example(from: &std::path::Path, to: &Case, rel: &str) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let target = format!("{rel}/{name}");
        if path.is_dir() {
            copy_example(&path, to, &target);
        } else if name == "BUILD.bazel" {
            write(&to.join(&target), &consumer_build(&read(&path)));
        } else {
            write(&to.join(&target), &read(&path));
        }
    }
}

fn example() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("prose-example");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    let overrides = root_overrides(&read(&rules.join("MODULE.bazel")));
    let module =
        MODULE.replace("RULES_DX", &rules.display().to_string()) + EXAMPLE_MODULE + &overrides;
    write(&root.join("MODULE.bazel"), &module);
    copy_tree(&rules.join("patches"), &root, "patches");
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), "");
    copy_example(
        &rules.join("examples/prose-docs"),
        &root,
        "examples/prose-docs",
    );
    root
}

fn dx_example(root: &Case, command: &str, target: &str, flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("prose-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(flags);
    argv.extend_from_slice(&[target, "--", EXAMPLE_POLICY]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn sources(root: &Case) -> Vec<(String, String)> {
    let mut files = tree(root);
    files.retain(|(path, _)| path != "MODULE.bazel.lock");
    files
}

#[test]
fn the_prose_docs_example_passes_and_its_drafts_are_reported() {
    let root = example();
    let before = sources(&root);
    let lint = dx_example(&root, "lint", "//examples/prose-docs/...", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    let format = dx_example(&root, "format", "//examples/prose-docs/...", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
    assert_eq!(sources(&root), before, "check must not change the tree");

    let drafts = dx_example(&root, "lint", "//examples/prose-docs/drafts:todo_rst", &[]);
    assert_eq!(drafts.status.code(), Some(1), "{}", drafts.combined());
    assert!(
        drafts.combined().contains("drafts/todo.rst"),
        "{}",
        drafts.combined()
    );

    let messy = "//examples/prose-docs/drafts:messy";
    let check = dx_example(&root, "format", messy, &[]);
    assert_eq!(check.status.code(), Some(1), "{}", check.combined());
    assert!(
        check.combined().contains("drafts/messy.typ"),
        "{}",
        check.combined()
    );
    let apply = dx_example(&root, "format", messy, &["--apply"]);
    assert_eq!(apply.status.code(), Some(0), "{}", apply.combined());
    let after = sources(&root);
    let changed: Vec<&str> = after
        .iter()
        .filter(|entry| !before.contains(entry))
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(changed, ["examples/prose-docs/drafts/messy.typ"]);
    let recheck = dx_example(&root, "format", messy, &[]);
    assert_eq!(recheck.status.code(), Some(0), "{}", recheck.combined());
}
