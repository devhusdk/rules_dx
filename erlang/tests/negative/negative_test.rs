use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};

const APP_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "erlang_app")

erlang_app(
    app_name = "{name}",
    app_version = "1.0.0",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    visibility = ["//visibility:public"],
)
"#;

const DEP_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "erlang_app")

erlang_app(
    app_name = "needs_dep",
    app_version = "1.0.0",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    deps = DEPS,
)
"#;

const SUITE_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "ct_suite", "erlang_app", "test_erlang_app")

erlang_app(
    app_name = "failing",
    app_version = "1.0.0",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
)

test_erlang_app(
    app_name = "failing",
    app_version = "1.0.0",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
)

ct_suite(
    name = "failing_SUITE",
    app_name = "failing",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
)
"#;

const PLATFORMS_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_app")

erlang_app(
    app_name = "portable",
    app_version = "1.0.0",
)
"#;

const CLEAN: &str = "-module(clean).\n\n-export([add/2]).\n\nadd(A, B) -> A + B.\n";

const SYNTAX: &str = "-module(syntax_error).\n\n-export([add/2]).\n\nadd(A, B) -> A + .\n";

const WARNING: &str =
    "-module(unused_variable).\n\n-export([add/2]).\n\nadd(A, B) -> Unused = A, B.\n";

const NEEDS_DEP: &str = "-module(needs_dep).\n\n-include_lib(\"base_dep/include/base_dep.hrl\").\n\n-export([value/0]).\n\nvalue() -> ?BASE_VALUE.\n";

const BASE_HEADER: &str = "-define(BASE_VALUE, 42).\n";

const FAILING_SUITE: &str = "-module(failing_SUITE).\n\n-export([all/0, always_fails/1]).\n\nall() -> [always_fails].\n\nalways_fails(_Config) -> 1 = 2.\n";

const FAILING_SRC: &str = "-module(failing).\n\n-export([ok/0]).\n\nok() -> ok.\n";

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
    dx_testing::rules_dx_source()
}

fn app(root: &Path, name: &str, module: &str, source: &str) {
    let dir = root.join(name);
    write(&dir.join("BUILD.bazel"), &APP_BUILD.replace("{name}", name));
    write(&dir.join(format!("src/{module}.erl")), source);
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
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
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
    app(&root, "base_dep", "base_dep", "-module(base_dep).\n");
    write(&root.join("base_dep/include/base_dep.hrl"), BASE_HEADER);
    for (name, deps) in [
        ("missing_dep", "[]"),
        ("declared_dep", "[\"//base_dep:erlang_app\"]"),
    ] {
        let dir = root.join(name);
        write(
            &dir.join("BUILD.bazel"),
            &format!("DEPS = {deps}\n\n{DEP_BUILD}"),
        );
        write(&dir.join("src/needs_dep.erl"), NEEDS_DEP);
    }
    write(&root.join("failing/BUILD.bazel"), SUITE_BUILD);
    write(&root.join("failing/src/failing.erl"), FAILING_SRC);
    write(&root.join("failing/test/failing_SUITE.erl"), FAILING_SUITE);
    write(&root.join("portable/BUILD.bazel"), PLATFORMS_BUILD);
    write(
        &root.join("portable/src/portable.erl"),
        "-module(portable).\n",
    );
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

fn fails_with(case: &Case, target: &str, expected: &str) {
    let built = bazel(case, "build", target, &[]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(built.text.contains(expected), "{}", built.text);
}

#[test]
fn a_clean_app_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//clean:erlang_app", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    fails_with(
        &case,
        "//syntax_error:erlang_app",
        "syntax_error.erl:5:18: syntax error before: '.'",
    );
}

#[test]
fn a_warning_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    fails_with(
        &case,
        "//unused_variable:erlang_app",
        "unused_variable.erl:5:14: variable 'Unused' is unused",
    );
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    fails_with(
        &case,
        "//missing_dep:erlang_app",
        "can't find include lib \"base_dep/include/base_dep.hrl\"",
    );
}

#[test]
fn a_declared_dependency_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_failing_suite_fails_the_test() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        "//failing:failing_SUITE",
        &["--test_output=all"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(tested.text.contains("always_fails"), "{}", tested.text);
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
