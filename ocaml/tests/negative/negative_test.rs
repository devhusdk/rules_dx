use dx_nested::Case;

const BUILD: &str = r#"load("@rules_dx//cc/rules:defs.bzl", "cc_library")
load("@rules_dx//ocaml/rules:defs.bzl", "ocaml_binary", "ocaml_module", "ocaml_test")

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

cc_library(
    name = "good_stub",
    srcs = ["good_stub.c"],
)

cc_library(
    name = "bad_stub",
    srcs = ["bad_stub.c"],
)

ocaml_module(
    name = "good_binding",
    struct = "binding.ml",
    cc_deps = {":good_stub": "static"},
)

ocaml_module(
    name = "bad_binding",
    struct = "binding.ml",
    cc_deps = {":bad_stub": "static"},
)

ocaml_module(
    name = "bare_binding",
    struct = "binding.ml",
)

ocaml_binary(
    name = "good_app",
    main = ":good_binding",
)

ocaml_binary(
    name = "bad_app",
    main = ":bad_binding",
)

ocaml_binary(
    name = "bare_app",
    main = ":bare_binding",
)
"#;

const BINDING: &str = "external add : (int[@untagged]) -> (int[@untagged]) -> (int[@untagged]) = \"dx_add_bytecode\" \"dx_add\" [@@noalloc]\n\nlet () = if add 1 2 <> 3 then exit 1\n";

const GOOD_STUB: &str = "long dx_add(long a, long b) { return a + b; }\n";

const BAD_STUB: &str = "long dx_add(long a, long b) { return a +; }\n";

const NATIVE: &str = "--@rules_dx//ocaml/toolchain:emitter=sys";

const CLEAN: &str = "let add a b = a + b\n";

const SYNTAX: &str = "let add a b =\n  a +\n";

const NEEDS_DEP: &str = "let value () = Base_dep.value ()\n";

const TESTED: &str = "let answer () = 41\n";

const FAILING_TEST: &str = "let () =\n  if Tested.answer () <> 42 then begin\n    prerr_endline \"the answer is wrong: 41\";\n    exit 1\n  end\n";

fn consumer() -> Case {
    let case = Case::consumer("negative", "");
    case.write("BUILD.bazel", "");
    case.write("ocaml/BUILD.bazel", BUILD);
    case.write("ocaml/clean.ml", CLEAN);
    case.write("ocaml/syntax_error.ml", SYNTAX);
    case.write("ocaml/missing_dep.ml", NEEDS_DEP);
    case.write("ocaml/tested.ml", TESTED);
    case.write("ocaml/failing_test.ml", FAILING_TEST);
    case.write("ocaml/binding.ml", BINDING);
    case.write("ocaml/good_stub.c", GOOD_STUB);
    case.write("ocaml/bad_stub.c", BAD_STUB);
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

#[test]
fn a_clean_module_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//ocaml:clean", &[]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    case.build_fails_with(
        "//ocaml:syntax_error",
        &["ocaml/syntax_error.ml\", line 3", "Error: Syntax error"],
    );
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    case.build_fails_with(
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

#[test]
fn a_native_binary_with_its_stub_builds() {
    let case = consumer();
    let built = bazel(&case, "build", "//ocaml:good_app", &[NATIVE]);
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_native_binary_without_cc_deps_fails_to_link() {
    let case = consumer();
    let built = bazel(&case, "build", "//ocaml:bare_app", &[NATIVE]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(built.text.contains("dx_add"), "{}", built.text);
}

#[test]
fn malformed_c_fails_the_native_build() {
    let case = consumer();
    let built = bazel(&case, "build", "//ocaml:bad_app", &[NATIVE]);
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(built.text.contains("bad_stub.c"), "{}", built.text);
}

#[test]
fn native_actions_name_the_hermetic_c_tools_and_no_host_path() {
    let case = consumer();
    let queried = bazel(
        &case,
        "aquery",
        "mnemonic(\"CompileOCamlModule|LinkOCaml.*\", deps(//ocaml:good_app_upstream))",
        &[NATIVE, "--output=text"],
    );
    assert_eq!(queried.code, Some(0), "{}", queried.text);
    for name in [
        "CONDA_OCAML_AR",
        "CONDA_OCAML_AS",
        "CONDA_OCAML_CC",
        "CONDA_OCAML_MKEXE",
    ] {
        assert!(
            queried.text.contains(&format!("{name}=external/")),
            "{name}\n{}",
            queried.text
        );
        assert!(
            !queried.text.contains(&format!("{name}=/")),
            "{name}\n{}",
            queried.text
        );
    }
    assert!(
        queried.text.contains("PATH=external/") && !queried.text.contains("PATH=/usr"),
        "{}",
        queried.text
    );
}
