use std::path::{Path, PathBuf};

fn file(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} is set")))
}

fn julia_bin() -> PathBuf {
    let key = std::env::var("DX_JULIA").expect("DX_JULIA is set");
    if let Some(dir) = std::env::var_os("RUNFILES_DIR") {
        let direct = PathBuf::from(dir).join(&key);
        if direct.exists() {
            return direct;
        }
    }
    dx_testing::resolve_runfiles(&key)
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set")).join(name);
    std::fs::create_dir_all(&dir).expect("scratch dir is created");
    dir
}

fn julia(home: &Path, args: &[&str]) -> dx_testing::Run {
    let depot = home.join("depot");
    let env = [
        ("HOME", home.to_string_lossy().into_owned()),
        ("USERPROFILE", home.to_string_lossy().into_owned()),
        ("JULIA_DEPOT_PATH", depot.to_string_lossy().into_owned()),
    ];
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    dx_testing::run(&julia_bin(), args, &env).expect("julia runs")
}

fn run_file(name: &str, var: &str) -> dx_testing::Run {
    let home = scratch(name);
    let path = file(var).to_string_lossy().into_owned();
    let run = julia(&home, &["--startup-file=no", "--color=no", path.as_str()]);
    assert!(
        !home.join(".julia").exists(),
        "julia created a user depot in {}",
        home.display()
    );
    run
}

fn format_check(paths: &[String]) -> dx_testing::Run {
    let mut args = vec!["--check"];
    args.extend(paths.iter().map(String::as_str));
    dx_testing::run(&file("DX_FORMAT"), &args, &[]).expect("julia_format runs")
}

#[test]
fn clean_source_runs() {
    let run = run_file("clean", "DX_CLEAN");
    assert!(run.status.success(), "{}", run.combined());
    assert_eq!(run.stdout.trim(), "Hello, julia!");
}

#[test]
fn syntax_error_is_rejected_with_its_position() {
    let run = run_file("syntax", "DX_SYNTAX");
    assert!(!run.status.success());
    assert!(run.stderr.contains("ParseError:"), "{}", run.stderr);
    assert!(run.stderr.contains("syntax_error.jl:3:1"), "{}", run.stderr);
    assert!(run.stderr.contains("invalid identifier"), "{}", run.stderr);
}

#[test]
fn runtime_error_names_the_line() {
    let run = run_file("runtime", "DX_RUNTIME");
    assert!(!run.status.success());
    assert!(run.stderr.contains("boom 1"), "{}", run.stderr);
    assert!(run.stderr.contains("runtime_error.jl:2"), "{}", run.stderr);
    assert!(run.stderr.contains("runtime_error.jl:5"), "{}", run.stderr);
}

#[test]
fn missing_dependency_is_reported() {
    let run = run_file("missing", "DX_MISSING_DEP");
    assert!(!run.status.success());
    assert!(
        run.stderr.contains("Package OrderedCollections not found"),
        "{}",
        run.stderr
    );
}

#[test]
fn unformatted_source_fails_the_format_check() {
    let path = file("DX_UNFORMATTED").to_string_lossy().into_owned();
    let run = format_check(std::slice::from_ref(&path));
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(run.stdout.trim(), path);
}

#[test]
fn example_sources_are_formatted() {
    let srcs = std::env::var("DX_FORMAT_SRCS").expect("DX_FORMAT_SRCS is set");
    let paths: Vec<String> = srcs
        .split_whitespace()
        .map(|rel| {
            dx_testing::resolve_runfiles(rel)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(paths.len() > 5);
    let run = format_check(&paths);
    assert!(run.status.success(), "{}", run.combined());
}
