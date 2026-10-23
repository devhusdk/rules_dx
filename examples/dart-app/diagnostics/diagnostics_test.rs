use std::path::PathBuf;

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn dart(args: &[&str], source: &str) -> dx_testing::Run {
    let path = file(source).to_string_lossy().into_owned();
    let mut all = vec!["--suppress-analytics"];
    all.extend_from_slice(args);
    all.push(path.as_str());
    dx_testing::run(&file("DX_DART"), &all, &[]).expect("dart runs")
}

fn compile(source: &str) -> dx_testing::Run {
    let out = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join(format!("{source}.dill"));
    let out = out.to_string_lossy().into_owned();
    dart(&["compile", "kernel", "-o", out.as_str()], source)
}

fn analyze(source: &str) -> dx_testing::Run {
    dart(&["analyze", "--format=machine", "--fatal-infos"], source)
}

fn format(source: &str) -> dx_testing::Run {
    dart(
        &["format", "--output=none", "--set-exit-if-changed"],
        source,
    )
}

#[test]
fn clean_source_compiles_formats_and_analyzes() {
    let built = compile("DX_CLEAN");
    assert!(built.status.success(), "{}", built.combined());
    let formatted = format("DX_CLEAN");
    assert_eq!(formatted.status.code(), Some(0), "{}", formatted.combined());
    let analyzed = analyze("DX_CLEAN");
    assert_eq!(analyzed.status.code(), Some(0), "{}", analyzed.combined());
}

#[test]
fn syntax_error_fails_compile_format_and_analysis() {
    let built = compile("DX_SYNTAX");
    assert!(!built.status.success());
    assert!(
        built.combined().contains("Can't find ')' to match '('"),
        "{}",
        built.combined()
    );
    let formatted = format("DX_SYNTAX");
    assert_eq!(
        formatted.status.code(),
        Some(65),
        "{}",
        formatted.combined()
    );
    let analyzed = analyze("DX_SYNTAX");
    assert_eq!(analyzed.status.code(), Some(3), "{}", analyzed.combined());
    assert!(
        analyzed
            .stdout
            .contains("ERROR|SYNTACTIC_ERROR|EXPECTED_TOKEN|"),
        "{}",
        analyzed.combined()
    );
}

#[test]
fn type_error_fails_compile_and_analysis() {
    let built = compile("DX_TYPE");
    assert!(!built.status.success());
    assert!(
        built
            .combined()
            .contains("can't be returned from a function"),
        "{}",
        built.combined()
    );
    let analyzed = analyze("DX_TYPE");
    assert_eq!(analyzed.status.code(), Some(3), "{}", analyzed.combined());
    assert!(
        analyzed.stdout.contains("|RETURN_OF_INVALID_TYPE|"),
        "{}",
        analyzed.combined()
    );
}

#[test]
fn undeclared_dependency_fails_compile_and_analysis() {
    let built = compile("DX_MISSING_DEP");
    assert!(!built.status.success());
    assert!(
        built.combined().contains("package:args/args.dart"),
        "{}",
        built.combined()
    );
    let analyzed = analyze("DX_MISSING_DEP");
    assert_eq!(analyzed.status.code(), Some(3), "{}", analyzed.combined());
    assert!(
        analyzed.stdout.contains("|URI_DOES_NOT_EXIST|"),
        "{}",
        analyzed.combined()
    );
}

#[test]
fn configured_lints_are_errors_under_fatal_infos() {
    let analyzed = analyze("DX_LINT");
    assert_eq!(analyzed.status.code(), Some(1), "{}", analyzed.combined());
    assert!(
        analyzed.stdout.contains("|LINT|AVOID_PRINT|"),
        "{}",
        analyzed.combined()
    );
    assert!(
        analyzed.stdout.contains("|LINT|PREFER_SINGLE_QUOTES|"),
        "{}",
        analyzed.combined()
    );
}

#[test]
fn unformatted_source_is_rejected_without_rewriting_it() {
    let before = std::fs::read(file("DX_FORMAT")).expect("read source");
    let formatted = format("DX_FORMAT");
    assert_eq!(formatted.status.code(), Some(1), "{}", formatted.combined());
    assert!(
        formatted.stdout.contains("Changed "),
        "{}",
        formatted.combined()
    );
    assert_eq!(
        std::fs::read(file("DX_FORMAT")).expect("read source"),
        before
    );
}
