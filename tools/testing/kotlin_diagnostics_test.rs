use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn kotlinc(source: &str) -> dx_testing::Run {
    let out =
        PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set")).join("classes");
    std::fs::create_dir_all(&out).expect("create output directory");
    let out = out.to_string_lossy().into_owned();
    let stdlib = file("DX_STDLIB").to_string_lossy().into_owned();
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_KOTLINC"),
        &[
            "-Werror",
            "-no-stdlib",
            "-no-reflect",
            "-classpath",
            stdlib.as_str(),
            "-d",
            out.as_str(),
            path.as_str(),
        ],
        &[],
    )
    .expect("kotlinc runs")
}

fn ktfmt(source: &str) -> dx_testing::Run {
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_KTFMT"),
        &[
            "--kotlinlang-style",
            "--dry-run",
            "--set-exit-if-changed",
            path.as_str(),
        ],
        &[],
    )
    .expect("ktfmt runs")
}

fn ktlint(source: &str) -> dx_testing::Run {
    let path = file(source).to_string_lossy().into_owned();
    dx_testing::run(&file("DX_KTLINT"), &[path.as_str()], &[]).expect("ktlint runs")
}

#[test]
fn clean_source_formats_and_lints() {
    let formatted = ktfmt("DX_CLEAN");
    assert!(formatted.status.success(), "{}", formatted.combined());
    assert!(formatted.stdout.trim().is_empty(), "{}", formatted.stdout);
    let linted = ktlint("DX_CLEAN");
    assert!(linted.status.success(), "{}", linted.combined());
    let built = kotlinc("DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
}

#[test]
fn compiler_syntax_error_is_rejected() {
    let run = kotlinc("DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.combined().contains("error:"), "{}", run.combined());
}

#[test]
fn type_error_is_rejected() {
    let run = kotlinc("DX_TYPE");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("type mismatch"),
        "{}",
        run.combined()
    );
}

#[test]
fn compiler_warning_is_an_error() {
    let run = kotlinc("DX_WARNING");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("nchecked cast"),
        "{}",
        run.combined()
    );
    assert!(
        run.combined()
            .contains("warnings found and -Werror specified"),
        "{}",
        run.combined()
    );
}

#[test]
fn syntax_error_is_rejected() {
    let formatted = ktfmt("DX_SYNTAX");
    assert!(!formatted.status.success());
    assert!(
        formatted.combined().contains("Expecting an element"),
        "{}",
        formatted.combined()
    );
    let linted = ktlint("DX_SYNTAX");
    assert!(!linted.status.success());
}

#[test]
fn lint_finding_is_an_error() {
    let run = ktlint("DX_LINT");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("no-wildcard-imports"),
        "{}",
        run.combined()
    );
}

#[test]
fn unformatted_source_is_rejected() {
    let run = ktfmt("DX_FORMAT");
    assert!(!run.status.success());
    assert!(run.stdout.contains("Unformatted.kt"), "{}", run.combined());
}
