use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const LIBRARY_BUILD: &str = r#"load("@rules_dx//gleam/rules:defs.bzl", "GLEAM_COMPATIBLE_WITH", "gleam_js_library", "gleam_library")

gleam_library(
    name = "{name}",
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
    visibility = ["//visibility:public"],
    deps = DEPS,
)

gleam_js_library(
    name = "{name}_js",
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
    visibility = ["//visibility:public"],
    deps = JS_DEPS,
)
"#;

const TEST_BUILD: &str = r#"load("@rules_dx//gleam/rules:defs.bzl", "GLEAM_COMPATIBLE_WITH", "gleam_binary", "gleam_js_binary", "gleam_js_library", "gleam_js_test", "gleam_library", "gleam_test")

gleam_library(
    name = "tested",
    srcs = ["src/tested.gleam"],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
)

gleam_js_library(
    name = "tested_js",
    srcs = ["src/tested.gleam"],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
)

[gleam_test(
    name = name,
    srcs = ["test/{}.gleam".format(name)],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
    deps = [":tested"],
) for name in ["failing_test", "empty_test"]]

[gleam_js_test(
    name = name.replace("_test", "_js_test"),
    srcs = ["test/{}.gleam".format(name)],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
    deps = [":tested_js"],
) for name in ["failing_test", "empty_test"]]

gleam_binary(
    name = "crash",
    srcs = ["src/crash.gleam"],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
)

gleam_js_binary(
    name = "crash_js",
    srcs = ["src/crash.gleam"],
    target_compatible_with = GLEAM_COMPATIBLE_WITH,
)
"#;

const PLATFORMS_BUILD: &str = r#"load("@rules_dx//gleam/rules:defs.bzl", "gleam_js_library", "gleam_library")

gleam_library(name = "portable")

gleam_js_library(name = "portable_js")
"#;

const BAD_HEX_MODULE: &str = r#"
gleam_hex_package = use_repo_rule("@rules_dx//gleam/rules:hex.bzl", "gleam_hex_package")

gleam_hex_package(
    name = "tampered_json",
    package = "gleam_json",
    sha256 = "0000000000000000000000000000000000000000000000000000000000000000",
    version = "3.1.0",
    deps = ["@rules_dx//gleam/lib:gleam_stdlib"],
)
"#;

const CLEAN: &str = "pub fn add(a: Int, b: Int) -> Int {\n  a + b\n}\n";

const SYNTAX: &str = "pub fn add(a: Int, b: Int) -> Int {\n  a +\n}\n";

const WARNING: &str = "pub fn add(a: Int, b: Int) -> Int {\n  let unused = a\n  b\n}\n";

const TYPE_ERROR: &str = "pub fn add(a: Int, b: Int) -> String {\n  a + b\n}\n";

const NEEDS_DEP: &str = "import base_dep\n\npub fn value() -> Int {\n  base_dep.value()\n}\n";

const BASE_DEP: &str = "pub fn value() -> Int {\n  42\n}\n";

const TESTED: &str = "pub fn answer() -> Int {\n  41\n}\n";

const FAILING_TEST: &str =
    "import tested\n\npub fn the_answer_is_right_test() {\n  assert tested.answer() == 42\n}\n";

const EMPTY_TEST: &str = "pub fn helper() -> Int {\n  1\n}\n";

const CRASH: &str = "pub fn main() -> Nil {\n  panic as \"the ledger is empty\"\n}\n";

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
    let source = resolve_runfiles("gleam/tests/negative/negative_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn library(root: &Path, name: &str, deps: &str, source: &str) {
    let dir = root.join(name);
    let js_deps = deps.replace("\"]", "_js\"]");
    write(
        &dir.join("BUILD.bazel"),
        &format!(
            "DEPS = {deps}\n\nJS_DEPS = {js_deps}\n\n{}",
            LIBRARY_BUILD.replace("{name}", name)
        ),
    );
    write(&dir.join(format!("src/{name}.gleam")), source);
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let case = nested::case("negative");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n{BAD_HEX_MODULE}",
            rules.display()
        ),
    );
    write(
        &root.join(".bazelrc"),
        &read(&rules.join("tools/bazelrc/preset.bazelrc")),
    );
    write(&root.join("BUILD.bazel"), "");
    library(&root, "clean", "[]", CLEAN);
    library(&root, "syntax_error", "[]", SYNTAX);
    library(&root, "unused_variable", "[]", WARNING);
    library(&root, "type_error", "[]", TYPE_ERROR);
    library(&root, "base_dep", "[]", BASE_DEP);
    for (name, deps) in [
        ("missing_dep", "[]"),
        ("declared_dep", "[\"//base_dep:base_dep\"]"),
    ] {
        library(&root, name, deps, NEEDS_DEP);
    }
    write(&root.join("tested/BUILD.bazel"), TEST_BUILD);
    write(&root.join("tested/src/tested.gleam"), TESTED);
    write(&root.join("tested/src/crash.gleam"), CRASH);
    write(&root.join("tested/test/failing_test.gleam"), FAILING_TEST);
    write(&root.join("tested/test/empty_test.gleam"), EMPTY_TEST);
    write(&root.join("portable/BUILD.bazel"), PLATFORMS_BUILD);
    write(
        &root.join("portable/src/portable.gleam"),
        "pub fn value() -> Int {\n  1\n}\n",
    );
    case
}

fn snapshot(dir: &Path, root: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("list {dir:?}: {error}"))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path()
        })
        .collect();
    entries.sort();
    for path in entries {
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let name = relative.to_string_lossy();
        if name.starts_with("bazel-") || name == "MODULE.bazel.lock" {
            continue;
        }
        if path.is_dir() {
            snapshot(&path, root, files);
        } else {
            files.push((relative, std::fs::read(&path).unwrap_or_default()));
        }
    }
}

fn workspace_files(case: &Case) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    snapshot(case.root(), case.root(), &mut files);
    files
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
    assert_eq!(built.code, Some(1), "{}", built.text);
    for line in expected {
        assert!(built.text.contains(line), "{line}\n{}", built.text);
    }
}

fn both(name: &str) -> [String; 2] {
    [format!("//{name}"), format!("//{name}:{name}_js")]
}

#[test]
fn a_clean_library_builds_for_both_targets() {
    let case = consumer();
    for target in both("clean") {
        let built = bazel(&case, "build", &target, &[]);
        assert_eq!(built.code, Some(0), "{target}\n{}", built.text);
    }
}

#[test]
fn building_and_testing_leaves_the_workspace_unchanged() {
    let case = consumer();
    let before = workspace_files(&case);
    for target in both("declared_dep") {
        let built = bazel(&case, "build", &target, &[]);
        assert_eq!(built.code, Some(0), "{target}\n{}", built.text);
    }
    for target in ["//tested:failing_test", "//tested:failing_js_test"] {
        let tested = bazel(&case, "test", target, &[]);
        assert_eq!(tested.code, Some(3), "{target}\n{}", tested.text);
    }
    let after = workspace_files(&case);
    let names = |files: &[(PathBuf, Vec<u8>)]| -> Vec<PathBuf> {
        files.iter().map(|(path, _)| path.clone()).collect()
    };
    assert_eq!(names(&before), names(&after));
    assert!(before == after, "a workspace file changed");
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    for target in both("syntax_error") {
        fails_with(
            &case,
            &target,
            &[
                "error: Syntax error",
                "src/syntax_error.gleam:2:5",
                "This operator has no value on its right side",
            ],
        );
    }
}

#[test]
fn a_type_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    for target in both("type_error") {
        fails_with(
            &case,
            &target,
            &[
                "error: Type mismatch",
                "src/type_error.gleam:2:3",
                "Expected type:",
                "String",
            ],
        );
    }
}

#[test]
fn a_warning_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    for target in both("unused_variable") {
        fails_with(
            &case,
            &target,
            &[
                "warning: Unused variable",
                "src/unused_variable.gleam:2:7",
                "error: Gleam warnings are errors for",
            ],
        );
    }
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    for target in both("missing_dep") {
        fails_with(
            &case,
            &target,
            &[
                "error: Unknown module",
                "src/missing_dep.gleam:1:1",
                "No module has been found with the name `base_dep`.",
            ],
        );
    }
}

#[test]
fn a_declared_dependency_builds() {
    let case = consumer();
    for target in both("declared_dep") {
        let built = bazel(&case, "build", &target, &[]);
        assert_eq!(built.code, Some(0), "{target}\n{}", built.text);
    }
}

#[test]
fn a_tampered_hex_package_fails_the_fetch() {
    let case = consumer();
    let built = bazel(&case, "build", "@tampered_json//...", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    assert!(
        built.text.contains("Checksum was 44fdaa8847be8fc48ca7a1c089706bd54badcc4c45b237a992eddf9f2cdb2836 but wanted 0000000000000000000000000000000000000000000000000000000000000000"),
        "{}",
        built.text
    );
}

#[test]
fn a_failing_test_fails_with_its_assertion() {
    let case = consumer();
    for target in ["//tested:failing_test", "//tested:failing_js_test"] {
        let tested = bazel(&case, "test", target, &["--test_output=all"]);
        assert_eq!(tested.code, Some(3), "{target}\n{}", tested.text);
        for line in ["the_answer_is_right_test", "Assertion failed", "41"] {
            assert!(
                tested.text.contains(line),
                "{target}: {line}\n{}",
                tested.text
            );
        }
    }
}

#[test]
fn a_suite_that_runs_no_tests_fails() {
    let case = consumer();
    for target in ["//tested:empty_test", "//tested:empty_js_test"] {
        let tested = bazel(&case, "test", target, &["--test_output=all"]);
        assert_eq!(tested.code, Some(3), "{target}\n{}", tested.text);
        assert!(
            tested.text.contains("No tests found!"),
            "{target}\n{}",
            tested.text
        );
    }
}

#[test]
fn a_panic_fails_the_run_with_its_message() {
    let case = consumer();
    for target in ["//tested:crash", "//tested:crash_js"] {
        let ran = bazel(&case, "run", target, &[]);
        assert_eq!(ran.code, Some(1), "{target}\n{}", ran.text);
        assert!(
            ran.text.contains("the ledger is empty"),
            "{target}\n{}",
            ran.text
        );
    }
}

#[test]
fn another_target_platform_does_not_resolve_the_toolchain() {
    let case = consumer();
    for target in both("portable") {
        let built = bazel(
            &case,
            "build",
            &target,
            &["--platforms=@rules_dx//appimage/platforms:linux_arm64"],
        );
        assert_eq!(built.code, Some(1), "{target}\n{}", built.text);
        assert!(
            built
                .text
                .contains("No matching toolchains found for types"),
            "{target}\n{}",
            built.text
        );
    }
}
