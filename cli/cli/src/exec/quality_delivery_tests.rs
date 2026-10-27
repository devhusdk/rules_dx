use super::super::delivery_sink::{Fault, Sink};
use super::super::test_support::*;
use std::io;

fn harness(name: &str, fixable: bool) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    let replacements = if fixable {
        vec![harness.replacement(b"y")]
    } else {
        vec![]
    };
    let result = harness.valid_result(vec![Harness::diagnostic("unused", fixable)], replacements);
    harness.results.insert("//test:corpus".to_owned(), result);
    harness
}

fn clean(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    let result = harness.valid_result(vec![], vec![]);
    harness.results.insert("//test:corpus".to_owned(), result);
    harness
}

fn run(harness: &Harness, words: &[&str], sink: &mut Sink) -> (i32, String) {
    let inv = invocation(words);
    let runner = harness.runner();
    let mut err = Vec::new();
    let code = harness.run_streams(&inv, &runner, sink, &mut err);
    (code, String::from_utf8(err).expect("stderr"))
}

fn source(harness: &Harness) -> Vec<u8> {
    std::fs::read(harness.workspace.join("src/a.py")).expect("source")
}

fn launches(harness: &Harness) -> usize {
    harness.seen_env.borrow().len()
}

const KINDS: [(io::ErrorKind, i32); 2] = [
    (io::ErrorKind::BrokenPipe, 141),
    (io::ErrorKind::PermissionDenied, 1),
];

#[test]
fn a_failed_start_event_starts_no_child_and_no_mutation() {
    for command in ["lint", "format"] {
        for (kind, want) in KINDS {
            let h = harness("q-start-fails", true);
            let mut sink = Sink::new(Fault::WriteFails(kind), 1);
            let (code, _) = run(&h, &[command, "--apply", "--output=json"], &mut sink);
            assert_eq!(code, want, "{command} {kind:?}");
            assert_eq!(launches(&h), 0, "{command} {kind:?}");
            assert_eq!(source(&h), b"x = 1\n", "{command} {kind:?}");
            assert!(sink.bytes.is_empty());
        }
    }
}

#[test]
fn a_failed_text_summary_starts_no_child_and_no_mutation() {
    for (kind, want) in KINDS {
        let h = harness("q-summary-fails", true);
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let (code, _) = run(&h, &["format", "--apply", "--output=text"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(launches(&h), 0);
        assert_eq!(source(&h), b"x = 1\n");
    }
}

#[test]
fn a_failure_on_the_last_event_never_returns_success() {
    let h = clean("q-last-count");
    let mut full = Sink::new(Fault::None, 0);
    let (code, _) = run(&h, &["lint", "--output=json"], &mut full);
    assert_eq!(code, 0);
    let total = full.names().len();
    assert_eq!(
        full.names().last().map(String::as_str),
        Some("command_finished")
    );
    for (kind, want) in KINDS {
        let h = clean("q-last-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), total);
        let (code, _) = run(&h, &["lint", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(sink.names().len(), total - 1);
        assert!(!sink.text().contains("command_finished"));
    }
}

#[test]
fn a_flush_failure_never_returns_success() {
    for (kind, want) in KINDS {
        let h = clean("q-flush-fails");
        let mut sink = Sink::new(Fault::FlushFails(kind), 0);
        let (code, _) = run(&h, &["lint", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(
            sink.names().last().map(String::as_str),
            Some("command_finished")
        );
    }
}

#[test]
fn a_failed_finding_record_ends_the_stream_without_a_terminal_event() {
    for (kind, want) in KINDS {
        let h = harness("q-finding-fails", true);
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let (code, _) = run(&h, &["lint", "--apply", "--output=json"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert_eq!(sink.names(), ["command_started"]);
        assert_eq!(launches(&h), 1, "no second child");
    }
}

#[test]
fn a_failed_text_finding_never_returns_success() {
    for (kind, want) in KINDS {
        let h = harness("q-text-finding-fails", false);
        let mut sink = Sink::new(Fault::WriteFails(kind), 2);
        let (code, _) = run(&h, &["lint", "--check", "--output=text"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
    }
}

#[test]
fn a_failed_diff_never_returns_success() {
    for (kind, want) in KINDS {
        let h = harness("q-diff-fails", true);
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let (code, _) = run(&h, &["format", "--check", "--output=diff"], &mut sink);
        assert_eq!(code, want, "{kind:?}");
        assert!(sink.bytes.is_empty());
        assert_eq!(source(&h), b"x = 1\n");
    }
}

#[test]
fn partial_writes_deliver_every_event_whole() {
    let h = clean("q-partial");
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (code, _) = run(&h, &["lint", "--output=json"], &mut sink);
    assert_eq!(code, 0);
    assert_eq!(
        sink.names().first().map(String::as_str),
        Some("command_started")
    );
    assert_eq!(
        sink.names().last().map(String::as_str),
        Some("command_finished")
    );
}

#[test]
fn partial_writes_deliver_the_whole_diff() {
    let h = harness("q-partial-diff", true);
    let mut whole = Sink::new(Fault::None, 0);
    let (want, _) = run(&h, &["format", "--check", "--output=diff"], &mut whole);
    let h = harness("q-partial-diff-bytes", true);
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (code, _) = run(&h, &["format", "--check", "--output=diff"], &mut sink);
    assert_eq!(code, want);
    assert_eq!(sink.bytes, whole.bytes);
    assert!(!sink.bytes.is_empty());
}

#[test]
fn a_stdout_report_owns_stdout() {
    let h = harness("q-report-owner", false);
    let mut sink = Sink::new(Fault::OneByteWrites, 0);
    let (_, err) = run(
        &h,
        &["lint", "--check", "--output=text", "--report=sarif=-"],
        &mut sink,
    );
    let document: serde_json::Value = serde_json::from_slice(&sink.bytes).expect("only SARIF");
    assert!(document.get("runs").is_some());
    assert!(err.contains("unused"), "{err}");
}

#[test]
fn a_failed_stdout_report_never_returns_success() {
    for (kind, want) in KINDS {
        let h = clean("q-report-fails");
        let mut sink = Sink::new(Fault::WriteFails(kind), 1);
        let (code, _) = run(
            &h,
            &["lint", "--check", "--output=text", "--report=sarif=-"],
            &mut sink,
        );
        assert_eq!(code, want, "{kind:?}");
    }
}

#[test]
fn a_report_file_still_lands_beside_clean_output() {
    let h = harness("q-report-file", false);
    let mut sink = Sink::new(Fault::None, 0);
    let (code, _) = run(
        &h,
        &[
            "lint",
            "--check",
            "--output=json",
            "--report=sarif=out.sarif",
        ],
        &mut sink,
    );
    assert_eq!(code, 1);
    assert!(h.workspace.join("out.sarif").exists());
    assert!(sink.names().contains(&"report".to_owned()));
}

#[test]
#[cfg(any(unix, windows))]
fn closed_pipe_starts_nothing() {
    let mut writer = closed_pipe();
    let h = harness("q-closed-pipe", true);
    let inv = invocation(&["format", "--apply", "--output=json"]);
    let runner = h.runner();
    let code = h.run_streams(&inv, &runner, &mut writer, &mut Vec::new());
    assert_eq!(code, 141);
    assert_eq!(launches(&h), 0);
    assert_eq!(source(&h), b"x = 1\n");
}
