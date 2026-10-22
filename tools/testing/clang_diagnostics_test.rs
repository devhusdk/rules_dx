use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn text(var: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| panic!("{var} is set"))
}

fn compile(source: &str) -> dx_testing::Run {
    let clang = file("DX_CLANG");
    let std_flag = text("DX_STD");
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &clang,
        &[
            "-fsyntax-only",
            "-Wall",
            "-Wextra",
            "-Werror",
            std_flag.as_str(),
            path.as_str(),
        ],
        &[],
    )
    .expect("clang runs")
}

fn tidy(source: &str) -> dx_testing::Run {
    let tool = file("DX_CLANG_TIDY");
    let std_flag = text("DX_STD");
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &tool,
        &[
            "--checks=-*,readability-braces-around-statements",
            "--warnings-as-errors=*",
            path.as_str(),
            "--",
            std_flag.as_str(),
        ],
        &[],
    )
    .expect("clang-tidy runs")
}

#[test]
fn clean_source_compiles_and_lints() {
    let built = compile("DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
    let linted = tidy("DX_CLEAN");
    assert!(linted.status.success(), "{}", linted.combined());
}

#[test]
fn syntax_error_is_rejected() {
    let run = compile("DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.stderr.contains("expected"), "{}", run.stderr);
}

#[test]
fn type_error_is_rejected() {
    let run = compile("DX_TYPE");
    assert!(!run.status.success());
    assert!(run.stderr.contains("error:"), "{}", run.stderr);
}

#[test]
fn lint_finding_is_an_error() {
    let run = tidy("DX_LINT");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("readability-braces-around-statements"),
        "{}",
        run.combined()
    );
}
