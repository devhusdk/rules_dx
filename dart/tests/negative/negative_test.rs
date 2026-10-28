use std::path::PathBuf;

use dx_nested::Case;

const PUB_MODULE: &str = r#"
bazel_dep(name = "rules_dart", version = "0.8.0")

pub = use_extension("@rules_dart//dart/pub:extensions.bzl", "pub")
pub.from_lock(
    name = "app_deps",
    lock = "//:pubspec.lock",
    on_version_conflict = "upgrade",
)
use_repo(pub, "app_deps")
"#;

const BUILD: &str = r#"load("@rules_dx//dart/rules:defs.bzl", "dart_binary", "dart_test")

dart_test(
    name = "clean_test",
    main = "test/clean_test.dart",
    deps = [
        "//base",
        "@app_deps//:test",
    ],
)

dart_test(
    name = "compile_error_test",
    main = "test/broken_test.dart",
    deps = [
        "//broken",
        "@app_deps//:test",
    ],
)

dart_test(
    name = "failing_test",
    main = "test/failing_test.dart",
    deps = [
        "//base",
        "@app_deps//:test",
    ],
)

dart_test(
    name = "transitive_import_test",
    main = "test/transitive_test.dart",
    deps = [
        "//middle",
        "@app_deps//:test",
    ],
)

dart_test(
    name = "missing_pub_test",
    main = "test/clean_test.dart",
    deps = [
        "//base",
        "@app_deps//:no_such_package",
    ],
)

dart_binary(
    name = "crash",
    main = "bin/crash.dart",
)
"#;

const LIBRARY: &str = r#"load("@rules_dx//dart/rules:defs.bzl", "dart_library")

dart_library(
    name = "NAME",
    srcs = ["lib/NAME.dart"],
    package_name = "NAME",
    visibility = ["//visibility:public"],
)
"#;

const MIDDLE_BUILD: &str = r#"load("@rules_dx//dart/rules:defs.bzl", "dart_library")

dart_library(
    name = "middle",
    srcs = ["lib/middle.dart"],
    package_name = "middle",
    visibility = ["//visibility:public"],
    deps = ["//base"],
)
"#;

const BASE: &str = "int answer() => 41;\n";

const MIDDLE: &str = "import 'package:base/base.dart';\n\nint middle() => answer();\n";

const BROKEN: &str = "int answer() => 41 +;\n";

const CLEAN_TEST: &str = "import 'package:base/base.dart';\nimport 'package:test/test.dart';\n\nvoid main() {\n  test('clean', () => expect(answer(), 41));\n}\n";

const BROKEN_TEST: &str = "import 'package:broken/broken.dart';\nimport 'package:test/test.dart';\n\nvoid main() {\n  test('broken', () => expect(answer(), 41));\n}\n";

const FAILING_TEST: &str = "import 'package:base/base.dart';\nimport 'package:test/test.dart';\n\nvoid main() {\n  test('the answer is right', () => expect(answer(), 42));\n}\n";

const TRANSITIVE_TEST: &str = "import 'package:base/base.dart';\nimport 'package:middle/middle.dart';\nimport 'package:test/test.dart';\n\nvoid main() {\n  test('transitive', () => expect(middle() + answer(), 82));\n}\n";

const CRASH: &str =
    "import 'dart:io';\n\nvoid main() {\n  stderr.writeln('the ledger is empty');\n  exit(3);\n}\n";

fn consumer() -> Case {
    let rules = dx_testing::rules_dx_source();
    let case = Case::consumer("negative", PUB_MODULE);
    case.write("BUILD.bazel", "exports_files([\"pubspec.lock\"])\n");
    case.write(
        "pubspec.lock",
        &dx_testing::read_text(&rules.join("third_party/dart/pubspec.lock")),
    );
    case.write("app/BUILD.bazel", BUILD);
    for name in ["base", "broken"] {
        case.write(
            format!("{name}/BUILD.bazel"),
            &LIBRARY.replace("NAME", name),
        );
    }
    case.write("base/lib/base.dart", BASE);
    case.write("broken/lib/broken.dart", BROKEN);
    case.write("middle/BUILD.bazel", MIDDLE_BUILD);
    case.write("middle/lib/middle.dart", MIDDLE);
    case.write("app/test/clean_test.dart", CLEAN_TEST);
    case.write("app/test/broken_test.dart", BROKEN_TEST);
    case.write("app/test/failing_test.dart", FAILING_TEST);
    case.write("app/test/transitive_test.dart", TRANSITIVE_TEST);
    case.write("app/bin/crash.dart", CRASH);
    case
}

fn workspace_files(case: &Case) -> Vec<(PathBuf, Vec<u8>)> {
    case.files()
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

fn test_fails_with(case: &Case, target: &str, expected: &[&str]) {
    let tested = bazel(case, "test", target, &["--test_output=all"]);
    assert_eq!(tested.code, Some(3), "{target}\n{}", tested.text);
    for line in expected {
        assert!(
            tested.text.contains(line),
            "{target}: {line}\n{}",
            tested.text
        );
    }
}

#[test]
fn a_clean_test_passes() {
    let case = consumer();
    let tested = bazel(&case, "test", "//app:clean_test", &[]);
    assert_eq!(tested.code, Some(0), "{}", tested.text);
}

#[test]
fn building_and_testing_leaves_the_workspace_unchanged() {
    let case = consumer();
    let before = workspace_files(&case);
    let built = bazel(&case, "build", "//base", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
    let tested = bazel(&case, "test", "//app:failing_test", &[]);
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    let after = workspace_files(&case);
    let names = |files: &[(PathBuf, Vec<u8>)]| -> Vec<PathBuf> {
        files.iter().map(|(path, _)| path.clone()).collect()
    };
    assert_eq!(names(&before), names(&after));
    assert!(before == after, "a workspace file changed");
}

#[test]
fn a_compile_error_fails_with_the_compiler_diagnostic() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        "//app:compile_error_test",
        &["--test_output=all"],
    );
    assert_ne!(tested.code, Some(0), "{}", tested.text);
    assert!(tested.text.contains("broken.dart"), "{}", tested.text);
    assert!(tested.text.contains("Error"), "{}", tested.text);
}

#[test]
fn a_failing_assertion_fails_the_test() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:failing_test",
        &["Expected: <42>", "Actual: <41>"],
    );
}

#[test]
fn an_import_of_a_package_that_is_not_a_direct_dependency_fails() {
    let case = consumer();
    let built = bazel(&case, "build", "//app:transitive_import_test", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    assert!(built.text.contains("package:base"), "{}", built.text);
}

#[test]
fn a_missing_pub_dependency_fails_analysis() {
    let case = consumer();
    let built = bazel(&case, "build", "//app:missing_pub_test", &[]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(built.text.contains("no_such_package"), "{}", built.text);
}

#[test]
fn a_crashing_binary_exits_non_zero() {
    let case = consumer();
    let ran = bazel(&case, "run", "//app:crash", &[]);
    assert_ne!(ran.code, Some(0), "{}", ran.text);
    assert!(ran.text.contains("the ledger is empty"), "{}", ran.text);
}

#[test]
fn a_platform_without_a_rules_dart_sdk_is_incompatible() {
    let case = consumer();
    for target in ["//base", "//app:crash", "//app:clean_test"] {
        let built = bazel(
            &case,
            "build",
            target,
            &["--platforms=@rules_dx//msix/platforms:windows_arm64"],
        );
        assert_eq!(built.code, Some(1), "{target}\n{}", built.text);
        assert!(
            built
                .text
                .contains("is incompatible and cannot be built, but was explicitly requested"),
            "{target}\n{}",
            built.text
        );
    }
}
