//! Runs real `rego_test` executables the way `bazel test` does.

use std::process::Command;

fn run(name: &str, filter: Option<&str>) -> (i32, String) {
    let exe = std::env::var(format!("{name}_EXE")).expect("executable path");
    let key = std::env::var(format!("{name}_MANIFEST")).expect("manifest key");
    let scratch = dx_test_scratch::scratch(&format!("opa_run_{name}_{}", filter.unwrap_or("all")));
    let mut command = Command::new(exe);
    command.env("TEST_TMPDIR", scratch.path());
    command.env("DX_REGO_TEST_MANIFEST", key);
    command.env_remove("TESTBRIDGE_TEST_ONLY");
    if let Some(filter) = filter {
        command.env("TESTBRIDGE_TEST_ONLY", filter);
    }
    let output = command.output().expect("run rego_test");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().expect("exit code"), text)
}

#[test]
fn passing_target_exits_zero() {
    let (code, text) = run("PASS", None);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn filter_selects_one_rule() {
    let (code, text) = run("PASS", Some("test_data"));
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("test_value"), "{text}");
}

#[test]
fn failing_target_exits_two_and_names_the_rule() {
    let (code, text) = run("FAIL", None);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("test_always_fails"), "{text}");
}

#[test]
fn target_without_tests_exits_one() {
    let (code, text) = run("EMPTY", None);
    assert_eq!(code, 1, "{text}");
}
