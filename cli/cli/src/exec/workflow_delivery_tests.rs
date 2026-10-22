use super::super::test_support::*;
use std::io::{self, Write};

#[derive(Clone, Copy)]
enum Fault {
    WriteFails(io::ErrorKind),
    FlushFails(io::ErrorKind),
    OneByteWrites,
}

struct Sink {
    fault: Fault,
    fail_from_write: usize,
    writes: usize,
    bytes: Vec<u8>,
}

impl Sink {
    fn new(fault: Fault, fail_from_write: usize) -> Self {
        Sink {
            fault,
            fail_from_write,
            writes: 0,
            bytes: Vec::new(),
        }
    }

    fn lines(&self) -> Vec<String> {
        String::from_utf8_lossy(&self.bytes)
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        match self.fault {
            Fault::WriteFails(kind) if self.writes >= self.fail_from_write => {
                Err(io::Error::from(kind))
            }
            Fault::OneByteWrites if !buf.is_empty() => {
                self.bytes.push(buf[0]);
                Ok(1)
            }
            _ => {
                self.bytes.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.fault {
            Fault::FlushFails(kind) => Err(io::Error::from(kind)),
            _ => Ok(()),
        }
    }
}

fn event_names(sink: &Sink) -> Vec<String> {
    sink.lines()
        .iter()
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("event line");
            value["event"].as_str().expect("event name").to_owned()
        })
        .collect()
}

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
        assert_eq!(event_names(&sink), ["command_started"]);
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
        assert_eq!(event_names(&sink), ["command_started", "command_finished"]);
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
        assert_eq!(event_names(&sink), ["command_started"]);
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
        event_names(&sink),
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
    assert_eq!(event_names(&sink), ["command_started", "command_finished"]);
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
    let text = String::from_utf8_lossy(&sink.bytes);
    assert!(!text.contains("Running build"), "{text}");
}

fn closed_pipe_schedules_no_work() {
    let (reader, mut writer) = io::pipe().expect("pipe");
    drop(reader);
    let harness = Harness::new("wf-closed-pipe");
    let inv = invocation(&["build", "--output=json"]);
    let run = harness.probe_streams(&inv, &[Some(0)], &mut writer, &mut Vec::new());
    assert_eq!(run.code, 141);
    assert!(run.argv.is_empty(), "{:?}", run.argv);
}

#[test]
#[cfg(unix)]
fn unix_closed_pipe_schedules_no_work() {
    closed_pipe_schedules_no_work();
}

#[test]
#[cfg(windows)]
fn windows_closed_pipe_schedules_no_work() {
    closed_pipe_schedules_no_work();
}
