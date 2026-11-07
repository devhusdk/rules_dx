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
        run.combined()
            .contains("readability-braces-around-statements"),
        "{}",
        run.combined()
    );
}

#[test]
fn standard_tier_reports_unused_function_and_parameter() {
    let tool = file("DX_CLANG_TIDY");
    let policy = file("DX_TIDY_STANDARD").to_string_lossy().into_owned();
    let config = format!("--config-file={policy}");
    let std_flag = text("DX_STD");
    let path = file("DX_UNUSED").to_string_lossy().into_owned();
    let run = dx_testing::run(
        &tool,
        &[
            config.as_str(),
            path.as_str(),
            "--",
            "-Wall",
            std_flag.as_str(),
        ],
        &[],
    )
    .expect("clang-tidy runs");
    let out = run.combined();
    assert!(out.contains("misc-unused-parameters"), "{out}");
    assert!(out.contains("clang-diagnostic-unused-function"), "{out}");
}

fn profile_output(profile: &str) -> String {
    let tool = file("DX_CLANG_TIDY");
    let policy = file(profile).to_string_lossy().into_owned();
    let config = format!("--config-file={policy}");
    let std_flag = text("DX_STD");
    let path = file("DX_MODERNIZE").to_string_lossy().into_owned();
    dx_testing::run(
        &tool,
        &[config.as_str(), path.as_str(), "--", std_flag.as_str()],
        &[],
    )
    .expect("clang-tidy runs")
    .combined()
}

#[test]
fn strict_profile_reports_modernize_and_standard_does_not() {
    let strict = profile_output("DX_STRICT");
    assert!(strict.contains("modernize-use-using"), "{strict}");
    assert!(strict.contains("misc-unused-parameters"), "{strict}");
    let standard = profile_output("DX_STANDARD");
    assert!(!standard.contains("modernize-use-using"), "{standard}");
    assert!(standard.contains("misc-unused-parameters"), "{standard}");
}
