use std::sync::atomic::{AtomicI32, Ordering};

use super::*;
use crate::lib_tests::probe_argv;

fn runner() -> BinaryRunner {
    BinaryRunner {
        inherit_stdout: false,
        stop: || None,
        on_stop: |_| {},
    }
}

static STOPPED: AtomicI32 = AtomicI32::new(0);

fn cancelling() -> BinaryRunner {
    BinaryRunner {
        inherit_stdout: false,
        stop: || Some(15),
        on_stop: |signo| STOPPED.store(signo, Ordering::SeqCst),
    }
}

fn cwd() -> PathBuf {
    std::env::temp_dir()
}

#[test]
fn run_keeps_exit_codes() {
    for code in [0, 3, 42] {
        let argv = probe_argv(&[&format!("--exit-code={code}")]);
        let status = runner().run(&argv, &cwd(), &[]).expect("run");
        assert_eq!(status.code, Some(code));
    }
}

#[test]
fn run_passes_extra_env() {
    let argv = probe_argv(&["--require-env=DX_RUNNER_PROBE=forwarded"]);
    let ok = runner()
        .run(&argv, &cwd(), &[("DX_RUNNER_PROBE", "forwarded")])
        .expect("run");
    assert_eq!(ok.code, Some(0));
    let bad = runner()
        .run(&argv, &cwd(), &[("DX_RUNNER_PROBE", "other")])
        .expect("run");
    assert_eq!(bad.code, Some(70));
}

#[test]
fn run_hermetic_clears_ambient_env() {
    let argv = probe_argv(&["--forbid-env=TEST_SRCDIR"]);
    let hermetic = runner().run_hermetic(&argv, &cwd(), &[]).expect("hermetic");
    assert_eq!(hermetic.code, Some(0));
    let plain = runner().run(&argv, &cwd(), &[]).expect("run");
    assert_ne!(plain.code, Some(0));
}

#[test]
fn run_hermetic_forwards_extra_env() {
    let argv = probe_argv(&["--require-env=DX_RUNNER_PROBE=forwarded"]);
    let status = runner()
        .run_hermetic(&argv, &cwd(), &[("DX_RUNNER_PROBE", "forwarded")])
        .expect("hermetic");
    assert_eq!(status.code, Some(0));
}

#[test]
fn bad_invocations_are_errors() {
    let empty = runner().run(&[], &cwd(), &[]).expect_err("empty argv");
    assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
    let missing = ["/nonexistent/dx-runner-binary".to_owned()];
    assert!(runner().run(&missing, &cwd(), &[]).is_err());
}

#[test]
fn run_withdrawable_honours_withdraw() {
    let argv = probe_argv(&["--exit-code=5"]);
    let withdrawn = runner()
        .run_withdrawable(&argv, &cwd(), &[], &|| true)
        .expect("withdrawable");
    assert_eq!(withdrawn, None);
    let ran = runner()
        .run_withdrawable(&argv, &cwd(), &[], &|| false)
        .expect("withdrawable");
    assert_eq!(ran.and_then(|status| status.code), Some(5));
}

#[test]
fn cancellation_reports_through_on_stop() {
    let argv = probe_argv(&["--exit-code=0"]);
    let status = cancelling().run(&argv, &cwd(), &[]).expect("run");
    assert_eq!(status, ChildStatus::signalled(None));
    assert_eq!(STOPPED.load(Ordering::SeqCst), 15);
    let withdrawable = cancelling()
        .run_withdrawable(&argv, &cwd(), &[], &|| false)
        .expect("withdrawable");
    assert_eq!(withdrawable, Some(ChildStatus::signalled(None)));
    let spec = stream_spec(&argv, &cwd(), &[], false, false);
    assert_eq!(
        cancelling().stream(spec).expect("stream"),
        Some(ChildStatus::signalled(None))
    );
}

#[test]
fn stream_spec_follows_flags() {
    let argv = probe_argv(&[]);
    let hermetic = stream_spec(&argv, &cwd(), &[("A", "b")], true, true);
    assert!(matches!(hermetic.env, EnvPolicy::Controlled { .. }));
    assert!(matches!(hermetic.stdout, StreamOutput::Inherit));
    assert!(matches!(hermetic.stdin, StreamStdin::Inherit));
    assert!(matches!(hermetic.stderr, StreamOutput::Inherit));
    let plain = stream_spec(&argv, &cwd(), &[], false, false);
    assert!(matches!(plain.env, EnvPolicy::Inherited { .. }));
    assert!(matches!(plain.stdout, StreamOutput::Forward(_)));
}

#[test]
fn stream_cleanup_budget_is_fixed() {
    assert_eq!(STREAM_CLEANUP.graceful, Duration::from_secs(10));
    assert_eq!(STREAM_CLEANUP.forced, Duration::from_secs(2));
    assert_eq!(STREAM_CLEANUP.drain, Duration::from_secs(5));
}

#[test]
fn free_disk_bytes_delegates() {
    assert!(runner().free_disk_bytes(&cwd()).is_some());
}

static VERSION_STOPPED: AtomicI32 = AtomicI32::new(0);

#[test]
fn a_stopped_version_probe_reports_through_on_stop() {
    let tool = PathBuf::from(&probe_argv(&[])[0]);
    let stopping = BinaryRunner {
        inherit_stdout: false,
        stop: || Some(15),
        on_stop: |signo| VERSION_STOPPED.store(signo, Ordering::SeqCst),
    };
    assert_eq!(stopping.tool_version(&tool), None);
    assert_eq!(VERSION_STOPPED.load(Ordering::SeqCst), 15);
}

#[test]
fn a_failing_version_probe_does_not_report_a_stop() {
    let tool = PathBuf::from("/nonexistent-dx-tool");
    assert_eq!(runner().tool_version(&tool), None);
}
