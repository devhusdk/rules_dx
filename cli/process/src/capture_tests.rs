use super::*;
use std::time::Instant;

fn probe(flags: &[&str]) -> Command {
    let mut command = Command::new(dx_testing::process_probe());
    command.args(flags);
    command
}

#[test]
fn captures_both_streams_and_the_exit_code() {
    let captured = run_captured(
        &mut probe(&["--stdout-text=out", "--stderr-text=err", "--exit-code=3"]),
        Duration::from_secs(30),
    )
    .expect("runs");
    assert_eq!(captured.code, Some(3));
    assert!(!captured.success());
    assert_eq!(captured.stdout_text(), "out");
    assert_eq!(captured.stderr_text(), "err");
}

#[test]
fn checked_run_reports_stderr_of_a_failure() {
    let error = run_checked(
        &mut probe(&["--stderr-text=  boom\n", "--exit-code=2"]),
        Duration::from_secs(30),
    )
    .expect_err("non-zero exit fails");
    assert_eq!(error.stderr(), "boom");
    let text = error.to_string();
    assert!(text.ends_with("failed: boom"), "{text}");
}

#[test]
fn a_failure_without_stderr_names_the_exit_code() {
    let error = run_checked(&mut probe(&["--exit-code=4"]), Duration::from_secs(30))
        .expect_err("non-zero exit fails");
    assert!(error.to_string().ends_with("failed with exit 4"), "{error}");
}

#[test]
fn checked_run_returns_output_of_a_success() {
    let captured = run_checked(&mut probe(&["--stdout-text=ok"]), Duration::from_secs(30))
        .expect("zero exit passes");
    assert_eq!(captured.stdout_text(), "ok");
}

#[test]
fn a_sleeping_child_times_out_and_is_stopped() {
    let started = Instant::now();
    let error = run_captured(
        &mut probe(&["--sleep-ms=60000"]),
        Duration::from_millis(300),
    )
    .expect_err("a sleeping child must time out");
    assert!(matches!(error, RunError::TimedOut { .. }), "{error}");
    assert!(error.to_string().contains("timed out after 0s"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn a_missing_program_is_a_spawn_error() {
    let error = run_captured(
        &mut Command::new("/nonexistent-dx-program"),
        Duration::from_secs(5),
    )
    .expect_err("cannot spawn");
    assert!(matches!(error, RunError::Spawn { .. }), "{error}");
    assert!(
        error
            .to_string()
            .starts_with("cannot run nonexistent-dx-program: "),
        "{error}"
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn git_isolated_drops_repository_selection_and_sets_nothing_else() {
    let command = git_isolated(Path::new("/"));
    let env: std::collections::BTreeMap<_, _> = command
        .get_envs()
        .map(|(key, value)| (key.to_string_lossy().into_owned(), value.is_some()))
        .collect();
    for name in GIT_ENV {
        assert_eq!(env.get(name), Some(&false), "{name}");
    }
    assert_eq!(env.len(), GIT_ENV.len());
    assert_eq!(command.get_current_dir(), Some(Path::new("/")));
}

#[test]
fn plain_git_leaves_the_environment_alone() {
    let command = git(Path::new("/"));
    assert_eq!(command.get_envs().count(), 0);
    assert_eq!(command.get_current_dir(), Some(Path::new("/")));
}

#[test]
fn git_runs_in_the_given_directory() {
    let scratch = dx_test_scratch::scratch("dx-process-git-");
    let mut command = git(scratch.path());
    command.args(["init", "-q"]);
    dx_test_scratch::isolate_git(&mut command, scratch.path());
    run_checked(&mut command, DEFAULT_TIMEOUT).expect("init");
    assert!(scratch.path().join(".git").is_dir());
}

#[test]
fn input_reaches_stdin_of_the_child() {
    let scratch = dx_test_scratch::scratch("dx-process-input-");
    let mut init = git(scratch.path());
    init.args(["init", "-q"]);
    dx_test_scratch::isolate_git(&mut init, scratch.path());
    run_checked(&mut init, DEFAULT_TIMEOUT).expect("init");
    let mut hash = git(scratch.path());
    hash.args(["hash-object", "--stdin"]);
    dx_test_scratch::isolate_git(&mut hash, scratch.path());
    let captured = run_captured_with_input(&mut hash, Some(b"hello\n"), DEFAULT_TIMEOUT)
        .expect("hash")
        .into_success()
        .expect("zero exit");
    assert_eq!(
        captured.stdout_text().trim(),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );
}

#[test]
fn a_foreground_child_still_times_out_and_is_stopped() {
    let started = Instant::now();
    let error = run_captured_with(
        &mut probe(&["--sleep-ms=60000"]),
        None,
        Duration::from_millis(300),
        RunOptions {
            foreground: true,
            ..RunOptions::default()
        },
    )
    .expect_err("a sleeping child must time out");
    assert!(matches!(error, RunError::TimedOut { .. }), "{error}");
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn output_is_unbounded_unless_a_cap_is_set() {
    let captured = run_captured(
        &mut probe(&["--stdout-bytes=70000"]),
        Duration::from_secs(60),
    )
    .expect("runs");
    assert_eq!(captured.stdout.len(), 70000);
    let error = run_captured_with(
        &mut probe(&["--stdout-bytes=70000"]),
        None,
        Duration::from_secs(60),
        RunOptions {
            max_bytes: Some(1024),
            ..RunOptions::default()
        },
    )
    .expect_err("the cap rejects the output");
    assert!(matches!(error, RunError::OutputTooLarge { .. }), "{error}");
    assert!(error.to_string().contains("wrote more than"), "{error}");
}
