use super::*;
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
        outcome => panic!("controlled probe must exit zero, got {outcome:?}"),
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
        outcome => panic!("inherited probe must see TEST_SRCDIR, got {outcome:?}"),
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
        outcome => panic!("extra entry must reach the child, got {outcome:?}"),
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
        outcome => panic!("probe must finish with code 5, got {outcome:?}"),
    }
}

#[test]
fn signal_exit_is_distinct_from_an_exit_code() {
    let first = controlled_spec(probe_argv(&["--exit-code=3"]), Duration::from_secs(10));
    match run(&first).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit, Exit::Code(3)),
        outcome => panic!("probe must exit 3, got {outcome:?}"),
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
        outcome => panic!("raised signal must surface, got {outcome:?}"),
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
        started.elapsed()
    );
}

#[test]
fn timeout_reaps_a_descendant_holding_the_pipes() {
    let scratch = dx_testing::mkscratch("dx-lifecycle-descendant").expect("scratch");
    let marker = scratch.join("descendant.finished");
    let spec = controlled_spec(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=1500",
            "--descendant-stdout-bytes=1048576",
            &format!("--descendant-marker={}", marker.display()),
        ]),
        Duration::from_millis(500),
    );
    let started = Instant::now();
    let outcome = run(&spec).expect("timeout is an outcome");
    assert_eq!(outcome, ChildOutcome::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "descendant must not hold the run past {:?}",
        started.elapsed()
    );
    let deadline = started + Duration::from_millis(1900);
    loop {
        assert!(
            !marker.exists(),
            "the descendant survived the run: {}",
            marker.display()
        );
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
fn a_tight_budget_bounds_a_blocked_child() {
    let scratch = dx_testing::mkscratch("dx-lifecycle-tight-blocked").expect("scratch");
    let marker = scratch.join("child.finished");
    let spec = controlled_spec(
        probe_argv(&["--sleep-ms=2000", &format!("--marker={}", marker.display())]),
        Duration::from_millis(200),
    );
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1500),
        "a blocked child held a tight budget for {elapsed:?}"
    );
    std::thread::sleep(Duration::from_millis(2500).saturating_sub(started.elapsed()));
    assert!(!marker.exists(), "the blocked child survived the run");
}

#[test]
fn a_tight_budget_bounds_a_descendant_holding_stdout_after_exit() {
    let scratch = dx_testing::mkscratch("dx-lifecycle-tight-descendant").expect("scratch");
    let marker = scratch.join("descendant.finished");
    let spec = controlled_spec(
        probe_argv(&[
            "--stdout-text=//pkg:one",
            "--detached-descendant",
            "--descendant-sleep-ms=2000",
            &format!("--descendant-marker={}", marker.display()),
        ]),
        Duration::from_millis(200),
    );
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1500),
        "a descendant held a tight budget for {elapsed:?}"
    );
    std::thread::sleep(Duration::from_millis(2500).saturating_sub(started.elapsed()));
    assert!(!marker.exists(), "the owned descendant survived the run");
}

fn daemon_probe(marker: &std::path::Path, sleep_ms: u32, descendants: Descendants) -> SpawnSpec {
    let mut spec = controlled_spec(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-own-group",
            "--descendant-sleep-ms=1500",
            &format!("--descendant-marker={}", marker.display()),
            &format!("--sleep-ms={sleep_ms}"),
        ]),
        Duration::from_millis(300),
    );
    spec.descendants = descendants;
    spec
}

#[test]
fn a_detachable_run_leaves_a_detached_server_running() {
    let scratch = dx_testing::mkscratch("dx-lifecycle-detachable").expect("scratch");
    let marker = scratch.join("server.finished");
    let spec = daemon_probe(&marker, 30000, Descendants::Detachable);
    let started = Instant::now();
    assert_eq!(
        run_within(&spec, INTERACTIVE).expect("timeout is an outcome"),
        ChildOutcome::TimedOut
    );
    while !marker.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the detached server died with the run: {}",
            marker.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
#[test]
fn an_owned_run_kills_a_detached_server() {
    let scratch = dx_testing::mkscratch("dx-lifecycle-owned-server").expect("scratch");
    let marker = scratch.join("server.finished");
    let spec = daemon_probe(&marker, 0, Descendants::Owned);
    let started = Instant::now();
    match run_within(&spec, INTERACTIVE).expect("probe runs") {
        ChildOutcome::Finished { exit, .. } => assert_eq!(exit.code(), Some(0)),
        outcome => panic!("the probe must finish, got {outcome:?}"),
    }
    assert_never_finishes(&marker, started, Duration::from_millis(3500));
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
        started.elapsed()
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
        outcome => panic!("megabyte must drain, got {outcome:?}"),
    }
}

#[test]
fn output_past_the_caller_limit_reports_the_limit() {
    let mut spec = controlled_spec(probe_argv(&["--stdout-bytes=17"]), Duration::from_secs(10));
    spec.capture = CapturePolicy { max_bytes: 16 };
    match run(&spec).expect("oversize is an outcome") {
        ChildOutcome::OutputTooLarge { limit } => assert_eq!(limit, 16),
        outcome => panic!("oversize must report the limit, got {outcome:?}"),
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
        stdout: StreamStdout::Forward(Box::new(sink.clone())),
        group,
    }
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

fn assert_never_finishes(marker: &std::path::Path, started: Instant, window: Duration) {
    std::thread::sleep(window.saturating_sub(started.elapsed()));
    assert!(
        !marker.exists(),
        "an owned process survived: {}",
        marker.display()
    );
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

#[test]
fn streamed_stop_before_launch_never_spawns() {
    let scratch = dx_testing::mkscratch("dx-stream-before").expect("scratch");
    let marker = scratch.join("child.finished");
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
    std::thread::sleep(Duration::from_millis(300));
    assert!(!marker.exists(), "a stopped run launched its child");
}

#[test]
fn streamed_empty_argv_is_rejected() {
    let sink = Sink::default();
    let spec = streamed(Vec::new(), &sink, StreamGroup::Owned);
    let err = run_streamed(spec, STREAM_BUDGET, &never).expect_err("empty argv");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[cfg(unix)]
#[test]
fn streamed_stop_kills_an_owned_tree_that_ignores_it() {
    let scratch = dx_testing::mkscratch("dx-stream-blocked").expect("scratch");
    let marker = scratch.join("descendant.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=3000",
            &format!("--descendant-marker={}", marker.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let stop = stop_after(Duration::from_millis(200), 15);
    assert_eq!(
        run_streamed(spec, STREAM_BUDGET, &stop).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 15 }
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "stop took {:?}",
        started.elapsed()
    );
    assert_never_finishes(&marker, started, Duration::from_millis(3500));
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
        started.elapsed()
    );
    assert_eq!(sink.bytes(), b"first");
}

#[cfg(unix)]
#[test]
fn streamed_stop_during_drain_cleans_up_the_owned_descendant() {
    let scratch = dx_testing::mkscratch("dx-stream-drain").expect("scratch");
    let marker = scratch.join("descendant.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--detached-descendant",
            "--descendant-sleep-ms=3000",
            &format!("--descendant-marker={}", marker.display()),
        ]),
        &sink,
        StreamGroup::Owned,
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
        started.elapsed()
    );
    assert_never_finishes(&marker, started, Duration::from_millis(3500));
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
        started.elapsed()
    );
}

#[cfg(unix)]
fn counting_probe(
    scratch: &std::path::Path,
    group: StreamGroup,
) -> (std::process::Child, std::path::PathBuf) {
    use std::os::unix::process::CommandExt;
    let log = scratch.join(format!("{group:?}.sigint"));
    let mut command = Command::new(dx_testing::process_probe());
    command
        .arg(format!("--count-sigint={}", log.display()))
        .arg("--sleep-ms=1000");
    if group == StreamGroup::Owned {
        command.process_group(0);
    }
    let child = command.spawn().expect("probe starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !log.exists() {
        assert!(Instant::now() < deadline, "probe never armed");
        std::thread::sleep(Duration::from_millis(10));
    }
    (child, log)
}

#[cfg(unix)]
fn sigints(log: &std::path::Path) -> usize {
    std::thread::sleep(Duration::from_millis(150));
    std::fs::read(log).expect("sigint log").len()
}

#[cfg(unix)]
#[test]
fn terminal_child_counts_one_sigint_per_terminal_interrupt() {
    let scratch = dx_testing::mkscratch("dx-forward-terminal").expect("scratch");
    let (mut child, log) = counting_probe(&scratch, StreamGroup::Terminal);
    let target = child.id() as i32;
    backend::send(target, 2);
    forward(target, 2, StopOrigin::Terminal);
    assert_eq!(
        sigints(&log),
        1,
        "a terminal interrupt reached the child twice"
    );
    forward(target, 2, StopOrigin::Process);
    assert_eq!(
        sigints(&log),
        2,
        "a kill from a process must reach the child"
    );
    assert_eq!(child.wait().expect("probe exits").code(), Some(0));
}

#[cfg(unix)]
#[test]
fn owned_child_gets_every_forwarded_sigint() {
    let scratch = dx_testing::mkscratch("dx-forward-owned").expect("scratch");
    let (mut child, log) = counting_probe(&scratch, StreamGroup::Owned);
    let target = -(child.id() as i32);
    forward(target, 2, StopOrigin::Terminal);
    assert_eq!(sigints(&log), 1);
    forward(target, 2, StopOrigin::Process);
    assert_eq!(sigints(&log), 2);
    assert_eq!(child.wait().expect("probe exits").code(), Some(0));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn stop_origin_reads_si_code() {
    assert_eq!(stop_origin(0x80, 0), StopOrigin::Terminal);
    assert_eq!(stop_origin(0, 0), StopOrigin::Process);
    assert_eq!(stop_origin(0, 4242), StopOrigin::Process);
    assert_eq!(stop_origin(-1, 0), StopOrigin::Process);
    assert_eq!(stop_origin(-6, 4242), StopOrigin::Process);
}

#[cfg(unix)]
fn withdraw_after(delay: Duration) -> impl Fn() -> bool {
    let start = Instant::now();
    move || start.elapsed() >= delay
}

#[test]
fn streamed_withdraw_before_launch_never_spawns() {
    let scratch = dx_testing::mkscratch("dx-stream-withdraw-before").expect("scratch");
    let marker = scratch.join("child.finished");
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
    std::thread::sleep(Duration::from_millis(300));
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
fn streamed_withdraw_terminates_the_owned_tree() {
    let scratch = dx_testing::mkscratch("dx-stream-withdraw").expect("scratch");
    let marker = scratch.join("descendant.finished");
    let sink = Sink::default();
    let spec = streamed(
        probe_argv(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=3000",
            &format!("--descendant-marker={}", marker.display()),
        ]),
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let withdraw = withdraw_after(Duration::from_millis(200));
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &withdraw).expect("withdraw is an outcome"),
        StreamOutcome::Withdrawn
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "withdraw took {:?}",
        started.elapsed()
    );
    assert_never_finishes(&marker, started, Duration::from_millis(3500));
}

#[cfg(unix)]
#[test]
fn streamed_withdraw_kills_a_tree_that_ignores_sigterm() {
    let scratch = dx_testing::mkscratch("dx-stream-withdraw-ignored").expect("scratch");
    let marker = scratch.join("finished");
    let sink = Sink::default();
    let script = format!("trap '' TERM; sleep 3; touch '{}'", marker.display());
    let spec = streamed(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        &sink,
        StreamGroup::Owned,
    );
    let started = Instant::now();
    let withdraw = withdraw_after(Duration::from_millis(200));
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &never, &withdraw).expect("withdraw is an outcome"),
        StreamOutcome::Withdrawn
    );
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "withdraw took {:?}",
        started.elapsed()
    );
    assert_never_finishes(&marker, started, Duration::from_millis(3500));
}

#[cfg(unix)]
#[test]
fn streamed_stop_during_withdraw_is_a_cancel() {
    let sink = Sink::default();
    let script = "trap '' TERM; sleep 3";
    let spec = streamed(
        vec!["/bin/sh".into(), "-c".into(), script.into()],
        &sink,
        StreamGroup::Owned,
    );
    let withdraw = withdraw_after(Duration::from_millis(100));
    let stop = stop_after(Duration::from_millis(200), 2);
    assert_eq!(
        run_streamed_until(spec, STREAM_BUDGET, &stop, &withdraw).expect("stop is an outcome"),
        StreamOutcome::Cancelled { signal: 2 }
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
