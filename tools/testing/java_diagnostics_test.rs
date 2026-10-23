use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn classpath(vars: &[&str]) -> String {
    std::env::join_paths(vars.iter().map(|var| file(var)))
        .expect("classpath entries join")
        .to_string_lossy()
        .into_owned()
}

fn compile(label: &str, sources: &[PathBuf], vars: &[&str]) -> dx_testing::Run {
    let out = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join(format!("classes-{label}"));
    std::fs::create_dir_all(&out).expect("create output directory");
    let out = out.to_string_lossy().into_owned();
    let path = classpath(vars);
    let sources: Vec<String> = sources
        .iter()
        .map(|source| source.to_string_lossy().into_owned())
        .collect();
    let mut args = vec!["-Xlint:all", "-Werror", "-proc:none", "-d", out.as_str()];
    if !vars.is_empty() {
        args.extend(["-classpath", path.as_str()]);
    }
    args.extend(sources.iter().map(String::as_str));
    dx_testing::run(&file("DX_JAVAC"), &args, &[]).expect("javac runs")
}

fn javac(label: &str, source: &str) -> dx_testing::Run {
    compile(label, &[file(source)], &[])
}

fn app_sources() -> Vec<PathBuf> {
    std::env::var("DX_APP_SRCS")
        .expect("DX_APP_SRCS lists the app sources")
        .split_whitespace()
        .map(dx_testing::resolve_runfiles)
        .collect()
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
    let built = javac("clean", "DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
    let linted = checkstyle("DX_CLEAN");
    assert!(linted.status.success(), "{}", linted.combined());
    let formatted = format_check("DX_CLEAN");
    assert!(formatted.status.success(), "{}", formatted.combined());
}

#[test]
fn syntax_error_is_rejected() {
    let run = javac("syntax", "DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.stderr.contains("';' expected"), "{}", run.stderr);
}

#[test]
fn type_error_is_rejected() {
    let run = javac("type", "DX_TYPE");
    assert!(!run.status.success());
    assert!(run.stderr.contains("incompatible types"), "{}", run.stderr);
}

#[test]
fn compiler_warning_is_an_error() {
    let run = javac("warning", "DX_WARNING");
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

#[test]
fn app_without_gson_is_rejected() {
    let run = compile("no-gson", &app_sources(), &["DX_GUAVA", "DX_ANNOTATIONS"]);
    assert!(!run.status.success());
    assert!(
        run.stderr
            .contains("package com.google.gson does not exist"),
        "{}",
        run.stderr
    );
}

#[test]
fn app_without_guava_is_rejected() {
    let run = compile("no-guava", &app_sources(), &["DX_GSON", "DX_ANNOTATIONS"]);
    assert!(!run.status.success());
    assert!(
        run.stderr
            .contains("package com.google.common.collect does not exist"),
        "{}",
        run.stderr
    );
}

#[test]
fn app_with_declared_dependencies_compiles() {
    let run = compile(
        "declared",
        &app_sources(),
        &["DX_GSON", "DX_GUAVA", "DX_ANNOTATIONS"],
    );
    assert!(run.status.success(), "{}", run.combined());
}
