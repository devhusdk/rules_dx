use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn javac(source: &str) -> dx_testing::Run {
    let out =
        PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set")).join("classes");
    std::fs::create_dir_all(&out).expect("create output directory");
    let out = out.to_string_lossy().into_owned();
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_JAVAC"),
        &[
            "-Xlint:all",
            "-Werror",
            "-proc:none",
            "-d",
            out.as_str(),
            path.as_str(),
        ],
        &[],
    )
    .expect("javac runs")
}

fn checkstyle(source: &str) -> dx_testing::Run {
    let config = file("DX_CHECKSTYLE_CONFIG").to_string_lossy().into_owned();
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_CHECKSTYLE"),
        &["-c", config.as_str(), path.as_str()],
        &[],
    )
    .expect("checkstyle runs")
}

fn format_check(source: &str) -> dx_testing::Run {
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_FORMATTER"),
        &["--dry-run", "--set-exit-if-changed", path.as_str()],
        &[],
    )
    .expect("formatter runs")
}

#[test]
fn clean_source_compiles_lints_and_formats() {
    let built = javac("DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
    let linted = checkstyle("DX_CLEAN");
    assert!(linted.status.success(), "{}", linted.combined());
    let formatted = format_check("DX_CLEAN");
    assert!(formatted.status.success(), "{}", formatted.combined());
}

#[test]
fn syntax_error_is_rejected() {
    let run = javac("DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.stderr.contains("';' expected"), "{}", run.stderr);
}

#[test]
fn type_error_is_rejected() {
    let run = javac("DX_TYPE");
    assert!(!run.status.success());
    assert!(run.stderr.contains("incompatible types"), "{}", run.stderr);
}

#[test]
fn compiler_warning_is_an_error() {
    let run = javac("DX_WARNING");
    assert!(!run.status.success());
    assert!(run.stderr.contains("[rawtypes]"), "{}", run.stderr);
}

#[test]
fn lint_finding_is_an_error() {
    let run = checkstyle("DX_LINT");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("Unused import"),
        "{}",
        run.combined()
    );
}

#[test]
fn unformatted_source_is_rejected() {
    let run = format_check("DX_FORMAT");
    assert!(!run.status.success());
    assert!(
        run.stdout.contains("Unformatted.java"),
        "{}",
        run.combined()
    );
}
