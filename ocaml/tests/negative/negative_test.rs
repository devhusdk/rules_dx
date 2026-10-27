use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const BUILD: &str = r#"load("@rules_dx//ocaml/rules:defs.bzl", "ocaml_module", "ocaml_test")

ocaml_module(
    name = "clean",
    struct = "clean.ml",
)

ocaml_module(
    name = "syntax_error",
    struct = "syntax_error.ml",
)

ocaml_module(
    name = "missing_dep",
    struct = "missing_dep.ml",
)

ocaml_module(
    name = "tested",
    struct = "tested.ml",
)

ocaml_module(
    name = "failing_test_main",
    testonly = True,
    struct = "failing_test.ml",
    deps = [":tested"],
)

ocaml_test(
    name = "failing_test",
    size = "small",
    main = ":failing_test_main",
)
"#;

const CLEAN: &str = "let add a b = a + b\n";

const SYNTAX: &str = "let add a b =\n  a +\n";

const NEEDS_DEP: &str = "let value () = Base_dep.value ()\n";

const TESTED: &str = "let answer () = 41\n";

const FAILING_TEST: &str = "let () =\n  if Tested.answer () <> 42 then begin\n    prerr_endline \"the answer is wrong: 41\";\n    exit 1\n  end\n";

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
    let source = resolve_runfiles("ocaml/tests/negative/negative_test.rs");
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
        &read(&rules.join("tools/bazelrc/preset.bazelrc")),
    );
    write(&root.join("BUILD.bazel"), "");
    write(&root.join("ocaml/BUILD.bazel"), BUILD);
    write(&root.join("ocaml/clean.ml"), CLEAN);
    write(&root.join("ocaml/syntax_error.ml"), SYNTAX);
    write(&root.join("ocaml/missing_dep.ml"), NEEDS_DEP);
    write(&root.join("ocaml/tested.ml"), TESTED);
    write(&root.join("ocaml/failing_test.ml"), FAILING_TEST);
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
fn a_clean_module_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//ocaml:clean", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    fails_with(
        &case,
        "//ocaml:syntax_error",
        &["ocaml/syntax_error.ml\", line 3", "Error: Syntax error"],
    );
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    fails_with(
        &case,
        "//ocaml:missing_dep",
        &[
            "ocaml/missing_dep.ml\", line 1",
            "Error: Unbound module Base_dep",
        ],
    );
}

#[test]
fn a_failing_test_fails_with_its_message() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        "//ocaml:failing_test",
        &["--test_output=all"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(
        tested.text.contains("the answer is wrong: 41"),
        "{}",
        tested.text
    );
}
