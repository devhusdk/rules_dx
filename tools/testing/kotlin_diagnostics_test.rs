use dx_testing::env_file as file;
use std::path::PathBuf;

fn compile(label: &str, sources: &[PathBuf], vars: &[&str]) -> dx_testing::Run {
    let out = dx_testing::path_text(&dx_testing::scratch_dir(&format!("classes-{label}")));
    let classpath = dx_testing::classpath(vars);
    let sources: Vec<String> = sources
        .iter()
        .map(|source| source.to_string_lossy().into_owned())
        .collect();
    let mut args = vec![
        "-Werror",
        "-no-stdlib",
        "-no-reflect",
        "-classpath",
        classpath.as_str(),
        "-d",
        out.as_str(),
    ];
    args.extend(sources.iter().map(String::as_str));
    dx_testing::run(&file("DX_KOTLINC"), &args, &[]).expect("kotlinc runs")
}

fn kotlinc(label: &str, source: &str) -> dx_testing::Run {
    compile(label, &[file(source)], &["DX_STDLIB"])
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

#[test]
fn clean_source_formats_and_compiles() {
    let formatted = ktfmt("DX_CLEAN");
    assert!(formatted.status.success(), "{}", formatted.combined());
    assert!(formatted.stdout.trim().is_empty(), "{}", formatted.stdout);
    let built = kotlinc("clean", "DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
}

#[test]
fn compiler_syntax_error_is_rejected() {
    let run = kotlinc("syntax", "DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.combined().contains("error:"), "{}", run.combined());
}

#[test]
fn type_error_is_rejected() {
    let run = kotlinc("type", "DX_TYPE");
    assert!(!run.status.success());
    assert!(
        run.combined().contains("type mismatch"),
        "{}",
        run.combined()
    );
}

#[test]
fn compiler_warning_is_an_error() {
    let run = kotlinc("warning", "DX_WARNING");
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
}

#[test]
fn unformatted_source_is_rejected() {
    let run = ktfmt("DX_FORMAT");
    assert!(!run.status.success());
    assert!(run.stdout.contains("Unformatted.kt"), "{}", run.combined());
}

#[test]
fn app_without_gson_is_rejected() {
    let run = compile(
        "no-gson",
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_STDLIB", "DX_GUAVA"],
    );
    assert!(!run.status.success());
    assert!(
        run.combined().contains("unresolved reference 'gson'"),
        "{}",
        run.combined()
    );
}

#[test]
fn app_without_guava_is_rejected() {
    let run = compile(
        "no-guava",
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_STDLIB", "DX_GSON"],
    );
    assert!(!run.status.success());
    assert!(
        run.combined().contains("unresolved reference 'common'"),
        "{}",
        run.combined()
    );
}

#[test]
fn app_with_declared_dependencies_compiles() {
    let run = compile(
        "declared",
        &dx_testing::env_files("DX_APP_SRCS"),
        &["DX_STDLIB", "DX_GSON", "DX_GUAVA"],
    );
    assert!(run.status.success(), "{}", run.combined());
}
