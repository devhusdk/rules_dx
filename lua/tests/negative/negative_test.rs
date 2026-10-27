use std::path::PathBuf;

use dx_testing::{resolve_runfiles, run};

fn input(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} is not set"));
    resolve_runfiles(&rel)
}

fn run_bin(var: &str, args: &[&str]) -> dx_testing::Run {
    let bin = input(var);
    run(&bin, args, &[]).unwrap_or_else(|error| panic!("cannot run {}: {error}", bin.display()))
}

#[test]
fn luac_reports_the_file_and_line_of_a_syntax_error() {
    let source = input("SYNTAX_ERROR");
    let source_arg = source.to_string_lossy().into_owned();
    let out = run_bin("LUAC", &["-p", &source_arg]);
    let text = out.combined();
    assert!(
        !out.status.success(),
        "luac accepted a syntax error:\n{text}"
    );
    assert!(
        text.contains("syntax_error.lua:4:"),
        "no file:line in:\n{text}"
    );
}

#[test]
fn a_runtime_error_exits_one_with_a_traceback() {
    let out = run_bin("RUNTIME_ERROR", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("runtime boom"), "{text}");
    assert!(text.contains("stack traceback:"), "{text}");
    assert!(text.contains("runtime_error.lua:2:"), "{text}");
}

#[test]
fn an_undeclared_module_fails_and_names_the_module() {
    let out = run_bin("MISSING_DEP", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("module 'absent.module' not found"), "{text}");
    assert!(
        text.contains("no module 'absent.module' in the deps"),
        "{text}"
    );
}

#[test]
fn c_modules_are_never_searched() {
    let out = run_bin("CPATH_DENIED", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("module 'socket.core' not found"), "{text}");
    assert!(!text.contains(".so'"), "a C path was searched:\n{text}");
}

#[test]
fn arguments_and_exit_codes_pass_through() {
    let out = run_bin("PROBE", &["agent", "3"]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(3), "{text}");
    assert!(out.stdout.contains("hello, agent"), "{text}");
}
