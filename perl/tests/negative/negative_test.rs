use std::path::PathBuf;

use dx_nested::Case;

const CPAN_MODULE: &str = r#"
bazel_dep(name = "rules_perl", version = "1.5.0")

cpan = use_extension("@rules_perl//perl/cpan:extensions.bzl", "cpan")
cpan.install(
    name = "tampered_cpan",
    lock = "//:tampered.lock.json",
)
use_repo(cpan, "tampered_cpan")
"#;

const TAMPERED_LOCK: &str = r#"{
   "Text-CSV" : {
      "dependencies" : [],
      "sha256" : "0000000000000000000000000000000000000000000000000000000000000000",
      "strip_prefix" : "Text-CSV-2.06",
      "url" : "https://cpan.metacpan.org/authors/id/I/IS/ISHIGAKI/Text-CSV-2.06.tar.gz"
   }
}
"#;

const ROOT_BUILD: &str = "exports_files([\"tampered.lock.json\"])\n";

const BUILD: &str = r#"load("@rules_dx//perl/rules:defs.bzl", "perl_binary", "perl_library", "perl_test")

filegroup(
    name = "not_perl",
    srcs = ["data.txt"],
)

perl_library(
    name = "clean",
    srcs = ["lib/Clean.pm"],
)

perl_library(
    name = "broken",
    srcs = ["lib/Broken.pm"],
)

perl_library(
    name = "base",
    srcs = ["lib/Base.pm"],
)

perl_library(
    name = "wrong_dep",
    srcs = ["lib/Clean.pm"],
    deps = [":not_perl"],
)

perl_test(
    name = "clean_test",
    srcs = ["t/clean.t"],
    deps = [":clean"],
)

perl_test(
    name = "syntax_error_test",
    srcs = ["t/broken.t"],
    deps = [":broken"],
)

perl_test(
    name = "failing_test",
    srcs = ["t/failing.t"],
    deps = [":clean"],
)

perl_test(
    name = "dies_test",
    srcs = ["t/dies.t"],
)

perl_test(
    name = "missing_module_test",
    srcs = ["t/missing.t"],
)

perl_test(
    name = "undeclared_dep_test",
    srcs = ["t/base.t"],
)

perl_test(
    name = "declared_dep_test",
    srcs = ["t/base.t"],
    deps = [":base"],
)

perl_binary(
    name = "crash",
    srcs = ["bin/crash.pl"],
)
"#;

const CLEAN: &str =
    "package Clean;\n\nuse strict;\nuse warnings;\n\nsub answer { return 41; }\n\n1;\n";

const BROKEN: &str =
    "package Broken;\n\nuse strict;\nuse warnings;\n\nsub answer { return 41 +; }\n\n1;\n";

const BASE: &str =
    "package Base;\n\nuse strict;\nuse warnings;\n\nsub value { return 42; }\n\n1;\n";

const CLEAN_TEST: &str =
    "use strict;\nuse warnings;\nuse Test::More tests => 1;\nuse Clean;\n\nis(Clean::answer(), 41, 'clean');\n";

const BROKEN_TEST: &str =
    "use strict;\nuse warnings;\nuse Test::More tests => 1;\nuse Broken;\n\nis(Broken::answer(), 41, 'broken');\n";

const FAILING_TEST: &str = "use strict;\nuse warnings;\nuse Test::More tests => 1;\nuse Clean;\n\nis(Clean::answer(), 42, 'the answer is right');\n";

const DIES_TEST: &str =
    "use strict;\nuse warnings;\nuse Test::More tests => 1;\n\ndie \"the ledger is empty\\n\";\n";

const MISSING_TEST: &str =
    "use strict;\nuse warnings;\nuse Test::More tests => 1;\nuse Missing::Module;\n\nok(1);\n";

const BASE_TEST: &str =
    "use strict;\nuse warnings;\nuse Test::More tests => 1;\nuse Base;\n\nis(Base::value(), 42, 'base');\n";

const CRASH: &str = "use strict;\nuse warnings;\n\ndie \"the ledger is empty\\n\";\n";

fn consumer() -> Case {
    let case = Case::consumer("negative", CPAN_MODULE);
    case.write("BUILD.bazel", ROOT_BUILD);
    case.write("tampered.lock.json", TAMPERED_LOCK);
    case.write("app/BUILD.bazel", BUILD);
    case.write("app/data.txt", "data\n");
    case.write("app/lib/Clean.pm", CLEAN);
    case.write("app/lib/Broken.pm", BROKEN);
    case.write("app/lib/Base.pm", BASE);
    case.write("app/t/clean.t", CLEAN_TEST);
    case.write("app/t/broken.t", BROKEN_TEST);
    case.write("app/t/failing.t", FAILING_TEST);
    case.write("app/t/dies.t", DIES_TEST);
    case.write("app/t/missing.t", MISSING_TEST);
    case.write("app/t/base.t", BASE_TEST);
    case.write("app/bin/crash.pl", CRASH);
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
fn a_clean_test_and_a_declared_dependency_pass() {
    let case = consumer();
    for target in ["//app:clean_test", "//app:declared_dep_test"] {
        let tested = bazel(&case, "test", target, &[]);
        assert_eq!(tested.code, Some(0), "{target}\n{}", tested.text);
    }
}

#[test]
fn building_and_testing_leaves_the_workspace_unchanged() {
    let case = consumer();
    let before = workspace_files(&case);
    let built = bazel(&case, "build", "//app:clean", &[]);
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
fn a_syntax_error_fails_with_the_compiler_diagnostic() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:syntax_error_test",
        &[
            "syntax error at",
            "lib/Broken.pm line 6",
            "Compilation failed in require",
        ],
    );
}

#[test]
fn a_failing_assertion_fails_the_test() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:failing_test",
        &[
            "not ok 1 - the answer is right",
            "got: '41'",
            "expected: '42'",
        ],
    );
}

#[test]
fn a_die_fails_the_test_and_the_binary() {
    let case = consumer();
    test_fails_with(&case, "//app:dies_test", &["the ledger is empty"]);
    let ran = bazel(&case, "run", "//app:crash", &[]);
    assert_ne!(ran.code, Some(0), "{}", ran.text);
    assert!(ran.text.contains("the ledger is empty"), "{}", ran.text);
}

#[test]
fn a_missing_module_fails_with_cant_locate() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:missing_module_test",
        &["Can't locate Missing/Module.pm in @INC"],
    );
}

#[test]
fn an_undeclared_dependency_is_not_on_the_include_path() {
    let case = consumer();
    test_fails_with(
        &case,
        "//app:undeclared_dep_test",
        &["Can't locate Base.pm in @INC"],
    );
}

#[test]
fn a_dependency_without_perl_info_fails_analysis() {
    let case = consumer();
    let built = bazel(&case, "build", "//app:wrong_dep", &[]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(
        built
            .text
            .contains("does not have mandatory providers: 'PerlInfo'"),
        "{}",
        built.text
    );
}

#[test]
fn a_tampered_cpan_lock_fails_the_fetch() {
    let case = consumer();
    let built = bazel(&case, "build", "@tampered_cpan//:Text-CSV", &[]);
    assert_ne!(built.code, Some(0), "{}", built.text);
    assert!(
        built.text.contains("Checksum was dfcaec925a788b0ba41e51bc6d16e21b0e98b4c7af9b79395090add75f5e506f but wanted 0000000000000000000000000000000000000000000000000000000000000000"),
        "{}",
        built.text
    );
}

#[test]
fn a_platform_without_a_rules_perl_toolchain_is_incompatible() {
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
