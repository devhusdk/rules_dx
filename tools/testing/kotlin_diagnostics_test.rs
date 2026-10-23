use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn compile(label: &str, sources: &[PathBuf], vars: &[&str]) -> dx_testing::Run {
    let out = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join(format!("classes-{label}"));
    std::fs::create_dir_all(&out).expect("create output directory");
    let out = out.to_string_lossy().into_owned();
    let classpath = std::env::join_paths(vars.iter().map(|var| file(var)))
        .expect("classpath entries join")
        .to_string_lossy()
        .into_owned();
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

fn app_sources() -> Vec<PathBuf> {
    std::env::var("DX_APP_SRCS")
        .expect("DX_APP_SRCS lists the app sources")
        .split_whitespace()
        .map(dx_testing::resolve_runfiles)
        .collect()
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

#[test]
fn app_without_gson_is_rejected() {
    let run = compile("no-gson", &app_sources(), &["DX_STDLIB", "DX_GUAVA"]);
    assert!(!run.status.success());
    assert!(
        run.combined().contains("unresolved reference 'gson'"),
        "{}",
        run.combined()
    );
}

#[test]
fn app_without_guava_is_rejected() {
    let run = compile("no-guava", &app_sources(), &["DX_STDLIB", "DX_GSON"]);
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
        &app_sources(),
        &["DX_STDLIB", "DX_GSON", "DX_GUAVA"],
    );
    assert!(run.status.success(), "{}", run.combined());
}
