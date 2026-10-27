use std::path::PathBuf;
use std::time::{Duration, Instant};

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
        "--gate",
        "--require-env",
        "--forbid-env",
        "--print-env",
        "--print-env-all",
        "--print-argv",
        "--print-cwd",
        "--print-exe",
        "--spawn-descendant",
        "--detached-descendant",
        "--descendant-own-group",
        "--descendant-exit",
        "--descendant-sleep-ms",
        "--descendant-stdout-bytes",
        "--descendant-marker",
        "--marker",
        "--alive-lock",
        "--descendant-alive-lock",
        "--raise-signal",
        "--count-sigint",
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
fn a_gate_file_ends_the_sleep() {
    let dir = std::env::temp_dir().join(format!("probe-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let gate = dir.join("gate");
    std::fs::write(&gate, b"").expect("gate");
    let started = Instant::now();
    let run = run(&[
        &format!("--gate={}", gate.display()),
        "--sleep-ms=30000",
        "--exit-code=5",
    ]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.status.code(), Some(5));
    assert!(
        started.elapsed().as_secs() < 10,
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
fn a_shim_run_records_its_call_and_answers_from_the_environment() {
    let scratch_tmp = dx_test_scratch::scratch("probe-shim");
    let scratch = scratch_tmp.path().to_path_buf();
    let log = scratch.join("argv.txt");
    let log_text = log.to_string_lossy().into_owned();
    let run = run_with(
        &["--nohome_rc", "query", "kind(rule, //pkg:*)"],
        &[
            ("PROCESS_PROBE_SHIM_LOG", &log_text),
            ("PROCESS_PROBE_SHIM_STDOUT", "//pkg:alpha\n"),
            ("PROCESS_PROBE_SHIM_EXIT", "3"),
        ],
    );
    assert_eq!(run.status.code(), Some(3), "{}", run.stderr);
    assert_eq!(run.stdout, "//pkg:alpha\n");
    let cwd = std::env::current_dir().expect("readable working directory");
    let recorded = std::fs::read_to_string(&log).expect("shim log");
    assert_eq!(
        recorded,
        format!(
            "{}\n--nohome_rc\nquery\nkind(rule, //pkg:*)\n",
            cwd.to_string_lossy()
        )
    );
}

#[test]
fn print_exe_reports_the_probe_binary() {
    let run = run(&["--print-exe"]);
    assert_eq!(run.status.code(), Some(0));
    let printed = std::fs::canonicalize(run.stdout.trim()).expect("printed path exists");
    let expected = std::fs::canonicalize(probe()).expect("probe path exists");
    assert_eq!(printed, expected);
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
fn a_waited_descendant_marker_records_a_normal_end() {
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-waited-marker");
    let scratch = scratch_tmp.path().to_path_buf();
    let marker = scratch.join("descendant.finished");
    let run = run(&[
        "--spawn-descendant",
        "--descendant-sleep-ms=10",
        &format!("--descendant-marker={}", marker.display()),
    ]);
    assert_eq!(run.status.code(), Some(0));
    assert!(
        marker.exists(),
        "a surviving descendant creates its marker: {}",
        marker.display()
    );
}

#[test]
fn a_detached_descendant_runs_without_supplying_the_exit_status() {
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-detached-marker");
    let scratch = scratch_tmp.path().to_path_buf();
    let marker = scratch.join("descendant.finished");
    let run = run(&[
        "--detached-descendant",
        "--exit-code=3",
        "--descendant-exit=9",
        "--descendant-sleep-ms=50",
        &format!("--descendant-marker={}", marker.display()),
    ]);
    assert_eq!(
        run.status.code(),
        Some(3),
        "the parent exits with its own code while the descendant runs"
    );
    assert!(
        marker.exists(),
        "the detached descendant runs to its normal end: {}",
        marker.display()
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
        "--marker",
        "--descendant-marker",
        "--alive-lock",
        "--descendant-alive-lock",
    ] {
        let run = run(&[arg]);
        assert_eq!(run.status.code(), Some(64), "{arg} must be rejected");
        assert!(run.stderr.contains("needs a value"), "{}", run.stderr);
    }
    for arg in ["--alive-lock=", "--descendant-alive-lock="] {
        let run = run(&[arg]);
        assert_eq!(run.status.code(), Some(64), "{arg} must be rejected");
        assert!(run.stderr.contains("needs a path"), "{}", run.stderr);
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
    for arg in [
        "--print-cwd=1",
        "--print-argv=",
        "--spawn-descendant=yes",
        "--detached-descendant=1",
    ] {
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

#[cfg(unix)]
struct Probe(std::process::Child);

#[cfg(unix)]
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
fn sigints_logged(log: &std::path::Path, expected: usize) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let bytes = std::fs::read(log).expect("log");
        if bytes.len() >= expected {
            return bytes;
        }
        assert!(
            Instant::now() < deadline,
            "the probe logged {} of {expected} SIGINTs",
            bytes.len()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[test]
fn counted_sigints_are_logged_and_do_not_stop_the_probe() {
    unsafe extern "C" {
        fn kill(pid: i32, signo: i32) -> i32;
    }
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-sigint");
    let scratch = scratch_tmp.path().to_path_buf();
    let log = scratch.join("sigint.log");
    let mut child = Probe(
        std::process::Command::new(probe())
            .arg(format!("--count-sigint={}", log.display()))
            .arg("--sleep-ms=30000")
            .spawn()
            .expect("probe starts"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !log.exists() {
        assert!(Instant::now() < deadline, "probe never armed");
        std::thread::sleep(Duration::from_millis(10));
    }
    for n in 1..=2 {
        assert_eq!(unsafe { kill(child.0.id() as i32, 2) }, 0);
        sigints_logged(&log, n);
    }
    assert_eq!(
        child.0.try_wait().expect("probe status"),
        None,
        "the probe died from a SIGINT"
    );
    assert_eq!(std::fs::read(&log).expect("log"), b"II");
}

#[test]
fn an_own_group_descendant_does_not_hold_the_parent_output() {
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-own-group");
    let scratch = scratch_tmp.path().to_path_buf();
    let marker = scratch.join("descendant.finished");
    let lock = scratch.join("descendant.alive");
    let run = run(&[
        "--detached-descendant",
        "--descendant-own-group",
        "--descendant-sleep-ms=30000",
        "--descendant-stdout-bytes=16",
        &format!("--descendant-marker={}", marker.display()),
        &format!("--descendant-alive-lock={}", lock.display()),
    ]);
    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.is_empty(), "{:?}", run.stdout);
    assert!(
        !marker.exists(),
        "the parent waited for its descendant: {}",
        marker.display()
    );
    assert!(
        dx_testing::alive_lock_held(&lock),
        "output() must return while the descendant lives"
    );
}

#[test]
fn an_alive_lock_is_held_until_the_probe_dies() {
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-alive-lock");
    let scratch = scratch_tmp.path().to_path_buf();
    let lock = scratch.join("probe.alive");
    let mut child = std::process::Command::new(probe())
        .arg(format!("--alive-lock={}", lock.display()))
        .arg("--sleep-ms=30000")
        .spawn()
        .expect("probe starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lock.exists() {
        assert!(Instant::now() < deadline, "probe never armed");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        dx_testing::alive_lock_held(&lock),
        "a live probe must hold its lock"
    );
    child.kill().expect("probe is killed");
    child.wait().expect("probe is reaped");
    dx_testing::assert_alive_lock_released(&lock);
}

#[test]
fn a_detached_descendant_holds_its_lock_before_the_parent_exits() {
    let scratch_tmp = dx_test_scratch::scratch("dx-probe-descendant-lock");
    let scratch = scratch_tmp.path().to_path_buf();
    let lock = scratch.join("descendant.alive");
    let run = run(&[
        "--detached-descendant",
        "--descendant-own-group",
        "--descendant-sleep-ms=30000",
        &format!("--descendant-alive-lock={}", lock.display()),
    ]);
    assert_eq!(run.status.code(), Some(0), "{}", run.stderr);
    assert!(
        lock.exists(),
        "the parent exited before its descendant armed"
    );
    assert!(
        dx_testing::alive_lock_held(&lock),
        "the descendant must hold its lock"
    );
}
