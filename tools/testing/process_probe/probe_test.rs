use std::path::PathBuf;
use std::time::Instant;

const ABSENT: &str = "DX_PROBE_ABSENT_9F2C1D";

fn probe() -> PathBuf {
    dx_testing::process_probe()
}

fn run(args: &[&str]) -> dx_testing::Run {
    dx_testing::run(&probe(), args, &[]).expect("probe must execute")
}

fn run_with(args: &[&str], envs: &[(&str, &str)]) -> dx_testing::Run {
    dx_testing::run(&probe(), args, envs).expect("probe must execute")
}

#[test]
fn help_names_every_flag_and_exits_zero() {
    let run = run(&["--help"]);
    assert_eq!(run.status.code(), Some(0));
    for flag in [
        "--exit-code",
        "--stdout-bytes",
        "--stderr-bytes",
        "--stdout-text",
        "--stderr-text",
        "--sleep-ms",
        "--require-env",
        "--forbid-env",
        "--print-env",
        "--print-env-all",
        "--print-argv",
        "--print-cwd",
        "--spawn-descendant",
        "--descendant-exit",
        "--descendant-sleep-ms",
        "--descendant-stdout-bytes",
        "--raise-signal",
        "--help",
    ] {
        assert!(
            run.stdout.contains(flag),
            "usage must document {flag}: {}",
            run.stdout
        );
    }
}

#[test]
fn chosen_exit_status_reaches_the_parent() {
    assert_eq!(run(&["--exit-code=0"]).status.code(), Some(0));
    assert_eq!(run(&["--exit-code=7"]).status.code(), Some(7));
    assert_eq!(run(&["--exit-code=255"]).status.code(), Some(255));
}

#[test]
fn stdout_and_stderr_byte_counts_are_exact() {
    let run = run(&["--stdout-bytes=5", "--stderr-bytes=3"]);
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stdout, "\0\0\0\0\0");
    assert_eq!(run.stderr, "\0\0\0");
}

#[test]
fn text_is_written_before_the_byte_fill() {
    let run = run(&[
        "--stdout-text=hi",
        "--stdout-bytes=2",
        "--stderr-text=oops",
        "--stderr-bytes=1",
    ]);
    assert_eq!(run.stdout, "hi\0\0");
    assert_eq!(run.stderr, "oops\0");
}

#[test]
fn sleeping_holds_the_child_before_it_exits() {
    let started = Instant::now();
    let run = run(&["--sleep-ms=250", "--exit-code=4"]);
    assert_eq!(run.status.code(), Some(4));
    assert!(
        started.elapsed().as_millis() >= 200,
        "probe exited after {:?}",
        started.elapsed()
    );
}

#[test]
fn required_environment_pins_a_value() {
    let held = run_with(
        &["--require-env=DX_PROBE_HELD=kept"],
        &[("DX_PROBE_HELD", "kept")],
    );
    assert_eq!(held.status.code(), Some(0));
    let wrong = run_with(
        &["--require-env=DX_PROBE_HELD=kept"],
        &[("DX_PROBE_HELD", "other")],
    );
    assert_eq!(wrong.status.code(), Some(70));
    assert!(
        wrong.stderr.contains("DX_PROBE_HELD must hold kept"),
        "{}",
        wrong.stderr
    );
    let missing = run(&[&format!("--require-env={ABSENT}=kept")]);
    assert_eq!(missing.status.code(), Some(70));
    let unset = run(&[&format!("--require-env={ABSENT}")]);
    assert_eq!(unset.status.code(), Some(70));
}

#[test]
fn forbidden_environment_reports_a_set_name() {
    let clear = run(&[&format!("--forbid-env={ABSENT}")]);
    assert_eq!(clear.status.code(), Some(0));
    let set = run_with(
        &["--forbid-env=DX_PROBE_HELD"],
        &[("DX_PROBE_HELD", "kept")],
    );
    assert_eq!(set.status.code(), Some(71));
    assert!(
        set.stderr.contains("DX_PROBE_HELD is set"),
        "{}",
        set.stderr
    );
}

#[test]
fn print_env_reports_set_and_unset_names() {
    let run = run_with(
        &[
            "--print-env=DX_PROBE_HELD",
            &format!("--print-env={ABSENT}"),
        ],
        &[("DX_PROBE_HELD", "kept")],
    );
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stdout, "kept\n<unset>\n");
}

#[test]
fn print_env_all_lists_sorted_variables() {
    let run = run_with(
        &["--print-env-all"],
        &[("DX_PROBE_HELD", "kept"), ("DX_PROBE_ALPHA", "1")],
    );
    assert_eq!(run.status.code(), Some(0));
    assert!(
        run.stdout.contains("\nDX_PROBE_ALPHA=1\n"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("\nDX_PROBE_HELD=kept\n"),
        "{}",
        run.stdout
    );
    let alpha = run.stdout.find("DX_PROBE_ALPHA=1").expect("alpha variable");
    let held = run
        .stdout
        .find("DX_PROBE_HELD=kept")
        .expect("held variable");
    assert!(
        alpha < held,
        "variables print in sorted order: {}",
        run.stdout
    );
}

#[test]
fn print_argv_echoes_one_argument_per_line() {
    let run = run(&["--print-argv", "--descendant-exit=0"]);
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stdout, "--print-argv\n--descendant-exit=0\n");
}

#[test]
fn print_cwd_reports_the_working_directory() {
    let run = run(&["--print-cwd"]);
    assert_eq!(run.status.code(), Some(0));
    let cwd = std::env::current_dir().expect("readable working directory");
    assert_eq!(run.stdout, format!("{}\n", cwd.to_string_lossy()));
}

#[test]
fn a_descendant_contributes_bytes_and_status() {
    let run = run(&[
        "--spawn-descendant",
        "--stdout-bytes=2",
        "--descendant-stdout-bytes=3",
        "--descendant-exit=9",
    ]);
    assert_eq!(run.status.code(), Some(9));
    assert_eq!(run.stdout.len(), 5, "both generations write to one pipe");
    assert_eq!(run.stdout, "\0\0\0\0\0");
}

#[test]
fn a_descendant_sleeps_before_the_parent_exits() {
    let started = Instant::now();
    let run = run(&["--spawn-descendant", "--descendant-sleep-ms=250"]);
    assert_eq!(run.status.code(), Some(0));
    assert!(
        started.elapsed().as_millis() >= 200,
        "parent exited after {:?}",
        started.elapsed()
    );
}

#[test]
fn a_missing_value_is_a_usage_error() {
    for arg in [
        "--exit-code",
        "--stdout-bytes",
        "--require-env",
        "--forbid-env",
        "--print-env",
    ] {
        let run = run(&[arg]);
        assert_eq!(run.status.code(), Some(64), "{arg} must be rejected");
        assert!(run.stderr.contains("needs a value"), "{}", run.stderr);
    }
}

#[test]
fn a_malformed_value_is_a_usage_error() {
    for arg in [
        "--exit-code=abc",
        "--exit-code=256",
        "--sleep-ms=-1",
        "--nope=1",
    ] {
        let run = run(&[arg]);
        assert_eq!(run.status.code(), Some(64), "{arg} must be rejected");
        assert!(run.stderr.contains("process_probe:"), "{}", run.stderr);
    }
}

#[test]
fn a_switch_with_a_value_is_a_usage_error() {
    for arg in ["--print-cwd=1", "--print-argv=", "--spawn-descendant=yes"] {
        let run = run(&[arg]);
        assert_eq!(run.status.code(), Some(64), "{arg} must be rejected");
        assert!(run.stderr.contains("takes no value"), "{}", run.stderr);
    }
}

#[test]
fn a_usage_error_documents_the_grammar() {
    let run = run(&["--nope=1"]);
    assert_eq!(run.status.code(), Some(64));
    assert!(
        run.stderr.contains("Usage: process_probe"),
        "{}",
        run.stderr
    );
}

#[cfg(unix)]
#[test]
fn a_raised_signal_kills_the_probe() {
    use std::os::unix::process::ExitStatusExt;
    let run = run(&["--raise-signal=15"]);
    assert_eq!(run.status.code(), None);
    assert_eq!(run.status.signal(), Some(15));
}
