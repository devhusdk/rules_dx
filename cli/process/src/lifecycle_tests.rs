use super::*;

fn probe_flags(flags: &[&str]) -> Vec<OsString> {
    let mut argv = vec![dx_testing::process_probe().into_os_string()];
    argv.extend(flags.iter().map(|flag| OsString::from(*flag)));
    argv
}

fn capture() -> CapturePolicy {
    CapturePolicy {
        max_output_bytes: 1024 * 1024,
    }
}

fn spec_with(argv: Vec<OsString>, timeout: Option<Duration>) -> SpawnSpec {
    SpawnSpec {
        argv,
        cwd: std::env::temp_dir(),
        stdin: StdinPolicy::Inherit,
        tree: TreePolicy::Owned,
        timeout,
    }
}

fn inherited_env() -> EnvPolicy {
    EnvPolicy::Inherited {
        extra: Vec::new(),
    }
}

#[test]
fn exit_status_round_trips_through_typed_kinds() {
    for code in [0, 1, 127, 255] {
        let status = status_from(&ExitKind::Code(code));
        assert_eq!(status.code(), Some(code));
        assert_eq!(classify(&status), ExitKind::Code(code));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        for signal in [2, 9, 15] {
            let status = ExitStatus::from_raw(signal);
            assert_eq!(classify(&status), ExitKind::Signaled { signal });
            let rebuilt = status_from(&ExitKind::Signaled { signal });
            assert_eq!(ExitStatusExt::signal(&rebuilt), Some(signal));
        }
    }
}

#[test]
fn spawn_failures_carry_the_spawn_stage() {
    let empty = spec_with(Vec::new(), Some(Duration::from_secs(5)));
    match run(&empty, &inherited_env(), &capture()) {
        ChildOutcome::Failed {
            stage: FailureStage::Spawn,
            error,
        } => {
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(error.to_string(), "invocation needs a binary");
        }
        other => panic!("expected a spawn failure, got {other:?}"),
    }
    let missing = spec_with(
        vec![OsString::from("/nonexistent-dx-lifecycle-probe")],
        Some(Duration::from_secs(5)),
    );
    match run(&missing, &inherited_env(), &capture()) {
        ChildOutcome::Failed {
            stage: FailureStage::Spawn,
            error,
        } => assert_eq!(error.kind(), io::ErrorKind::NotFound),
        other => panic!("expected a spawn failure, got {other:?}"),
    }
}

#[test]
fn completed_runs_capture_exact_bytes() {
    let spec = spec_with(
        probe_flags(&[
            "--stdout-text=out:",
            "--stdout-bytes=2",
            "--stderr-text=err:",
            "--stderr-bytes=1",
            "--exit-code=5",
        ]),
        Some(Duration::from_secs(10)),
    );
    match run(&spec, &inherited_env(), &capture()) {
        ChildOutcome::Completed {
            exit: ExitKind::Code(5),
            stdout,
            stderr,
        } => {
            assert_eq!(stdout, b"out:\0\0");
            assert_eq!(stderr, b"err:\0");
        }
        other => panic!("expected a completed run, got {other:?}"),
    }
}

#[test]
fn controlled_env_is_the_only_environment() {
    let spec = spec_with(
        probe_flags(&[
            "--forbid-env=PATH",
            "--require-env=DX_LIFECYCLE_PROBE=1",
            "--exit-code=0",
        ]),
        Some(Duration::from_secs(10)),
    );
    let env = EnvPolicy::Controlled {
        vars: vec![(
            OsString::from("DX_LIFECYCLE_PROBE"),
            OsString::from("1"),
        )],
    };
    match run(&spec, &env, &capture()) {
        ChildOutcome::Completed {
            exit: ExitKind::Code(0),
            ..
        } => {}
        other => panic!("expected a cleared environment to run, got {other:?}"),
    }
    let wants_parent = spec_with(
        probe_flags(&["--require-env=TEST_SRCDIR", "--exit-code=0"]),
        Some(Duration::from_secs(10)),
    );
    match run(&wants_parent, &env, &capture()) {
        ChildOutcome::Completed {
            exit: ExitKind::Code(70),
            ..
        } => {}
        other => panic!("expected the parent environment to stay hidden, got {other:?}"),
    }
}

#[test]
fn inherited_env_keeps_the_parent_and_adds_pairs() {
    let keeps_parent = spec_with(
        probe_flags(&["--require-env=TEST_SRCDIR", "--exit-code=0"]),
        Some(Duration::from_secs(10)),
    );
    match run(&keeps_parent, &inherited_env(), &capture()) {
        ChildOutcome::Completed {
            exit: ExitKind::Code(0),
            ..
        } => {}
        other => panic!("expected the parent environment, got {other:?}"),
    }
    let extra = EnvPolicy::Inherited {
        extra: vec![(
            OsString::from("DX_LIFECYCLE_PROBE"),
            OsString::from("kept"),
        )],
    };
    let wants_extra = spec_with(
        probe_flags(&["--require-env=DX_LIFECYCLE_PROBE=kept", "--exit-code=0"]),
        Some(Duration::from_secs(10)),
    );
    match run(&wants_extra, &extra, &capture()) {
        ChildOutcome::Completed {
            exit: ExitKind::Code(0),
            ..
        } => {}
        other => panic!("expected the extra pair, got {other:?}"),
    }
}

#[test]
fn capture_bounds_come_from_the_caller() {
    let spec = spec_with(
        probe_flags(&["--stdout-bytes=32"]),
        Some(Duration::from_secs(10)),
    );
    let tight = CapturePolicy {
        max_output_bytes: 8,
    };
    match run(&spec, &inherited_env(), &tight) {
        ChildOutcome::Failed {
            stage: FailureStage::Io,
            error,
        } => {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(error.to_string(), "tool output exceeds max size 8 bytes");
        }
        other => panic!("expected the caller limit to fail, got {other:?}"),
    }
}

#[test]
fn timeout_stops_a_sleeping_child() {
    let spec = spec_with(
        probe_flags(&["--sleep-ms=30000"]),
        Some(Duration::from_millis(50)),
    );
    let started = Instant::now();
    match run(&spec, &inherited_env(), &capture()) {
        ChildOutcome::TimedOut => {}
        other => panic!("expected a timeout, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the timeout returned after {:?}",
        started.elapsed()
    );
}

#[test]
fn timeout_returns_before_a_pipe_holding_descendant_finishes() {
    let spec = spec_with(
        probe_flags(&[
            "--spawn-descendant",
            "--descendant-sleep-ms=30000",
            "--descendant-stdout-bytes=1024",
        ]),
        Some(Duration::from_millis(200)),
    );
    let started = Instant::now();
    match run(&spec, &inherited_env(), &capture()) {
        ChildOutcome::TimedOut => {}
        other => panic!("expected a timeout, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the pipe-holding descendant kept the run open for {:?}",
        started.elapsed()
    );
}
