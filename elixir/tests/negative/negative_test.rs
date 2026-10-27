use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const APP_BUILD: &str = r#"load("@rules_dx//elixir/rules:defs.bzl", "ELIXIR_COMPATIBLE_WITH", "elixir_app")

elixir_app(
    app_name = "{name}",
    app_version = "1.0.0",
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
    visibility = ["//visibility:public"],
)
"#;

const DEP_BUILD: &str = r#"load("@rules_dx//elixir/rules:defs.bzl", "ELIXIR_COMPATIBLE_WITH", "elixir_app")

elixir_app(
    app_name = "needs_dep",
    app_version = "1.0.0",
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
    deps = DEPS,
)
"#;

const TEST_BUILD: &str = r#"load("@rules_dx//elixir/rules:defs.bzl", "ELIXIR_COMPATIBLE_WITH", "elixir_app", "ex_unit_test")

elixir_app(
    app_name = "tested",
    app_version = "1.0.0",
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
)

ex_unit_test(
    name = "failing_test",
    srcs = [
        "test/failing_test.exs",
        "test/test_helper.exs",
    ],
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
    deps = [":erlang_app"],
)

ex_unit_test(
    name = "empty_test",
    srcs = [
        "test/empty_test.exs",
        "test/test_helper.exs",
    ],
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
    deps = [":erlang_app"],
)
"#;

const PLATFORMS_BUILD: &str = r#"load("@rules_dx//elixir/rules:defs.bzl", "elixir_app")

elixir_app(
    app_name = "portable",
    app_version = "1.0.0",
)
"#;

const BAD_HEX_MODULE: &str = r#"
hex_package = use_repo_rule("@rules_dx//elixir/rules:hex.bzl", "hex_package")

hex_package(
    name = "tampered_jason",
    build_file_content = "exports_files([\"mix.exs\"])",
    package = "jason",
    sha256 = "0000000000000000000000000000000000000000000000000000000000000000",
    version = "1.4.4",
)
"#;

const CLEAN: &str = "defmodule Clean do\n  def add(a, b), do: a + b\nend\n";

const SYNTAX: &str = "defmodule SyntaxError do\n  def add(a, b), do: a +\nend\n";

const WARNING: &str =
    "defmodule UnusedVariable do\n  def add(a, b) do\n    unused = a\n    b\n  end\nend\n";

const NEEDS_DEP: &str = "defmodule NeedsDep do\n  def value, do: BaseDep.value()\nend\n";

const BASE_DEP: &str = "defmodule BaseDep do\n  def value, do: 42\nend\n";

const TESTED: &str = "defmodule Tested do\n  def answer, do: 41\nend\n";

const FAILING_TEST: &str = "defmodule FailingTest do\n  use ExUnit.Case\n\n  test \"the answer is right\" do\n    assert Tested.answer() == 42\n  end\nend\n";

const EMPTY_TEST: &str = "defmodule EmptyTest do\n  use ExUnit.Case\nend\n";

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
    let source = resolve_runfiles("elixir/tests/negative/negative_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn app(root: &Path, name: &str, file: &str, source: &str) {
    let dir = root.join(name);
    write(&dir.join("BUILD.bazel"), &APP_BUILD.replace("{name}", name));
    write(&dir.join(format!("lib/{file}.ex")), source);
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
    app(&root, "clean", "clean", CLEAN);
    app(&root, "syntax_error", "syntax_error", SYNTAX);
    app(&root, "unused_variable", "unused_variable", WARNING);
    app(&root, "base_dep", "base_dep", BASE_DEP);
    for (name, deps) in [
        ("missing_dep", "[]"),
        ("declared_dep", "[\"//base_dep:erlang_app\"]"),
    ] {
        let dir = root.join(name);
        write(
            &dir.join("BUILD.bazel"),
            &format!("DEPS = {deps}\n\n{DEP_BUILD}"),
        );
        write(&dir.join("lib/needs_dep.ex"), NEEDS_DEP);
    }
    write(&root.join("tested/BUILD.bazel"), TEST_BUILD);
    write(&root.join("tested/lib/tested.ex"), TESTED);
    write(
        &root.join("tested/test/test_helper.exs"),
        "ExUnit.start()\n",
    );
    write(&root.join("tested/test/failing_test.exs"), FAILING_TEST);
    write(&root.join("tested/test/empty_test.exs"), EMPTY_TEST);
    write(&root.join("portable/BUILD.bazel"), PLATFORMS_BUILD);
    write(
        &root.join("portable/lib/portable.ex"),
        "defmodule Portable do\nend\n",
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

#[test]
fn a_clean_app_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//clean:erlang_app", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn building_and_testing_leaves_the_workspace_unchanged() {
    let case = consumer();
    let before = workspace_files(&case);
    let built = bazel(&case, "build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
    let tested = bazel(&case, "test", "//tested:failing_test", &[]);
    assert_eq!(tested.code, Some(3), "{}", tested.text);
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
    fails_with(
        &case,
        "//syntax_error:erlang_app",
        &[
            "syntax_error/lib/syntax_error.ex:3:1",
            "syntax error before: end",
        ],
    );
}

#[test]
fn a_warning_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    fails_with(
        &case,
        "//unused_variable:erlang_app",
        &[
            "variable \"unused\" is unused",
            "unused_variable/lib/unused_variable.ex:3:5",
            "Compilation failed due to warnings while using the --warnings-as-errors option",
        ],
    );
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    fails_with(
        &case,
        "//missing_dep:erlang_app",
        &[
            "BaseDep.value/0 is undefined (module BaseDep is not available",
            "missing_dep/lib/needs_dep.ex:2:26",
            "Compilation failed due to warnings while using the --warnings-as-errors option",
        ],
    );
}

#[test]
fn a_declared_dependency_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_tampered_hex_package_fails_the_fetch() {
    let case = consumer();
    let built = bazel(&case, "build", "@tampered_jason//...", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    assert!(
        built.text.contains("Checksum was c5eb0cab91f094599f94d55bc63409236a8ec69a21a67814529e8d5f6cc90b3b but wanted 0000000000000000000000000000000000000000000000000000000000000000"),
        "{}",
        built.text
    );
}

#[test]
fn a_failing_test_fails_with_its_assertion() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        "//tested:failing_test",
        &["--test_output=all"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(
        tested
            .text
            .contains("test the answer is right (FailingTest)"),
        "{}",
        tested.text
    );
    assert!(tested.text.contains("left:  41"), "{}", tested.text);
}

#[test]
fn a_suite_that_runs_no_tests_fails() {
    let case = consumer();
    let tested = bazel(&case, "test", "//tested:empty_test", &["--test_output=all"]);
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(
        tested
            .text
            .contains("the suite executed no tests, and excluded none"),
        "{}",
        tested.text
    );
}

#[test]
fn another_target_platform_does_not_resolve_the_toolchain() {
    let case = consumer();
    let built = bazel(
        &case,
        "build",
        "//portable:erlang_app",
        &["--platforms=@rules_dx//appimage/platforms:linux_arm64"],
    );
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(
        built
            .text
            .contains("No matching toolchains found for types"),
        "{}",
        built.text
    );
}
