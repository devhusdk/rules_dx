use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const BUILD: &str = r#"load("@rules_dx//fortran/rules:defs.bzl", "fortran_library", "fortran_test")

fortran_library(
    name = "clean",
    srcs = ["clean.f90"],
)

fortran_library(
    name = "syntax_error",
    srcs = ["syntax_error.f90"],
)

fortran_library(
    name = "missing_dep",
    srcs = ["missing_dep.f90"],
)

fortran_library(
    name = "same_target",
    srcs = [
        "provider.f90",
        "consumer.f90",
    ],
)

fortran_library(
    name = "tested",
    srcs = ["tested.f90"],
)

fortran_test(
    name = "failing_test",
    size = "small",
    srcs = ["failing_test.f90"],
    deps = [":tested"],
)
"#;

const CLEAN: &str = "module clean\n  implicit none\ncontains\n  pure integer function add(a, b)\n    integer, intent(in) :: a, b\n    add = a + b\n  end function add\nend module clean\n";

const SYNTAX: &str = "module broken\n  integer :: x =\nend module broken\n";

const NEEDS_DEP: &str = "module needs_dep\n  use base_dep, only: value\nend module needs_dep\n";

const PROVIDER: &str =
    "module provider\n  implicit none\n  integer, parameter :: answer = 42\nend module provider\n";

const CONSUMER: &str = "module consumer\n  use provider, only: answer\nend module consumer\n";

const TESTED: &str = "module tested\n  implicit none\ncontains\n  integer function answer()\n    answer = 41\n  end function answer\nend module tested\n";

const FAILING_TEST: &str = "program failing_test\n  use tested, only: answer\n  implicit none\n  if (answer() /= 42) error stop \"the answer is wrong: 41\"\nend program failing_test\n";

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
    let source = resolve_runfiles("fortran/tests/negative/negative_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
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
        &dx_testing::preset_rc(&rules),
    );
    write(&root.join("BUILD.bazel"), "");
    write(&root.join("fortran/BUILD.bazel"), BUILD);
    write(&root.join("fortran/clean.f90"), CLEAN);
    write(&root.join("fortran/syntax_error.f90"), SYNTAX);
    write(&root.join("fortran/missing_dep.f90"), NEEDS_DEP);
    write(&root.join("fortran/provider.f90"), PROVIDER);
    write(&root.join("fortran/consumer.f90"), CONSUMER);
    write(&root.join("fortran/tested.f90"), TESTED);
    write(&root.join("fortran/failing_test.f90"), FAILING_TEST);
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
    assert_eq!(built.code, Some(1), "{}", built.text);
    for line in expected {
        assert!(built.text.contains(line), "{line}\n{}", built.text);
    }
}

#[test]
fn a_clean_library_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//fortran:clean", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    fails_with(
        &case,
        "//fortran:syntax_error",
        &["fortran/syntax_error.f90:2:16: error: expected end of statement"],
    );
}

#[test]
fn a_module_outside_deps_fails_the_build() {
    let case = consumer();
    fails_with(
        &case,
        "//fortran:missing_dep",
        &["fortran/missing_dep.f90:2:7: error: Cannot parse module file for module 'base_dep'"],
    );
}

#[test]
fn a_module_from_another_source_of_the_same_target_fails_the_build() {
    let case = consumer();
    fails_with(
        &case,
        "//fortran:same_target",
        &["fortran/consumer.f90:2:7: error: Cannot parse module file for module 'provider'"],
    );
}

#[test]
fn a_failing_test_fails_with_its_message() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        "//fortran:failing_test",
        &["--test_output=all"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(
        tested.text.contains("the answer is wrong: 41"),
        "{}",
        tested.text
    );
}
