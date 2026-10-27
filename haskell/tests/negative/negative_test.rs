use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const LIBRARY_BUILD: &str = r#"load("@rules_dx//haskell/rules:defs.bzl", "HASKELL_COMPATIBLE_WITH", "haskell_library")

haskell_library(
    name = "{name}",
    srcs = ["{module}.hs"],
    target_compatible_with = HASKELL_COMPATIBLE_WITH,
    visibility = ["//visibility:public"],
    deps = DEPS + ["@dx_haskell_toolchains//:base"],
    GHCOPTS
)
"#;

const BASE_DEP: &str = "module BaseDep (value) where\n\nvalue :: Int\nvalue = 42\n";

const NEEDS_DEP: &str =
    "module NeedsDep (answer) where\n\nimport BaseDep (value)\n\nanswer :: Int\nanswer = value\n";

const CLEAN: &str = "module Clean (add) where\n\nadd :: Int -> Int -> Int\nadd a b = a + b\n";

const MALFORMED: &str = "module Malformed (add) where\n\nadd :: Int -> Int -> Int\nadd a b = a +\n";

const WARNING: &str =
    "module Unused (add) where\n\nadd :: Int -> Int -> Int\nadd a b = let unused = a in b\n";

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
    let source = resolve_runfiles("haskell/tests/negative/negative_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

/// Mirrors `source` as symlinks, replacing the file at `relative` with `text`.
fn farm(source: &Path, target: &Path, relative: &Path, text: &str) {
    std::fs::create_dir_all(target).unwrap_or_else(|error| panic!("mkdir {target:?}: {error}"));
    let mut first = relative.components();
    let head = first
        .next()
        .map(|component| component.as_os_str().to_owned());
    let rest: PathBuf = first.collect();
    for entry in
        std::fs::read_dir(source).unwrap_or_else(|error| panic!("list {source:?}: {error}"))
    {
        let entry = entry.unwrap_or_else(|error| panic!("entry: {error}"));
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("bazel-") {
            continue;
        }
        if Some(&name) == head.as_ref() {
            if rest.as_os_str().is_empty() {
                write(&target.join(&name), text);
            } else {
                farm(&entry.path(), &target.join(&name), &rest, text);
            }
        } else {
            std::os::unix::fs::symlink(entry.path(), target.join(&name))
                .unwrap_or_else(|error| panic!("symlink {name:?}: {error}"));
        }
    }
}

fn library(root: &Path, name: &str, module: &str, deps: &str, ghcopts: &str, source: &str) {
    let dir = root.join(name);
    let build = LIBRARY_BUILD
        .replace("{name}", name)
        .replace("{module}", module)
        .replace("    GHCOPTS\n", ghcopts);
    write(
        &dir.join("BUILD.bazel"),
        &format!("DEPS = {deps}\n\n{build}"),
    );
    write(&dir.join(format!("{module}.hs")), source);
}

fn consumer_of(rules: &Path) -> Case {
    let case = nested::case("negative");
    let root = case.root().to_path_buf();
    let source = rules_dx_root();
    write(
        &root.join(".bazelversion"),
        &read(&source.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&source));
    write(&root.join("BUILD.bazel"), "");
    library(&root, "clean", "Clean", "[]", "", CLEAN);
    library(&root, "malformed_source", "Malformed", "[]", "", MALFORMED);
    library(&root, "unused_variable", "Unused", "[]", "", WARNING);
    library(
        &root,
        "failing_tool",
        "Clean",
        "[]",
        "    ghcopts = [\"-fdx-no-such-flag\"],\n",
        CLEAN,
    );
    library(&root, "base_dep", "BaseDep", "[]", "", BASE_DEP);
    library(&root, "missing_dep", "NeedsDep", "[]", "", NEEDS_DEP);
    library(
        &root,
        "declared_dep",
        "NeedsDep",
        "[\"//base_dep\"]",
        "",
        NEEDS_DEP,
    );
    case
}

fn consumer() -> Case {
    consumer_of(&rules_dx_root())
}

/// A consumer of rules_dx whose Haskell pins have one edit applied.
fn tampered(old: &str, new: &str) -> Case {
    let source = rules_dx_root();
    let pins = Path::new("haskell/toolchain/pins.bzl");
    let text = read(&source.join(pins));
    assert_eq!(text.matches(old).count(), 1, "{old}");
    let case = consumer_of(Path::new("rules_dx"));
    let rules = case.root().join("rules_dx");
    farm(&source, &rules, pins, &text.replace(old, new));
    write(
        &case.root().join("MODULE.bazel"),
        &read(&case.root().join("MODULE.bazel")).replace(
            "path = \"rules_dx\"",
            &format!("path = \"{}\"", rules.display()),
        ),
    );
    write(&case.root().join(".bazelignore"), "rules_dx\n");
    case
}

struct Outcome {
    code: Option<i32>,
    text: String,
}

fn bazel(case: &Case, verb: &str, target: &str, flags: &[&str]) -> Outcome {
    let mut args = vec![verb, "--color=no", "--curses=no"];
    args.extend_from_slice(flags);
    args.push(target);
    let run = case.bazel(&args);
    Outcome {
        code: run.status.code(),
        text: run.combined(),
    }
}

fn fails_with(case: &Case, target: &str, expected: &[&str]) {
    let built = bazel(case, "build", target, &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    for line in expected {
        assert!(built.text.contains(line), "{line}\n{}", built.text);
    }
}

#[test]
fn a_clean_library_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//clean", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn malformed_source_fails_with_its_diagnostic() {
    fails_with(
        &consumer(),
        "//malformed_source",
        &["Malformed.hs:5:1: error:", "parse error"],
    );
}

#[test]
fn a_warning_fails_the_build() {
    fails_with(
        &consumer(),
        "//unused_variable",
        &["Unused.hs:4:15: error:", "Werror=unused-local-binds"],
    );
}

#[test]
fn missing_dep_fails_with_its_diagnostic() {
    fails_with(
        &consumer(),
        "//missing_dep",
        &[
            "NeedsDep.hs:3:1: error:",
            "Could not find module",
            "BaseDep",
        ],
    );
}

#[test]
fn a_declared_dependency_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//declared_dep", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn failing_tool_fails_with_its_diagnostic() {
    fails_with(
        &consumer(),
        "//failing_tool",
        &["unrecognised flag: -fdx-no-such-flag"],
    );
}

#[test]
fn tampered_ghc_sha_fails_the_fetch() {
    let pinned = "90f4e3082e1f43b984579f16b6349bd406dfdf88bda85733d7ccd54cb6a8eda8";
    let zeros = "0".repeat(64);
    let case = tampered(pinned, &zeros);
    fails_with(
        &case,
        "//clean",
        &[&format!("Checksum was {pinned} but wanted {zeros}")],
    );
}

#[test]
fn missing_closure_lib_fails_the_fetch() {
    let case = tampered(
        "\"usr/lib/x86_64-linux-gnu/libgmp.so.10\"",
        "\"usr/lib/x86_64-linux-gnu/libgmp.so.99\"",
    );
    fails_with(
        &case,
        "//clean",
        &["does not ship usr/lib/x86_64-linux-gnu/libgmp.so.99"],
    );
}
