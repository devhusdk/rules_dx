use std::path::PathBuf;

use dx_nested::Case;

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

fn app(case: &Case, name: &str, file: &str, source: &str) {
    case.write(
        format!("{name}/BUILD.bazel"),
        &APP_BUILD.replace("{name}", name),
    );
    case.write(format!("{name}/lib/{file}.ex"), source);
}

fn consumer() -> Case {
    let case = Case::consumer("negative", BAD_HEX_MODULE);
    case.write("BUILD.bazel", "");
    app(&case, "clean", "clean", CLEAN);
    app(&case, "syntax_error", "syntax_error", SYNTAX);
    app(&case, "unused_variable", "unused_variable", WARNING);
    app(&case, "base_dep", "base_dep", BASE_DEP);
    for (name, deps) in [
        ("missing_dep", "[]"),
        ("declared_dep", "[\"//base_dep:erlang_app\"]"),
    ] {
        case.write(
            format!("{name}/BUILD.bazel"),
            &format!("DEPS = {deps}\n\n{DEP_BUILD}"),
        );
        case.write(format!("{name}/lib/needs_dep.ex"), NEEDS_DEP);
    }
    case.write("tested/BUILD.bazel", TEST_BUILD);
    case.write("tested/lib/tested.ex", TESTED);
    case.write("tested/test/test_helper.exs", "ExUnit.start()\n");
    case.write("tested/test/failing_test.exs", FAILING_TEST);
    case.write("tested/test/empty_test.exs", EMPTY_TEST);
    case.write("portable/BUILD.bazel", PLATFORMS_BUILD);
    case.write("portable/lib/portable.ex", "defmodule Portable do\nend\n");
    case
}

#[test]
fn a_clean_app_builds() {
    let case = consumer();
    let built = case.plain("build", "//clean:erlang_app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
}

#[test]
fn building_and_testing_leaves_the_workspace_unchanged() {
    let case = consumer();
    let before = case.files();
    let built = case.plain("build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    let tested = case.plain("test", "//tested:failing_test", &[]);
    assert_eq!(tested.status.code(), Some(3), "{}", tested.combined());
    let after = case.files();
    let names = |files: &[(PathBuf, Vec<u8>)]| -> Vec<PathBuf> {
        files.iter().map(|(path, _)| path.clone()).collect()
    };
    assert_eq!(names(&before), names(&after));
    assert!(before == after, "a workspace file changed");
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    case.build_fails_with(
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
    case.build_fails_with(
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
    case.build_fails_with(
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
    let built = case.plain("build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
}

#[test]
fn a_tampered_hex_package_fails_the_fetch() {
    let case = consumer();
    let built = case.plain("build", "@tampered_jason//...", &[]);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built.combined().contains("Checksum was c5eb0cab91f094599f94d55bc63409236a8ec69a21a67814529e8d5f6cc90b3b but wanted 0000000000000000000000000000000000000000000000000000000000000000"),
        "{}",
        built.combined()
    );
}

#[test]
fn a_failing_test_fails_with_its_assertion() {
    let case = consumer();
    let tested = case.plain("test", "//tested:failing_test", &["--test_output=all"]);
    assert_eq!(tested.status.code(), Some(3), "{}", tested.combined());
    assert!(
        tested
            .combined()
            .contains("test the answer is right (FailingTest)"),
        "{}",
        tested.combined()
    );
    assert!(
        tested.combined().contains("left:  41"),
        "{}",
        tested.combined()
    );
}

#[test]
fn a_suite_that_runs_no_tests_fails() {
    let case = consumer();
    let tested = case.plain("test", "//tested:empty_test", &["--test_output=all"]);
    assert_eq!(tested.status.code(), Some(3), "{}", tested.combined());
    assert!(
        tested
            .combined()
            .contains("the suite executed no tests, and excluded none"),
        "{}",
        tested.combined()
    );
}

#[test]
fn another_target_platform_does_not_resolve_the_toolchain() {
    let case = consumer();
    let built = case.plain(
        "build",
        "//portable:erlang_app",
        &["--platforms=@rules_dx//appimage/platforms:linux_arm64"],
    );
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built
            .combined()
            .contains("No matching toolchains found for types"),
        "{}",
        built.combined()
    );
}
