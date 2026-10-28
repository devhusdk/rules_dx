use std::path::PathBuf;

use dx_testing::{resolve_runfiles, run};

fn input(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} is not set"));
    resolve_runfiles(&rel)
}

fn run_var(var: &str, args: &[&str]) -> dx_testing::Run {
    let bin = input(var);
    run(&bin, args, &[]).unwrap_or_else(|error| panic!("cannot run {}: {error}", bin.display()))
}

fn stylua_check(path: &str) -> dx_testing::Run {
    run_var("STYLUA", &["--check", path])
}

#[test]
fn luac_reports_the_file_and_line_of_a_syntax_error() {
    let source = input("SYNTAX_ERROR");
    let out = run_var("LUAC", &["-p", &source.to_string_lossy()]);
    let text = out.combined();
    assert!(
        !out.status.success(),
        "luac accepted a syntax error:\n{text}"
    );
    assert!(
        text.contains("syntax_error.lua:2:"),
        "no file:line in:\n{text}"
    );
}

#[test]
fn a_runtime_error_exits_one_with_a_traceback() {
    let out = run_var("RUNTIME_ERROR", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("runtime boom"), "{text}");
    assert!(text.contains("stack traceback:"), "{text}");
}

#[test]
fn an_undeclared_module_fails_and_names_the_module() {
    let out = run_var("MISSING_DEP", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("no module 'absent.module' in the deps"),
        "{text}"
    );
}

#[test]
fn stylua_accepts_every_example_source() {
    let list = std::env::var("STYLUA_CLEAN").expect("STYLUA_CLEAN is not set");
    for rel in list.split_whitespace() {
        let path = resolve_runfiles(rel);
        let out = stylua_check(&path.to_string_lossy());
        assert!(
            out.status.success(),
            "stylua rejects {rel}:\n{}",
            out.combined()
        );
    }
}

#[test]
fn stylua_rejects_unformatted_source() {
    let source = input("UNFORMATTED");
    let out = stylua_check(&source.to_string_lossy());
    assert!(!out.status.success(), "stylua accepted unformatted source");
}
