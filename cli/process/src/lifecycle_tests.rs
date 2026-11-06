use super::*;
use dx_testing::{assert_alive_lock_released as assert_released, assert_still_held, wait_armed};
use std::ffi::OsString;
use std::time::{Duration, Instant};

fn probe_argv(flags: &[&str]) -> Vec<OsString> {
    let mut argv = vec![dx_testing::process_probe().into_os_string()];
    argv.extend(flags.iter().map(OsString::from));
    argv
}

fn controlled_spec(argv: Vec<OsString>, timeout: Duration) -> SpawnSpec {
    SpawnSpec {
        argv,
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Controlled { vars: Vec::new() },
        capture: CapturePolicy {
            max_bytes: 1024 * 1024,
        },
        timeout,
        descendants: Descendants::Owned,
    }
}

fn ambient_probe_env() -> Vec<(OsString, OsString)> {
    vec![(OsString::from("DX_LIFECYCLE_PROBE"), OsString::from("kept"))]
}

#[test]
fn controlled_launch_delivers_only_the_declared_environment() {
    let spec = SpawnSpec {
        argv: probe_argv(&[
            "--require-env=DX_LIFECYCLE_PROBE=kept",
            "--forbid-env=TEST_SRCDIR",
        ]),
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Controlled {
            vars: ambient_probe_env(),
        },
        capture: CapturePolicy { max_bytes: 65536 },
        timeout: Duration::from_secs(10),
        descendants: Descendants::Owned,
    };
    match run(&spec).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("controlled probe must exit zero, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn inherited_launch_keeps_the_parent_environment() {
    let spec = SpawnSpec {
        argv: probe_argv(&["--require-env=TEST_SRCDIR"]),
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        capture: CapturePolicy { max_bytes: 65536 },
        timeout: Duration::from_secs(10),
        descendants: Descendants::Owned,
    };
    match run(&spec).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("inherited probe must see TEST_SRCDIR, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn inherited_launch_forwards_extra_entries() {
    let spec = SpawnSpec {
        argv: probe_argv(&["--require-env=DX_LIFECYCLE_PROBE=kept"]),
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Inherited {
            extra: ambient_probe_env(),
        },
        capture: CapturePolicy { max_bytes: 65536 },
        timeout: Duration::from_secs(10),
        descendants: Descendants::Owned,
    };
    match run(&spec).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("extra entry must reach the child, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn exit_code_and_both_streams_round_trip() {
    let spec = controlled_spec(
        probe_argv(&[
            "--stdout-text=out:",
            "--stdout-bytes=2",
            "--stderr-text=err:",
            "--stderr-bytes=1",
            "--exit-code=5",
        ]),
        Duration::from_secs(10),
    );
    match run(&spec).expect("probe runs") {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => {
            assert_eq!(exit.code(), Some(5));
            assert_eq!(stdout, b"out:\0\0");
            assert_eq!(stderr, b"err:\0");
        }
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("probe must finish with code 5, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn signal_exit_is_distinct_from_an_exit_code() {
    let first = controlled_spec(probe_argv(&["--exit-code=3"]), Duration::from_secs(10));
    match run(&first).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit, Exit::Code(3)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("probe must exit 3, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    assert_eq!(Exit::Code(0).code(), Some(0));
    assert_eq!(Exit::Signal(15).code(), None);
}

#[cfg(unix)]
#[test]
fn a_signaled_child_reports_its_signal() {
    let spec = controlled_spec(probe_argv(&["--raise-signal=15"]), Duration::from_secs(10));
    match run(&spec).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit, Exit::Signal(15)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("raised signal must surface, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn timeout_kills_a_sleeping_child_within_a_bound() {
    let spec = controlled_spec(
        probe_argv(&["--sleep-ms=30000"]),
        Duration::from_millis(200),
    );
    let started = Instant::now();
    let outcome = run(&spec).expect("timeout is an outcome");
    assert_eq!(outcome, ChildOutcome::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "timed out after {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
}

#[test]
fn timeout_reaps_a_descendant_holding_the_pipes() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-descendant");
    let lock = scratch.path().join("descendant.alive");
    let spec = controlled_spec(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            "--descendant-stdout-bytes=1048576",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        Duration::from_millis(1000),
    );
    let started = Instant::now();
    let outcome = run(&spec).expect("timeout is an outcome");
    assert_eq!(outcome, ChildOutcome::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "descendant must not hold the run past {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

const INTERACTIVE: CleanupBudget = CleanupBudget {
    graceful: Duration::from_millis(100),
    forced: Duration::from_millis(100),
    drain: Duration::from_millis(100),
};

#[test]
fn the_standard_budget_keeps_its_bounds() {
    assert_eq!(CleanupBudget::STANDARD.total(), Duration::from_secs(9));
    assert_eq!(INTERACTIVE.total(), Duration::from_millis(300));
}

#[test]
fn a_stop_probe_ends_a_blocked_child_before_its_deadline() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-stop-probe");
    let lock = scratch.path().join("child.alive");
    let spec = controlled_spec(
        probe_argv(&[
            "--sleep-ms=30000",
            &format!("--alive-lock={}", lock.display()),
        ]),
        Duration::from_secs(20),
    );
    let started = Instant::now();
    assert_eq!(
        run_within_until(&spec, INTERACTIVE, &|| lock.exists().then_some(15))
            .expect("stop is an outcome"),
        ChildOutcome::TimedOut
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_released(&lock);
}

#[test]
fn a_stop_probe_bounds_a_descendant_holding_stdout() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-stop-descendant");
    let lock = scratch.path().join("descendant.alive");
    let spec = controlled_spec(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            "--descendant-stdout-bytes=1048576",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        Duration::from_secs(3600),
    );
    let started = Instant::now();
    assert_eq!(
        run_within_until(&spec, INTERACTIVE, &|| lock.exists().then_some(15))
            .expect("stop is an outcome"),
        ChildOutcome::TimedOut
    );
    assert!(started.elapsed() < INTERACTIVE.total() + Duration::from_secs(5));
    assert_released(&lock);
}

#[test]
fn a_tight_budget_bounds_a_blocked_child() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-tight-blocked");
    let lock = scratch.path().join("child.alive");
    let spec = controlled_spec(
        probe_argv(&[
            "--sleep-ms=30000",
            &format!("--alive-lock={}", lock.display()),
        ]),
        Duration::from_millis(500),
    );
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    // Hang guard only: the outcome proves the budget bounded the child; a tight
    // wall-clock bound would race a loaded runner.
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the run hung far past a tight budget"
    );
    assert_released(&lock);
}

#[test]
fn a_tight_budget_bounds_a_descendant_holding_stdout_after_exit() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-tight-descendant");
    let lock = scratch.path().join("descendant.alive");
    let mut spec = controlled_spec(
        probe_argv(&[
            "--stdout-text=//pkg:one",
            "--detached-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        Duration::from_millis(500),
    );
    spec.descendants = Descendants::Detachable;
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    // Hang guard only: the outcome proves the budget bounded the child; a tight
    // wall-clock bound would race a loaded runner.
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the run hung far past a tight budget"
    );
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn timeout_kills_a_group_whose_leader_exits_on_term() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-term-group");
    let pidfile = scratch.path().join("child.pid");
    let script = format!(
        "sh -c 'trap \"\" TERM; echo $$ > {pid}; while :; do sleep 1; done' & wait",
        pid = pidfile.display()
    );
    let spec = controlled_spec(
        vec!["sh".into(), "-c".into(), script.into()],
        Duration::from_millis(1000),
    );
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    let pid = std::fs::read_to_string(&pidfile)
        .expect("the descendant recorded its pid")
        .trim()
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            break;
            // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        assert!(
            Instant::now() < deadline,
            // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            "descendant {pid} survived the run"
        );
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        std::thread::sleep(Duration::from_millis(50));
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

fn daemon_probe(lock: &std::path::Path, sleep_ms: u32, descendants: Descendants) -> SpawnSpec {
    let mut spec = controlled_spec(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=5000",
            &format!("--descendant-alive-lock={}", lock.display()),
            &format!("--sleep-ms={sleep_ms}"),
        ]),
        Duration::from_millis(300),
    );
    spec.descendants = descendants;
    spec
}

#[test]
fn a_detachable_run_leaves_a_detached_server_running() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-detachable");
    let lock = scratch.path().join("server.alive");
    let spec = daemon_probe(&lock, 30000, Descendants::Detachable);
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    assert_still_held(&lock);
}

#[test]
fn a_detachable_run_reaps_an_attached_descendant() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-detachable-attached");
    let lock = scratch.path().join("descendant.alive");
    let mut spec = controlled_spec(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        Duration::from_millis(1000),
    );
    spec.descendants = Descendants::Detachable;
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    assert_released(&lock);
}

#[test]
fn attached_targets_is_none_without_a_console() {
    assert_eq!(attached_targets(1, Some(&[2, 3]), &[]), None);
}

#[test]
fn attached_targets_is_none_without_a_member_list() {
    assert_eq!(attached_targets(1, None, &[2, 3]), None);
}

#[test]
fn attached_targets_never_includes_dx() {
    assert_eq!(attached_targets(1, Some(&[1, 2]), &[1, 2]), Some(vec![2]));
}

#[test]
fn attached_targets_skips_members_outside_the_console() {
    assert_eq!(attached_targets(1, Some(&[2, 9]), &[1, 2]), Some(vec![2]));
}

#[test]
fn attached_targets_is_the_intersection_in_any_order() {
    assert_eq!(
        attached_targets(1, Some(&[5, 4, 3]), &[3, 9, 5]),
        Some(vec![5, 3])
    );
}

#[cfg(windows)]
#[test]
fn a_detachable_run_reaps_a_client_but_keeps_its_detached_server() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-detachable-chain");
    let lock = scratch.path().join("server.alive");
    let mut spec = controlled_spec(
        probe_argv(&[
            &format!("--nested-server-lock={}", lock.display()),
            "--sleep-ms=30000",
        ]),
        Duration::from_millis(1000),
    );
    spec.descendants = Descendants::Detachable;
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    assert_still_held(&lock);
}

#[cfg(windows)]
#[test]
fn an_owned_run_kills_a_detached_server() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-owned-server");
    let lock = scratch.path().join("server.alive");
    let spec = daemon_probe(&lock, 0, Descendants::Owned);
    match run_within(&spec, INTERACTIVE).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        outcome => panic!("the probe must finish, got {outcome:?}"),
    }
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn an_owned_run_ends_an_attached_descendant_after_a_clean_exit() {
    let scratch = dx_test_scratch::scratch("dx-lifecycle-owned-attached");
    let lock = scratch.path().join("descendant.alive");
    let mut spec = controlled_spec(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        Duration::from_secs(10),
    );
    spec.descendants = Descendants::Owned;
    match run_within(&spec, INTERACTIVE).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("the probe must finish, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    assert_released(&lock);
}

#[test]
fn a_tight_budget_drains_large_output_concurrently() {
    let mut spec = controlled_spec(
        probe_argv(&["--stdout-bytes=1048576"]),
        Duration::from_secs(5),
    );
    spec.capture = CapturePolicy { max_bytes: 4096 };
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("oversize is an outcome"),
        ChildOutcome::OutputTooLarge { limit: 4096 }
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "a producer past pipe capacity must not wait for the deadline: {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
}

#[test]
fn large_output_drains_without_a_timeout() {
    let spec = controlled_spec(
        probe_argv(&["--stdout-bytes=1048576"]),
        Duration::from_secs(15),
    );
    match run(&spec).expect("large output drains") {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => {
            assert_eq!(exit.code(), Some(0));
            assert_eq!(stdout, vec![0u8; 1_048_576]);
            assert!(stderr.is_empty());
        }
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("megabyte must drain, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn output_past_the_caller_limit_reports_the_limit() {
    let mut spec = controlled_spec(probe_argv(&["--stdout-bytes=17"]), Duration::from_secs(10));
    spec.capture = CapturePolicy { max_bytes: 16 };
    match run(&spec).expect("oversize is an outcome") {
        ChildOutcome::OutputTooLarge { limit } => assert_eq!(limit, 16),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        outcome => panic!("oversize must report the limit, got {outcome:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn empty_argv_is_rejected_before_spawning() {
    let mut spec = controlled_spec(Vec::new(), Duration::from_secs(10));
    spec.capture = CapturePolicy { max_bytes: 16 };
    let err = run(&spec).expect_err("empty argv");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn a_missing_binary_is_an_io_error() {
    let spec = controlled_spec(
        vec![OsString::from("/nonexistent-dx-tool-9f2c1d")],
        Duration::from_secs(10),
    );
    assert!(run(&spec).is_err());
}

#[derive(Clone, Default)]
struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Sink {
    fn bytes(&self) -> Vec<u8> {
        self.0.lock().expect("sink lock").clone()
    }
}

fn streamed(argv: Vec<OsString>, sink: &Sink, group: StreamGroup) -> StreamSpec {
    StreamSpec {
        argv,
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        stdin: StreamStdin::Inherit,
        stdout: StreamOutput::Forward(Box::new(sink.clone())),
        stderr: StreamOutput::Inherit,
        group,
        held: Vec::new(),
    }
}

fn streamed_both(argv: Vec<OsString>, out: &Sink, err: &Sink, group: StreamGroup) -> StreamSpec {
    let mut spec = streamed(argv, out, group);
    spec.stderr = StreamOutput::Forward(Box::new(err.clone()));
    spec
}

const STREAM_BUDGET: CleanupBudget = CleanupBudget {
    graceful: Duration::from_millis(300),
    forced: Duration::from_millis(300),
    drain: Duration::from_millis(400),
};

fn never() -> Option<i32> {
    None
}

fn stop_after(delay: Duration, signal: i32) -> impl Fn() -> Option<i32> {
    let start = Instant::now();
    move || (start.elapsed() >= delay).then_some(signal)
}

#[cfg(any(unix, windows))]
fn stop_after_armed(
    lock: &std::path::Path,
    delay: Duration,
    signal: i32,
) -> impl Fn() -> Option<i32> {
    let lock = lock.to_path_buf();
    let start = Instant::now();
    move || (start.elapsed() >= delay && lock.exists()).then_some(signal)
}

#[cfg(any(unix, windows))]
fn withdraw_after_armed(lock: &std::path::Path, delay: Duration) -> impl Fn() -> bool {
    let lock = lock.to_path_buf();
    let start = Instant::now();
    move || start.elapsed() >= delay && lock.exists()
}

#[test]
fn streamed_exit_code_and_stdout_reach_the_caller() {
    for group in [StreamGroup::Owned, StreamGroup::Terminal] {
        let sink = Sink::default();
        let spec = streamed(
            probe_argv(&["--stdout-text=hello", "--exit-code=3"]),
            &sink,
            group,
        );
        assert_eq!(
            run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
            StreamOutcome::Exited(Exit::Code(3))
        );
        assert_eq!(sink.bytes(), b"hello");
    }
}

#[test]
fn streamed_large_output_is_copied_completely() {
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&["--stdout-bytes=4194304"]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(spec, CleanupBudget::STANDARD, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert_eq!(sink.bytes().len(), 4_194_304);
}

#[test]
fn streamed_controlled_env_delivers_only_the_declared_entries() {
    let sink = Sink::default();
    let mut spec = streamed(
        probe_argv(&[
            "--require-env=DX_LIFECYCLE_PROBE=kept",
            "--forbid-env=TEST_SRCDIR",
        ]),
        &sink,
        StreamGroup::Owned,
    );
    spec.env = EnvPolicy::Controlled {
        vars: ambient_probe_env(),
    };
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
}

#[cfg(unix)]
#[test]
fn streamed_signal_exit_is_reported() {
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&["--raise-signal=15"]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Signal(15))
    );
}

/// Runs a sentinel probe to completion so a wrongly launched sibling, which
/// finishes in the same instant, has written its marker; no fixed sleep.
fn settle_after_skipped_launch(scratch: &std::path::Path) {
    let done = scratch.join("sentinel.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[&format!("--marker={}", done.display())]),
        &sink,
        StreamGroup::Owned,
    );
    run_streamed(spec, STREAM_BUDGET, &never).expect("sentinel runs");
    assert!(done.exists(), "the sentinel probe never finished");
}

#[test]
fn streamed_stop_before_launch_never_spawns() {
    let scratch = dx_test_scratch::scratch("dx-stream-before");
    let marker = scratch.path().join("child.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[&format!("--marker={}", marker.display())]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &|| Some(2)).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
    );
    settle_after_skipped_launch(scratch.path());
    assert!(!marker.exists(), "a stopped run launched its child");
}

#[test]
fn streamed_empty_argv_is_rejected() {
    let sink = Sink::default();
    let spec = streamed(Vec::new(), &sink, StreamGroup::Owned);
    let err = run_streamed(spec, STREAM_BUDGET, &never).expect_err("empty argv");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[cfg(any(unix, windows))]
#[test]
fn streamed_stop_kills_an_owned_tree_that_ignores_it() {
    let scratch = dx_test_scratch::scratch("dx-stream-blocked");
    let lock = scratch.path().join("descendant.alive");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let stop = stop_after_armed(&lock, Duration::from_millis(200), 15);
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 15 }
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[test]
fn streamed_drain_is_bounded_when_a_descendant_keeps_stdout() {
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--stdout-text=first",
            "--detached-descendant",
            "--descendant-sleep-ms=3000",
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "drain took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_eq!(sink.bytes(), b"first");
}

#[test]
fn streamed_stderr_reaches_its_own_sink() {
    for group in [StreamGroup::Owned, StreamGroup::Terminal] {
        let out = Sink::default();
        let err = Sink::default();
        let spec = streamed_both(
            probe_argv(&[
                "--stdout-text=to-out",
                "--stderr-text=to-err",
                "--stderr-bytes=1048576",
                "--exit-code=4",
            ]),
            &out,
            &err,
            group,
        );
        assert_eq!(
            run_streamed(spec, CleanupBudget::STANDARD, &never).expect("probe runs"),
            StreamOutcome::Exited(Exit::Code(4))
        );
        assert_eq!(out.bytes(), b"to-out");
        let err = err.bytes();
        assert!(
            err.starts_with(b"to-err"),
            "{:?}",
            // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            &err[..err.len().min(16)] // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        );
        assert_eq!(err.len(), b"to-err".len() + 1_048_576);
    }
}

#[cfg(unix)]
#[test]
fn streamed_stop_with_both_streams_forwarded_kills_the_owned_tree() {
    let scratch = dx_test_scratch::scratch("dx-stream-both");
    let lock = scratch.path().join("descendant.alive");
    let out = Sink::default();
    let err = Sink::default();
    let spec = streamed_both(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &out,
        &err,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let stop = stop_after_armed(&lock, Duration::from_millis(200), 15);
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 15 }
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[test]
fn streamed_drain_is_bounded_when_a_descendant_keeps_stderr() {
    let out = Sink::default();
    let err = Sink::default();
    let mut spec = streamed_both(
        probe_argv(&[
            "--stderr-text=first",
            "--detached-descendant",
            "--descendant-sleep-ms=3000",
        ]),
        &out,
        &err,
        StreamGroup::Owned,
    );
    spec.stdout = StreamOutput::Inherit;
    let started = Instant::now();
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "drain took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_eq!(err.bytes(), b"first");
    assert!(out.bytes().is_empty());
}

#[cfg(unix)]
#[test]
fn streamed_stop_during_drain_cleans_up_the_owned_descendant() {
    let scratch = dx_test_scratch::scratch("dx-stream-drain");
    let lock = scratch.path().join("descendant.alive");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let budget = CleanupBudget {
        drain: Duration::from_secs(20),
        ..STREAM_BUDGET
    };
    let started = Instant::now();
    let stop = stop_after_armed(&lock, Duration::from_millis(300), 2);
    assert_eq!(
        run_streamed(spec, budget, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop during drain took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[test]
fn streamed_terminal_stop_during_drain_returns_without_waiting() {
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&["--detached-descendant", "--descendant-sleep-ms=3000"]),
        &sink,
        StreamGroup::Terminal,
    );
    let budget = CleanupBudget {
        drain: Duration::from_secs(20),
        ..STREAM_BUDGET
    };
    let started = Instant::now();
    let stop = stop_after(Duration::from_millis(300), 2);
    assert_eq!(
        run_streamed(spec, budget, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop during drain took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
}

/// A probe child that is killed and reaped on drop.
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
fn counting_probe(scratch: &std::path::Path) -> (Probe, std::path::PathBuf) {
    use std::os::unix::process::CommandExt;
    let log = scratch.join("owned.sigint");
    let mut command = Command::new(dx_testing::process_probe());
    command
        .arg(format!("--count-sigint={}", log.display()))
        .arg("--sleep-ms=30000")
        .process_group(0);
    let child = Probe(command.spawn().expect("probe starts"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !log.exists() {
        assert!(Instant::now() < deadline, "probe never armed");
        std::thread::sleep(Duration::from_millis(10));
    }
    (child, log)
}

#[cfg(unix)]
fn sigints(log: &std::path::Path, expected: usize) -> usize {
    let count = || std::fs::read(log).map_or(0, |bytes| bytes.len());
    let deadline = Instant::now() + Duration::from_secs(5);
    while count() < expected && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    count()
}

#[cfg(unix)]
fn assert_survived(probe: &mut Probe) {
    assert_eq!(
        probe.0.try_wait().expect("probe status"),
        None,
        "the probe died from a forwarded SIGINT"
    );
}

#[cfg(unix)]
#[test]
fn owned_child_gets_every_forwarded_sigint() {
    let scratch = dx_test_scratch::scratch("dx-forward-owned");
    let (mut child, log) = counting_probe(scratch.path());
    let target = -(child.0.id() as i32);
    forward(target, 2);
    assert_eq!(sigints(&log, 1), 1);
    forward(target, 2);
    assert_eq!(sigints(&log, 2), 2);
    assert_survived(&mut child);
}

#[cfg(target_os = "linux")]
#[test]
fn a_terminal_run_without_a_tty_puts_the_child_in_its_own_group() {
    let sink = Sink::default();
    let spec = streamed(
        vec![
            OsString::from("/bin/cat"),
            OsString::from("/proc/self/stat"),
        ],
        &sink,
        StreamGroup::Terminal,
    );
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("cat runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    let stat = String::from_utf8(sink.bytes()).expect("utf-8 stat");
    let after_name = &stat[stat.rfind(')').expect("stat names the command") + 2..];
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let pid: i32 = stat
        .split_whitespace()
        .next()
        .expect("pid")
        .parse()
        .expect("pid");
    let pgid: i32 = fields[2].parse().expect("pgrp");
    assert_ne!(pgid, unsafe { libc::getpgrp() }, "{stat}");
    if !terminal_foreground() {
        assert_eq!(pgid, pid, "{stat}");
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}

#[test]
fn streamed_withdraw_before_launch_never_spawns() {
    let scratch = dx_test_scratch::scratch("dx-stream-withdraw-before");
    let marker = scratch.path().join("child.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[&format!("--marker={}", marker.display())]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &|| true).expect("withdraw is an outcome"),
        StreamOutcome::Withdrawn
    );
    settle_after_skipped_launch(scratch.path());
    assert!(!marker.exists(), "a withdrawn run launched its child");
}

#[test]
fn streamed_exit_before_withdraw_reports_the_exit() {
    let sink = Sink::default();
    let spec = streamed(probe_argv(&["--exit-code=3"]), &sink, StreamGroup::Owned);
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &|| false).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(3))
    );
}

#[cfg(unix)]
#[test]
fn streamed_self_exit_stops_the_rest_of_the_owned_group() {
    let scratch = dx_test_scratch::scratch("dx-stream-self-exit-group");
    let lock = scratch.path().join("leftover.alive");
    let sink = Sink::default();
    let script = format!(
        "'{probe}' --alive-lock='{lock}' --sleep-ms=30000 >/dev/null 2>&1 & while [ ! -e '{lock}' ]; do sleep 0.05; done; exit 0",
        probe = dx_testing::process_probe().display(),
        lock = lock.display(),
    );
    let spec = streamed(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("shell runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "self-exit took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn streamed_self_exit_stops_a_descendant_holding_stdout() {
    let scratch = dx_test_scratch::scratch("dx-stream-self-exit-stdout");
    let lock = scratch.path().join("descendant.alive");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "self-exit took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn streamed_self_exit_leaves_a_detached_server_running() {
    let scratch = dx_test_scratch::scratch("dx-stream-self-exit-server");
    let lock = scratch.path().join("server.alive");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=5000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert_still_held(&lock);
}

#[cfg(any(unix, windows))]
#[test]
fn streamed_withdraw_terminates_the_owned_tree() {
    let scratch = dx_test_scratch::scratch("dx-stream-withdraw");
    let lock = scratch.path().join("descendant.alive");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            &format!("--descendant-alive-lock={}", lock.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let withdraw = withdraw_after_armed(&lock, Duration::from_millis(200));
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &withdraw).expect("withdraw is an outcome"),
        StreamOutcome::Withdrawn
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "withdraw took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn streamed_withdraw_kills_a_tree_that_ignores_sigterm() {
    let scratch = dx_test_scratch::scratch("dx-stream-withdraw-ignored");
    let lock = scratch.path().join("alive");
    let sink = Sink::default();
    let script = format!(
        "trap '' TERM; exec '{probe}' --alive-lock='{lock}' --sleep-ms=30000",
        probe = dx_testing::process_probe().display(),
        lock = lock.display(),
    );
    let spec = streamed(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let withdraw = withdraw_after_armed(&lock, Duration::from_millis(200));
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &withdraw).expect("withdraw is an outcome"),
        StreamOutcome::Withdrawn
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "withdraw took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
    assert_released(&lock);
}

#[cfg(unix)]
#[test]
fn streamed_stop_during_withdraw_is_a_cancel() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let sink = Sink::default();
    let script = "trap '' TERM; sleep 3";
    let spec = streamed(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        &sink,
        StreamGroup::Owned,
    );
    let withdrew = AtomicBool::new(false);
    let started = Instant::now();
    let withdraw = || {
        let due = started.elapsed() >= Duration::from_millis(100);
        if due {
            withdrew.store(true, Ordering::SeqCst);
        }
        due
    };
    let stop = || withdrew.load(Ordering::SeqCst).then_some(2);
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &stop, &withdraw).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
    );
    assert!(
        withdrew.load(Ordering::SeqCst),
        "withdraw was never consulted"
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop during withdraw took {:?}",
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        started.elapsed() // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    );
}

#[cfg(unix)]
#[test]
fn streamed_null_stdin_reads_end_of_file() {
    let sink = Sink::default();
    let mut spec = streamed(
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "read line; echo \"read=$?\"".into(),
        ],
        &sink,
        StreamGroup::Owned,
    );
    spec.stdin = StreamStdin::Null;
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("shell runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert_eq!(sink.bytes(), b"read=1\n");
}

fn held_lease(path: &std::path::Path) -> std::fs::File {
    dx_atomic_fs::lease::acquire_shared_at(path, Duration::from_secs(10))
        .expect("hold a shared lease")
        .into_file()
}

fn lease_free(path: &std::path::Path) -> bool {
    matches!(dx_atomic_fs::lease::try_lock_file(path, false), Ok(Some(_)))
}

fn wait_lease_free(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !lease_free(path) {
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        assert!(
            Instant::now() < deadline,
            "the held lease never came free: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(50));
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

fn wait_alive(lock: &std::path::Path) {
    wait_armed(lock);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dx_testing::alive_lock_held(lock) {
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        assert!(
            Instant::now() < deadline,
            "probe never took its alive lock: {}",
            lock.display()
        );
        std::thread::sleep(Duration::from_millis(10));
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[test]
fn streamed_held_lock_is_held_while_the_child_runs() {
    let _serial = crate::lib_tests::held_spawn();
    let scratch = dx_test_scratch::scratch("dx-stream-held-run");
    let lease = scratch.path().join("lease.lock");
    let alive = scratch.path().join("child.alive");
    let sink = Sink::default();
    let mut spec = streamed(
        probe_argv(&[
            "--sleep-ms=1500",
            &format!("--alive-lock={}", alive.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    spec.held = vec![held_lease(&lease)];
    let run = std::thread::spawn(move || run_streamed(spec, STREAM_BUDGET, &never));
    wait_alive(&alive);
    assert!(
        !lease_free(&lease),
        "the lease must be held while the child runs"
    );
    assert_eq!(
        run.join().expect("run thread").expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    wait_lease_free(&lease);
}

#[cfg(unix)]
#[test]
fn streamed_held_lock_outlives_parent_in_descendant() {
    let _serial = crate::lib_tests::held_spawn();
    let scratch = dx_test_scratch::scratch("dx-stream-held-descendant");
    let lease = scratch.path().join("lease.lock");
    let alive = scratch.path().join("descendant.alive");
    let sink = Sink::default();
    let mut spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=2000",
            &format!("--descendant-alive-lock={}", alive.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    spec.held = vec![held_lease(&lease)];
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert!(
        dx_testing::alive_lock_held(&alive),
        "the descendant outlives the run"
    );
    assert!(
        !lease_free(&lease),
        "the descendant keeps the lease after the run returns"
    );
    assert_released(&alive);
    wait_lease_free(&lease);
}

#[cfg(unix)]
#[test]
fn streamed_sibling_spawn_does_not_inherit_held() {
    let _serial = crate::lib_tests::held_spawn();
    let scratch = dx_test_scratch::scratch("dx-stream-held-sibling");
    let lease = scratch.path().join("lease.lock");
    let child_alive = scratch.path().join("child.alive");
    let sibling_alive = scratch.path().join("sibling.alive");
    let held_sink = Sink::default();
    let mut held_spec = streamed(
        probe_argv(&[
            "--sleep-ms=1500",
            &format!("--alive-lock={}", child_alive.display()),
        ]),
        &held_sink,
        StreamGroup::Owned,
    );
    held_spec.held = vec![held_lease(&lease)];
    let held_run = std::thread::spawn(move || run_streamed(held_spec, STREAM_BUDGET, &never));
    wait_alive(&child_alive);
    let sibling_sink = Sink::default();
    let sibling = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=8000",
            &format!("--descendant-alive-lock={}", sibling_alive.display()),
        ]),
        &sibling_sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(sibling, STREAM_BUDGET, &never).expect("sibling runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    assert_eq!(
        held_run.join().expect("run thread").expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    wait_lease_free(&lease);
    assert!(
        dx_testing::alive_lock_held(&sibling_alive),
        "the lease came free only because the sibling's descendant died"
    );
}

#[cfg(unix)]
#[test]
fn streamed_without_held_is_unchanged() {
    let _serial = crate::lib_tests::held_spawn();
    let scratch = dx_test_scratch::scratch("dx-stream-unheld");
    let lease = scratch.path().join("lease.lock");
    let alive = scratch.path().join("descendant.alive");
    let parent_lock = held_lease(&lease);
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=8000",
            &format!("--descendant-alive-lock={}", alive.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &never).expect("probe runs"),
        StreamOutcome::Exited(Exit::Code(0))
    );
    drop(parent_lock);
    assert!(
        lease_free(&lease),
        "a descendant must not inherit a lock the caller did not hand over"
    );
    assert!(dx_testing::alive_lock_held(&alive));
}

const HELD_PARENT_ENV: &str = "DX_STREAM_HELD_PARENT_DIR";

/// Runs as the parent process for the crash tests; a no-op otherwise.
#[test]
fn streamed_held_parent_main() {
    let Some(dir) = std::env::var_os(HELD_PARENT_ENV).map(std::path::PathBuf::from) else {
        return;
    };
    // LCOV_EXCL_START - reason: the probe runs as a child of the held-lease test, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    let alive = dir.join("alive.lock");
    let alive_flag = if cfg!(unix) {
        format!("--descendant-alive-lock={}", alive.display())
    } else {
        format!("--alive-lock={}", alive.display())
    };
    let flags: Vec<&str> = if cfg!(unix) {
        vec![
            "--sleep-ms=3000",
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=3000",
            &alive_flag,
        ]
    } else {
        vec!["--sleep-ms=30000", &alive_flag]
    };
    let sink = Sink::default();
    let mut spec = streamed(probe_argv(&flags), &sink, StreamGroup::Owned);
    spec.held = vec![held_lease(&dir.join("lease.lock"))];
    let _ = run_streamed(spec, STREAM_BUDGET, &never);
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}

/// The returned guard keeps the scratch dir alive while the orphaned
/// descendant still holds the lease.
fn crash_held_parent(
    prefix: &str,
) -> (
    dx_test_scratch::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let scratch = dx_test_scratch::scratch(prefix);
    let mut parent = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .arg("lifecycle::lifecycle_tests::streamed_held_parent_main")
        .arg("--exact")
        .env(HELD_PARENT_ENV, scratch.path())
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("spawn the held parent");
    let alive = scratch.path().join("alive.lock");
    wait_alive(&alive);
    parent.kill().expect("crash the held parent");
    let _ = parent.wait();
    let lease = scratch.path().join("lease.lock");
    (scratch, lease, alive)
}

#[cfg(unix)]
#[test]
fn streamed_held_lock_survives_parent_crash() {
    let (_scratch, lease, alive) = crash_held_parent("dx-stream-held-crash");
    assert!(
        !lease_free(&lease),
        "the running descendant keeps the lease after its parent dies"
    );
    assert!(dx_testing::alive_lock_held(&alive));
    assert_released(&alive);
    wait_lease_free(&lease);
}

#[cfg(windows)]
#[test]
fn streamed_held_tree_dies_with_parent_crash() {
    let (_scratch, lease, alive) = crash_held_parent("dx-stream-held-crash");
    assert_released(&alive);
    wait_lease_free(&lease);
}

#[test]
fn stream_group_is_owned_without_a_terminal() {
    if !terminal_foreground() {
        assert!(matches!(stream_group(), StreamGroup::Owned));
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}

#[cfg(unix)]
#[test]
fn a_prepared_command_keeps_its_removed_variable_out_of_the_child() {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "printf '%s' \"${DX_PREPARED_PROBE-unset}\""])
        .env("DX_PREPARED_PROBE", "leaked")
        .env_remove("DX_PREPARED_PROBE");
    let outcome = run_command_until(
        &mut command,
        &CapturePolicy { max_bytes: 1024 },
        Duration::from_secs(20),
        Descendants::Owned,
        INTERACTIVE,
        &|| None,
    )
    .expect("runs");
    match outcome {
        ChildOutcome::Finished { stdout, .. } => assert_eq!(stdout, b"unset"),
        // LCOV_EXCL_START - reason: failure-only arm or message, never evaluated when the test passes, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        other => panic!("unexpected outcome {other:?}"),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[cfg(unix)]
#[test]
fn a_stop_probe_ends_a_prepared_command_before_its_deadline() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30"]);
    let started = Instant::now();
    let outcome = run_command_until(
        &mut command,
        &CapturePolicy { max_bytes: 1024 },
        Duration::from_secs(20),
        Descendants::Owned,
        INTERACTIVE,
        &|| Some(15),
    )
    .expect("stop is an outcome");
    assert_eq!(outcome, ChildOutcome::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn console_ctrl_c_and_break_act_as_sigint() {
    assert_eq!(console_stop_signal(0), Some(2));
    assert_eq!(console_stop_signal(1), Some(2));
    for other in [2, 5, 6] {
        assert_eq!(console_stop_signal(other), None);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn sending_a_signal_keeps_the_callers_errno() {
    unsafe { *libc::__errno_location() = libc::EINTR };
    backend::send(i32::MAX - 1, 15);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EINTR)
    );
}

#[test]
fn stream_stdin_is_null_only_for_an_inherited_terminal_in_an_own_group() {
    for requested in [StreamStdin::Inherit, StreamStdin::Null] {
        for own_group in [false, true] {
            for tty in [false, true] {
                let expected = if requested == StreamStdin::Inherit && own_group && tty {
                    StreamStdin::Null
                } else {
                    requested
                };
                assert_eq!(
                    stream_stdin(requested, own_group, tty),
                    expected,
                    "{requested:?} own_group={own_group} tty={tty}"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn a_stop_reaches_only_the_runs_still_registered() {
    static TABLE: [Slot; FORWARD_SLOTS] = [const { Slot::new() }; FORWARD_SLOTS];
    fn ignore(_: i32) {}
    let scratch = dx_test_scratch::scratch("dx-forward-table");
    let (mut a, a_log) = counting_probe(scratch.path());
    let other = dx_test_scratch::scratch("dx-forward-table-b");
    let (b, b_log) = counting_probe(other.path());
    let first = Forwarding::register_in(&TABLE, -(a.0.id() as i32));
    let mut second = Forwarding::register_in(&TABLE, -(b.0.id() as i32));
    second.clear();
    forward_from(&TABLE, 2, ignore);
    assert_eq!(sigints(&a_log, 1), 1);
    assert_eq!(sigints(&b_log, 1), 0);
    assert!(first.claimed());
    assert!(!second.claimed());
    assert_survived(&mut a);
}

#[test]
fn a_full_forwarding_table_registers_without_a_slot() {
    static TABLE: [Slot; 1] = [Slot::new()];
    let first = Forwarding::register_in(&TABLE, 4242);
    let second = Forwarding::register_in(&TABLE, 4243);
    assert!(second.slot.is_none());
    assert!(!second.claimed());
    drop(first);
    assert_eq!(TABLE[0].target.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[test]
fn a_stop_seen_after_registering_sends_one_sigint() {
    let scratch = dx_test_scratch::scratch("dx-forward-once");
    let log = scratch.path().join("owned.sigint");
    let sink = Sink::default();
    let spec = streamed(
        vec![
            dx_testing::process_probe().into(),
            format!("--count-sigint={}", log.display()).into(),
            "--sleep-ms=30000".into(),
        ],
        &sink,
        StreamGroup::Owned,
    );
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let stop = || {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return None;
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !log.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        Some(2)
    };
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
    );
    assert_eq!(sigints(&log, 1), 1);
}

#[test]
fn a_new_slot_is_free_and_unclaimed() {
    let slot = Slot::new();
    assert_eq!(slot.target.load(Ordering::SeqCst), 0);
    assert!(!slot.claimed.load(Ordering::SeqCst));
}

#[test]
fn a_zero_target_takes_no_slot() {
    static TABLE: [Slot; 1] = [Slot::new()];
    let forwarding = Forwarding::register_in(&TABLE, 0);
    assert!(forwarding.slot.is_none());
    assert!(!forwarding.claimed());
    assert_eq!(TABLE[0].target.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[test]
fn a_zero_target_is_never_signaled() {
    backend::send(0, 2);
    assert!(!backend::tree_alive(0));
}

#[test]
fn waiting_with_nothing_pending_returns() {
    let mut pending = Vec::new();
    await_drained(&mut pending, Instant::now() + Duration::from_millis(10));
    assert!(pending.is_empty());
}

#[test]
fn a_missing_pipe_drains_to_nothing() {
    let rx = drain_pipe(None::<std::io::Empty>, 16);
    assert!(matches!(
        collect(&rx, Instant::now() + Duration::from_secs(10)),
        Drain::Ready(buf) if buf.is_empty()
    ));
}

struct FlakyPipe {
    reads: u8,
}

impl io::Read for FlakyPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        match self.reads {
            1 => Err(io::ErrorKind::Interrupted.into()),
            2 => {
                buf[..2].copy_from_slice(b"ok");
                Ok(2)
            }
            _ => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}

#[test]
fn a_drain_retries_interrupts_and_keeps_what_it_read_after_an_error() {
    let rx = drain_pipe(Some(FlakyPipe { reads: 0 }), 16);
    assert!(matches!(
        collect(&rx, Instant::now() + Duration::from_secs(10)),
        Drain::Ready(buf) if buf == b"ok"
    ));
}

#[cfg(unix)]
#[test]
fn a_child_that_ignores_term_is_killed_after_the_graceful_wait() {
    let script = "trap '' TERM; exec sleep 30";
    let spec = controlled_spec(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        Duration::from_millis(200),
    );
    let budget = CleanupBudget {
        graceful: Duration::from_millis(200),
        forced: Duration::from_secs(5),
        drain: Duration::from_secs(5),
    };
    assert_eq!(
        run_within(&spec, budget).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
}
