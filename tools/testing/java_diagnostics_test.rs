use dx_testing::env_file as file;
use std::path::PathBuf;

fn compile(label: &str, sources: &[PathBuf], vars: &[&str]) -> dx_testing::Run {
    let out = dx_testing::path_text(&dx_testing::scratch_dir(&format!("classes-{label}")));
    let path = dx_testing::classpath(vars);
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

fn pmd(source: &str) -> dx_testing::Run {
    let ruleset = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join("ruleset.xml");
    std::fs::write(
        &ruleset,
        "<?xml version=\"1.0\"?>\n<ruleset name=\"t\" xmlns=\"http://pmd.sourceforge.net/ruleset/2.0.0\">\n  <description>t</description>\n  <rule ref=\"category/java/codestyle.xml/UnnecessaryImport\"/>\n</ruleset>\n",
    )
    .expect("write ruleset");
    let ruleset = ruleset.to_string_lossy().into_owned();
    let original = file(source);
    let dir = dx_testing::scratch_dir(&format!("pmd-{source}"));
    let copy = dir.join(original.file_name().expect("source has a file name"));
    std::fs::copy(&original, &copy).expect("copy source");
    let path = copy.to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_PMD"),
        &[
            "check",
            "--no-cache",
            "--rulesets",
            ruleset.as_str(),
            "--dir",
            path.as_str(),
        ],
        &[],
    )
    .expect("pmd runs")
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
    let linted = pmd("DX_CLEAN");
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
    let run = pmd("DX_LINT");
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
    let run = compile(
        "no-gson",
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_GUAVA", "DX_ANNOTATIONS"],
    );
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
    let run = compile(
        "no-guava",
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_GSON", "DX_ANNOTATIONS"],
    );
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
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_GSON", "DX_GUAVA", "DX_ANNOTATIONS"],
    );
    assert!(run.status.success(), "{}", run.combined());
}
