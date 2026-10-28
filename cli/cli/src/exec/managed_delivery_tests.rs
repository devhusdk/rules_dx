use super::super::delivery_sink::{Fault, Sink};
use super::super::test_support::*;
use dx_setup::read_current_pair;
use std::io;

const KINDS: [(io::ErrorKind, i32); 2] = [
    (io::ErrorKind::BrokenPipe, 141),
    (io::ErrorKind::PermissionDenied, 1),
];

fn run(harness: &Harness, words: &[&str], sink: &mut Sink) -> i32 {
    let inv = invocation(words);
    let runner = harness.runner();
    harness.run_streams(&inv, &runner, sink, &mut Vec::new())
}

fn launches(harness: &Harness) -> usize {
    harness.seen_env.borrow().len()
}

fn selected(harness: &Harness) -> bool {
    read_current_pair(&harness.workspace)
        .expect("read current")
        .is_some()
}

#[test]
fn a_failed_start_event_starts_no_child_and_no_selection() {
    for command in ["codegen", "env"] {
        for (kind, want) in KINDS {
            let h = Harness::new("m-start-fails");
            let mut sink = Sink::new(Fault::WriteFails(kind), 1);
            let code = run(&h, &[command, "--apply", "--output=json"], &mut sink);
            assert_eq!(code, want, "{command} {kind:?}");
            assert_eq!(launches(&h), 0, "{command} {kind:?}");
            assert!(!selected(&h));
            assert!(sink.bytes.is_empty());
        }
    }
}

#[test]
fn a_failed_operation_event_starts_no_child() {
    for (kind, want) in KINDS {
        let h = Harness::new("m-operation-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let code = run(&h, &["env", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(sink.names(), ["command_started"]);
        assert_eq!(launches(&h), 0);
        assert!(!selected(&h));
    }
}

#[test]
fn a_failed_text_summary_starts_no_child() {
    for (kind, want) in KINDS {
        let h = Harness::new("m-summary-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let code = run(&h, &["codegen", "--apply", "--output=text"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(launches(&h), 0);
        assert!(!selected(&h));
    }
}

#[test]
fn a_failure_on_the_last_event_keeps_the_applied_selection() {
    let h = Harness::new("m-last-count");
    let mut full = Sink::new(Fault::None, 0);
    assert_eq!(run(&h, &["env", "--apply", "--output=json"], &mut full), 0);
    let total = full.names().len();
    assert_eq!(
        full.names().last().map(String::as_str),
        Some("command_finished")
    );
    for (kind, want) in KINDS {
        let h = Harness::new("m-last-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), total);
        let code = run(&h, &["env", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert!(!sink.text().contains("command_finished"));
        assert!(selected(&h), "applied edits are not rolled back");
    }
}

#[test]
fn a_failed_selection_line_never_returns_success() {
    for (kind, want) in KINDS {
        let mut lost_the_line = false;
        for from in 1..40 {
            let h = Harness::new("m-text-fails");
            let mut sink = Sink::new(Fault::WriteFails(kind), from);
            let code = run(&h, &["codegen", "--apply", "--output=text"], &mut sink);
            assert!(code == 0 || code == want, "{kind:?} from {from}: {code}");
            if code == want && selected(&h) && !sink.text().contains("selected setup") {
                lost_the_line = true;
            }
        }
        assert!(lost_the_line, "{kind:?}");
    }
}

#[test]
fn a_failed_drift_report_never_returns_success() {
    for (kind, want) in KINDS {
        let h = Harness::new("m-drift-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 3);
        let code = run(&h, &["env", "--check", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert!(!sink.text().contains("command_finished"));
    }
}

#[test]
fn a_flush_failure_never_returns_success() {
    for (kind, want) in KINDS {
        let h = Harness::new("m-flush-fails");
        let mut sink = Sink::new(Fault::FlushFails(kind), 0);
        let code = run(&h, &["env", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
    }
}

#[test]
fn partial_writes_deliver_every_event_whole() {
    let h = Harness::new("m-partial");
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let code = run(&h, &["env", "--apply", "--output=json"], &mut sink);
    assert_eq!(code, 0);
    let names = sink.names();
    assert_eq!(names.first().map(String::as_str), Some("command_started"));
    assert_eq!(names.last().map(String::as_str), Some("command_finished"));
}

#[test]
fn a_child_failure_keeps_its_status_when_delivery_succeeds() {
    let mut h = Harness::new("m-child-fails");
    h.bazel_code = 3;
    let mut sink = Sink::new(Fault::None, 0);
    let code = run(&h, &["env", "--apply", "--output=json"], &mut sink);
    assert_eq!(code, 3);
    assert!(!selected(&h));
}

#[test]
#[cfg(any(unix, windows))]
fn closed_pipe_starts_nothing() {
    let mut writer = closed_pipe();
    let h = Harness::new("m-closed-pipe");
    let inv = invocation(&["env", "--apply", "--output=json"]);
    let runner = h.runner();
    let code = h.run_streams(&inv, &runner, &mut writer, &mut Vec::new());
    assert_eq!(code, 141);
    assert_eq!(launches(&h), 0);
    assert!(!selected(&h));
}
