use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const TAMPERED_MODULE: &str = r#"
php_phar = use_repo_rule("@rules_dx//php/toolchain:repos.bzl", "php_phar")

php_phar(
    name = "tampered_phar",
    file = "composer.phar",
    sha256 = "0000000000000000000000000000000000000000000000000000000000000000",
    urls = ["https://getcomposer.org/download/2.10.3/composer.phar"],
)
"#;

const BUILD: &str = r#"load("@rules_dx//php/rules:defs.bzl", "php_binary", "php_library", "php_test")

filegroup(
    name = "not_php",
    srcs = ["data.txt"],
)

php_library(
    name = "clean",
    srcs = ["src/Clean/Answer.php"],
)

php_library(
    name = "broken",
    srcs = ["src/Broken/Answer.php"],
)

php_library(
    name = "wrong_dep",
    srcs = ["src/Clean/Answer.php"],
    deps = [":not_php"],
)

php_test(
    name = "clean_test",
    srcs = ["test/CleanTest.php"],
    deps = [":clean"],
)

php_test(
    name = "failing_test",
    srcs = ["test/FailingTest.php"],
    deps = [":clean"],
)

php_test(
    name = "empty_test",
    srcs = ["test/EmptyTest.php"],
)

php_test(
    name = "undeclared_dep_test",
    srcs = ["test/CleanTest.php"],
)

php_binary(
    name = "crash",
    main = "bin/crash.php",
)
"#;

const CLEAN: &str = "<?php\n\ndeclare(strict_types=1);\n\nnamespace Clean;\n\nfinal class Answer\n{\n    public static function value(): int\n    {\n        return 41;\n    }\n}\n";

const BROKEN: &str = "<?php\n\nnamespace Broken;\n\nfinal class Answer\n{\n    public static function value(): int\n    {\n        return 41 +;\n    }\n}\n";

const CLEAN_TEST: &str = "<?php\n\ndeclare(strict_types=1);\n\nuse Clean\\Answer;\nuse PHPUnit\\Framework\\TestCase;\n\nfinal class CleanTest extends TestCase\n{\n    public function testValue(): void\n    {\n        $this->assertSame(41, Answer::value());\n    }\n}\n";

const FAILING_TEST: &str = "<?php\n\ndeclare(strict_types=1);\n\nuse Clean\\Answer;\nuse PHPUnit\\Framework\\TestCase;\n\nfinal class FailingTest extends TestCase\n{\n    public function testTheAnswerIsRight(): void\n    {\n        $this->assertSame(42, Answer::value());\n    }\n}\n";

const EMPTY_TEST: &str = "<?php\n\ndeclare(strict_types=1);\n\nuse PHPUnit\\Framework\\TestCase;\n\nfinal class EmptyTest extends TestCase\n{\n}\n";

const CRASH: &str =
    "<?php\n\ndeclare(strict_types=1);\n\nthrow new RuntimeException('the ledger is empty');\n";

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
    let source = resolve_runfiles("php/tests/negative/negative_test.rs");
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
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n{TAMPERED_MODULE}",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&rules));
    write(&root.join("BUILD.bazel"), "");
    let pkg = root.join("app");
    write(&pkg.join("BUILD.bazel"), BUILD);
    write(&pkg.join("data.txt"), "data\n");
    write(&pkg.join("src/Clean/Answer.php"), CLEAN);
    write(&pkg.join("src/Broken/Answer.php"), BROKEN);
    write(&pkg.join("test/CleanTest.php"), CLEAN_TEST);
    write(&pkg.join("test/FailingTest.php"), FAILING_TEST);
    write(&pkg.join("test/EmptyTest.php"), EMPTY_TEST);
    write(&pkg.join("bin/crash.php"), CRASH);
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
fn a_syntax_error_fails_the_build_with_the_parser_diagnostic() {
    let case = consumer();
    let built = bazel(&case, "build", "//app:broken", &[]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    for line in [
        "PhpLint",
        "syntax error, unexpected token \";\"",
        "Answer.php on line 9",
    ] {
        assert!(built.text.contains(line), "{line}\n{}", built.text);
    }
}

#[test]
fn a_fatal_error_fails_the_binary() {
    let case = consumer();
    let ran = bazel(&case, "run", "//app:crash", &[]);
    assert_ne!(ran.code, Some(0), "{}", ran.text);
    assert!(
        ran.text
            .contains("Uncaught RuntimeException: the ledger is empty"),
        "{}",
        ran.text
    );
}

#[test]
fn a_failing_assertion_fails_the_test() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:failing_test",
        &[
            "FailingTest::testTheAnswerIsRight",
            "Failed asserting that 41 is identical to 42.",
        ],
    );
}

#[test]
fn an_empty_test_suite_fails() {
    let case = consumer();
    test_fails_with(&case, "//app:empty_test", &["No tests executed!"]);
}

#[test]
fn a_class_missing_from_deps_is_not_autoloaded() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:undeclared_dep_test",
        &["Class \"Clean\\Answer\" not found"],
    );
}

#[test]
fn a_dependency_without_php_info_fails_analysis() {
    let case = consumer();
    let built = bazel(&case, "build", "//app:wrong_dep", &[]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(
        built
            .text
            .contains("does not have mandatory providers: 'PhpInfo'"),
        "{}",
        built.text
    );
}

#[test]
fn a_tampered_phar_pin_fails_the_fetch() {
    let case = consumer();
    let built = bazel(&case, "build", "@tampered_phar//:composer.phar", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    assert!(
        built.text.contains("Checksum was 7a2d379d5b8ffdaa028580ef26494c36d2feef4b178d3dd1473a4dbc5e17c8d6 but wanted 0000000000000000000000000000000000000000000000000000000000000000"),
        "{}",
        built.text
    );
}

#[test]
fn a_platform_without_a_pinned_php_is_incompatible() {
    let case = consumer();
    for target in ["//app:clean", "//app:crash", "//app:clean_test"] {
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
