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
