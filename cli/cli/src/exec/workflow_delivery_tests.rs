use super::super::delivery_sink::{Fault, Sink};
use super::super::test_support::*;
use std::io;

fn run(
    name: &str,
    words: &[&str],
    codes: &[Option<i32>],
    sink: &mut Sink,
) -> (super::super::test_support::ProbeRun, String) {
    let harness = Harness::new(name);
    let inv = invocation(words);
    let mut err = Vec::new();
    let run = harness.probe_streams(&inv, codes, sink, &mut err);
    (run, String::from_utf8(err).expect("stderr"))
}

#[test]
fn a_failed_start_event_schedules_no_work() {
    for (kind, want) in [
        (io::ErrorKind::BrokenPipe, 141),
        (io::ErrorKind::PermissionDenied, 1),
    ] {
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let (run, _) = run(
            "wf-start-fails",
            &["build", "--output=json"],
            &[Some(0)],
            &mut sink,
        );
        assert_eq!(run.code, want, "{kind:?}");
        assert!(run.argv.is_empty(), "no child starts: {:?}", run.argv);
        assert!(sink.bytes.is_empty());
    }
}

#[test]
fn a_failed_text_summary_schedules_no_work() {
    let mut sink = Sink::new(Fault::WriteFails(io::ErrorKind::BrokenPipe), 1);
    let (run, _) = run(
        "wf-summary-fails",
        &["build", "--output=text"],
        &[Some(0)],
        &mut sink,
    );
    assert_eq!(run.code, 141);
    assert!(run.argv.is_empty(), "{:?}", run.argv);
}

#[test]
fn a_failed_final_event_never_returns_success() {
    for (kind, want) in [
        (io::ErrorKind::BrokenPipe, 141),
        (io::ErrorKind::StorageFull, 1),
    ] {
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let (run, _) = run(
            "wf-final-fails",
            &["build", "--output=json"],
            &[Some(0)],
            &mut sink,
        );
        assert_eq!(run.code, want, "{kind:?}");
        assert_eq!(run.argv.len(), 1, "the build ran once");
        assert_eq!(sink.names(), ["command_started"]);
    }
}

#[test]
fn a_flush_failure_after_the_final_event_never_returns_success() {
    for (kind, want) in [
        (io::ErrorKind::BrokenPipe, 141),
        (io::ErrorKind::PermissionDenied, 1),
    ] {
        let mut sink = Sink::new(Fault::FlushFails(kind), 0);
        let (run, _) = run(
            "wf-flush-fails",
            &["build", "--output=json"],
            &[Some(0)],
            &mut sink,
        );
        assert_eq!(run.code, want, "{kind:?}");
        assert_eq!(sink.names(), ["command_started", "command_finished"]);
    }
}

#[test]
fn a_child_failure_keeps_its_status_when_delivery_also_fails() {
    for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let (run, err) = run(
            "wf-child-failed",
            &["build", "--output=json"],
            &[Some(3)],
            &mut sink,
        );
        assert_eq!(run.code, 3, "{kind:?}");
        assert!(err.contains("keeping exit 3"), "{err}");
        assert_eq!(sink.names(), ["command_started"]);
    }
}

#[test]
fn a_child_failure_without_a_delivery_failure_keeps_its_status() {
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (run, _) = run(
        "wf-child-ok-pipe",
        &["build", "--output=json"],
        &[Some(3)],
        &mut sink,
    );
    assert_eq!(run.code, 3);
    assert_eq!(
        sink.names(),
        ["command_started", "notice", "error", "command_finished"]
    );
}

#[test]
fn partial_writes_deliver_every_event_whole() {
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (run, _) = run(
        "wf-partial",
        &["build", "--output=json"],
        &[Some(0)],
        &mut sink,
    );
    assert_eq!(run.code, 0);
    assert_eq!(sink.names(), ["command_started", "command_finished"]);
}

#[test]
fn a_stdout_report_owns_stdout_so_no_summary_is_written() {
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (run, _) = run(
        "wf-report-owner",
        &["build", "--output=text", "--report=failures=-"],
        &[Some(0)],
        &mut sink,
    );
    assert_eq!(run.code, 0);
    let text = sink.text();
    assert!(!text.contains("Running build"), "{text}");
}

#[test]
#[cfg(any(unix, windows))]
fn closed_pipe_schedules_no_work() {
    let (reader, mut writer) = io::pipe().expect("pipe");
    drop(reader);
    let harness = Harness::new("wf-closed-pipe");
    let inv = invocation(&["build", "--output=json"]);
    let run = harness.probe_streams(&inv, &[Some(0)], &mut writer, &mut Vec::new());
    assert_eq!(run.code, 141);
    assert!(run.argv.is_empty(), "{:?}", run.argv);
}
