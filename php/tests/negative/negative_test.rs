use dx_nested::Case;

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

fn consumer() -> Case {
    let case = Case::consumer("negative", TAMPERED_MODULE);
    case.write("BUILD.bazel", "");
    case.write("app/BUILD.bazel", BUILD);
    case.write("app/data.txt", "data\n");
    case.write("app/src/Clean/Answer.php", CLEAN);
    case.write("app/src/Broken/Answer.php", BROKEN);
    case.write("app/test/CleanTest.php", CLEAN_TEST);
    case.write("app/test/FailingTest.php", FAILING_TEST);
    case.write("app/test/EmptyTest.php", EMPTY_TEST);
    case.write("app/bin/crash.php", CRASH);
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

fn lock_entry(name: &str, kind: &str, url: &str, requires: &str) -> String {
    format!(
        r#"{{"name": "{name}", "version": "1.0.0", "type": "{kind}", "dist": {{"type": "tar", "url": "{url}", "reference": "r1"}}, "require": {{{requires}}}, "autoload": {{"psr-4": {{"V\\": "src/"}}}}}}"#
    )
}

/// A consumer whose composer.lock names one `file://` tar written by the test, so no network is needed.
fn composer_case(kind: &str, requires: &str, digests: &dyn Fn(&str) -> String) -> Case {
    let case = consumer();
    let root = case.root().to_path_buf();
    let src = root.join("tarball/pkg/src");
    dx_testing::write_text(
        &src.join("A.php"),
        "<?php\n\nnamespace V;\n\nfinal class A\n{\n}\n",
    );
    let tar = root.join("tarball/pkg.tar");
    let made = std::process::Command::new("tar")
        .arg("-cf")
        .arg(&tar)
        .arg("-C")
        .arg(root.join("tarball"))
        .arg("pkg")
        .status()
        .unwrap_or_else(|error| panic!("tar: {error}"));
    assert!(made.success());
    let url = format!("file://{}", tar.display());
    let lock = format!(
        r#"{{"packages": [{}], "packages-dev": []}}"#,
        lock_entry("v/a", kind, &url, requires)
    );
    dx_testing::write_text(&root.join("vendor/composer.lock"), &lock);
    dx_testing::write_text(&root.join("vendor/composer.digests.json"), &digests(&url));
    dx_testing::write_text(
        &root.join("vendor/BUILD.bazel"),
        "exports_files([\"composer.lock\", \"composer.digests.json\"])\n",
    );
    let module = dx_testing::read_text(&root.join("MODULE.bazel"));
    dx_testing::write_text(
        &root.join("MODULE.bazel"),
        &format!(
            "{module}\ncomposer = use_extension(\"@rules_dx//php/composer:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor_hub\", lock = \"//vendor:composer.lock\", digests = \"//vendor:composer.digests.json\")\nuse_repo(composer, \"vendor_hub\")\n"
        ),
    );
    case
}

fn pins(sha256: &str, reference: &str) -> impl Fn(&str) -> String {
    let sha256 = sha256.to_string();
    let reference = reference.to_string();
    move |_| {
        format!(
            r#"{{"version": 1, "packages": {{"v/a": {{"reference": "{reference}", "sha256": "{sha256}"}}}}}}"#
        )
    }
}

fn composer_build_fails_with(case: &Case, expected: &[&str]) {
    let built = bazel(case, "build", "@vendor_hub//v/a", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    for line in expected {
        assert!(built.text.contains(line), "{line}\n{}", built.text);
    }
}

#[test]
fn a_tampered_package_digest_fails_the_fetch() {
    let zeros = "0".repeat(64);
    let case = composer_case("library", "", &pins(&zeros, "r1"));
    composer_build_fails_with(&case, &["Checksum was", &format!("but wanted {zeros}")]);
}

#[test]
fn a_missing_package_digest_fails_and_prints_the_entry_to_commit() {
    let case = composer_case("library", "", &|_| {
        r#"{"version": 1, "packages": {}}"#.to_string()
    });
    composer_build_fails_with(
        &case,
        &[
            "has no entry in composer.digests.json",
            "Review it and commit to composer.digests.json",
            "\"v/a\": {",
        ],
    );
}

#[test]
fn a_stale_package_reference_says_re_pin() {
    let case = composer_case("library", "", &pins(&"a".repeat(64), "old"));
    composer_build_fails_with(&case, &["re-pin"]);
}

#[test]
fn a_composer_plugin_is_rejected() {
    let case = composer_case("composer-plugin", "", &pins(&"a".repeat(64), "r1"));
    composer_build_fails_with(&case, &["v/a is a composer-plugin"]);
}

#[test]
fn a_requirement_absent_from_the_lock_fails() {
    let case = composer_case(
        "library",
        r#""x/missing": "*""#,
        &pins(&"a".repeat(64), "r1"),
    );
    composer_build_fails_with(
        &case,
        &["v/a requires x/missing, which is not in composer.lock"],
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
