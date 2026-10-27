use super::super::delivery_sink::{Fault, Sink};
use super::super::test_support::*;
use std::io;

const SOURCE: &str = "rust/tests/fixtures/hello/BUILD.bazel";

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

fn source(harness: &Harness) -> Vec<u8> {
    std::fs::read(harness.workspace.join(SOURCE)).expect("source")
}

#[test]
fn a_failed_start_event_starts_no_child() {
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-start-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(launches(&h), 0, "{kind:?}");
        assert_eq!(source(&h), b"xyz\n");
        assert!(sink.bytes.is_empty());
    }
}

#[test]
fn a_failed_text_summary_starts_no_child() {
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-summary-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let code = run(&h, &["generate", "--apply", "--output=text"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(launches(&h), 0, "{kind:?}");
    }
}

#[test]
fn a_failed_change_record_ends_the_stream_without_a_terminal_event() {
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-change-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(sink.names(), ["command_started"]);
        assert_eq!(launches(&h), 1, "no second child");
    }
}

#[test]
fn a_failure_on_the_last_event_never_returns_success() {
    let h = generate_witness("xyz\n", "g-last-count");
    let mut full = Sink::new(Fault::None, 0);
    assert_eq!(
        run(&h, &["generate", "--apply", "--output=json"], &mut full),
        0
    );
    let total = full.names().len();
    assert_eq!(
        full.names().last().map(String::as_str),
        Some("command_finished")
    );
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-last-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), total);
        let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(sink.names().len(), total - 1);
        assert!(!sink.text().contains("command_finished"));
    }
}

#[test]
fn a_flush_failure_never_returns_success() {
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-flush-fails");
        let mut sink = Sink::new(Fault::FlushFails(kind), 0);
        let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(
            sink.names().last().map(String::as_str),
            Some("command_finished")
        );
    }
}

#[test]
fn a_failed_text_line_never_returns_success() {
    for (kind, want) in KINDS {
        let h = generate_witness("xyz\n", "g-text-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 3);
        let code = run(&h, &["generate", "--apply", "--output=text"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
    }
}

#[test]
fn a_failed_diff_never_returns_success() {
    for (kind, want) in KINDS {
        let h = generate_witness("abc\n", "g-diff-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let code = run(&h, &["generate", "--apply", "--output=diff"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert!(sink.bytes.is_empty());
    }
}

#[test]
fn partial_writes_deliver_every_event_whole() {
    let h = generate_witness("xyz\n", "g-partial");
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
    assert_eq!(code, 0);
    assert_eq!(
        sink.names(),
        ["command_started", "change", "command_finished"]
    );
}

#[test]
fn partial_writes_deliver_the_whole_diff() {
    let h = generate_witness("abc\n", "g-partial-diff");
    let mut whole = Sink::new(Fault::None, 0);
    let want = run(&h, &["generate", "--apply", "--output=diff"], &mut whole);
    let h = generate_witness("abc\n", "g-partial-diff-bytes");
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let code = run(&h, &["generate", "--apply", "--output=diff"], &mut sink);
    assert_eq!(code, want);
    assert_eq!(sink.bytes, whole.bytes);
    assert!(!sink.bytes.is_empty());
}

#[test]
fn a_child_failure_keeps_its_status_when_delivery_succeeds() {
    let mut h = generate_witness("xyz\n", "g-child-fails");
    h.bazel_code = 2;
    let mut sink = Sink::new(Fault::None, 0);
    let code = run(&h, &["generate", "--apply", "--output=json"], &mut sink);
    assert_eq!(code, 2);
    assert_eq!(
        sink.names().last().map(String::as_str),
        Some("command_finished")
    );
}

#[test]
fn a_child_failure_keeps_its_status_when_delivery_also_fails() {
    for (kind, _) in KINDS {
        let mut h = generate_witness("xyz\n", "g-child-and-delivery");
        h.bazel_code = 2;
        let mut sink = Sink::new(Fault::WriteFails(kind), 3);
        let mut err = Vec::new();
        let inv = invocation(&["generate", "--apply", "--output=json"]);
        let runner = h.runner();
        let code = h.run_streams(&inv, &runner, &mut sink, &mut err);
        assert_eq!(code, 2, "{kind:?}");
        assert!(String::from_utf8_lossy(&err).contains("keeping exit 2"));
    }
}

#[test]
#[cfg(any(unix, windows))]
fn closed_pipe_starts_nothing() {
    let mut writer = closed_pipe();
    let h = generate_witness("xyz\n", "g-closed-pipe");
    let inv = invocation(&["generate", "--apply", "--output=json"]);
    let runner = h.runner();
    let code = h.run_streams(&inv, &runner, &mut writer, &mut Vec::new());
    assert_eq!(code, 141);
    assert_eq!(launches(&h), 0);
    assert_eq!(source(&h), b"xyz\n");
}
