use dx_nested::Case;

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

const NIF_BUILD: &str = r#"load("@rules_dx//erlang/rules:defs.bzl", "erlang_nif")

erlang_nif(
    name = "probe_nif",
    srcs = ["probe_nif.c"],
    hdrs = HDRS,
)
"#;

const NIF_SOURCE: &str = "#include <erl_nif.h>\n#include \"probe/hidden.h\"\n\nstatic ERL_NIF_TERM probe(ErlNifEnv *env, int argc, const ERL_NIF_TERM argv[]) {\n  (void)argc;\n  (void)argv;\n  return enif_make_int(env, HIDDEN);\n}\n\nstatic ErlNifFunc funcs[] = {{\"probe\", 0, probe, 0}};\n\nERL_NIF_INIT(probe, funcs, NULL, NULL, NULL, NULL)\n";

const NIF_HEADER: &str = "#define HIDDEN 1\n";

const CLEAN: &str = "-module(clean).\n\n-export([add/2]).\n\nadd(A, B) -> A + B.\n";

const SYNTAX: &str = "-module(syntax_error).\n\n-export([add/2]).\n\nadd(A, B) -> A + .\n";

const WARNING: &str =
    "-module(unused_variable).\n\n-export([add/2]).\n\nadd(A, B) -> Unused = A, B.\n";

const NEEDS_DEP: &str = "-module(needs_dep).\n\n-include_lib(\"base_dep/include/base_dep.hrl\").\n\n-export([value/0]).\n\nvalue() -> ?BASE_VALUE.\n";

const BASE_HEADER: &str = "-define(BASE_VALUE, 42).\n";

const FAILING_SUITE: &str = "-module(failing_SUITE).\n\n-export([all/0, always_fails/1]).\n\nall() -> [always_fails].\n\nalways_fails(_Config) -> 1 = 2.\n";

const FAILING_SRC: &str = "-module(failing).\n\n-export([ok/0]).\n\nok() -> ok.\n";

fn app(case: &Case, name: &str, module: &str, source: &str) {
    case.write(
        format!("{name}/BUILD.bazel"),
        &APP_BUILD.replace("{name}", name),
    );
    case.write(format!("{name}/src/{module}.erl"), source);
}

fn consumer() -> Case {
    let case = Case::consumer("negative", "");
    case.write("BUILD.bazel", "");
    app(&case, "clean", "clean", CLEAN);
    app(&case, "syntax_error", "syntax_error", SYNTAX);
    app(&case, "unused_variable", "unused_variable", WARNING);
    app(&case, "base_dep", "base_dep", "-module(base_dep).\n");
    case.write("base_dep/include/base_dep.hrl", BASE_HEADER);
    for (name, deps) in [
        ("missing_dep", "[]"),
        ("declared_dep", "[\"//base_dep:erlang_app\"]"),
    ] {
        case.write(
            format!("{name}/BUILD.bazel"),
            &format!("DEPS = {deps}\n\n{DEP_BUILD}"),
        );
        case.write(format!("{name}/src/needs_dep.erl"), NEEDS_DEP);
    }
    case.write("failing/BUILD.bazel", SUITE_BUILD);
    case.write("failing/src/failing.erl", FAILING_SRC);
    case.write("failing/test/failing_SUITE.erl", FAILING_SUITE);
    for (name, hdrs) in [
        ("undeclared_header", "[]"),
        ("declared_header", "[\"hidden.h\"]"),
    ] {
        case.write(
            format!("{name}/BUILD.bazel"),
            &format!("HDRS = {hdrs}\n\n{NIF_BUILD}"),
        );
        case.write(
            format!("{name}/probe_nif.c"),
            &NIF_SOURCE.replace("probe/hidden.h", &format!("{name}/hidden.h")),
        );
        case.write(format!("{name}/hidden.h"), NIF_HEADER);
    }
    case.write("portable/BUILD.bazel", PLATFORMS_BUILD);
    case.write("portable/src/portable.erl", "-module(portable).\n");
    case
}

#[test]
fn a_clean_app_builds() {
    let case = consumer();
    let built = case.plain("build", "//clean:erlang_app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
}

#[test]
fn a_syntax_error_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    case.build_fails_with(
        "//syntax_error:erlang_app",
        &["syntax_error.erl:5:18: syntax error before: '.'"],
    );
}

#[test]
fn a_warning_fails_the_build_with_its_diagnostic() {
    let case = consumer();
    case.build_fails_with(
        "//unused_variable:erlang_app",
        &["unused_variable.erl:5:14: variable 'Unused' is unused"],
    );
}

#[test]
fn an_undeclared_dependency_fails_the_build() {
    let case = consumer();
    case.build_fails_with(
        "//missing_dep:erlang_app",
        &["can't find include lib \"base_dep/include/base_dep.hrl\""],
    );
}

#[test]
fn a_declared_dependency_builds() {
    let case = consumer();
    let built = case.plain("build", "//declared_dep:erlang_app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
}

#[test]
fn an_undeclared_nif_header_fails_the_build() {
    let case = consumer();
    case.build_fails_with(
        "//undeclared_header:probe_nif",
        &["'undeclared_header/hidden.h' file not found"],
    );
}

#[test]
fn a_declared_nif_header_builds() {
    let case = consumer();
    let built = case.plain("build", "//declared_header:probe_nif", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
}

#[test]
fn a_failing_suite_fails_the_test() {
    let case = consumer();
    let tested = case.plain("test", "//failing:failing_SUITE", &["--test_output=all"]);
    assert_eq!(tested.status.code(), Some(3), "{}", tested.combined());
    assert!(
        tested.combined().contains("always_fails"),
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
