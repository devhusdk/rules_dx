use super::super::test_support::*;
use std::io::{self, Write};

fn finding(harness: &mut Harness, labels: &[&str], start: u64, end: u64) {
    let mut diagnostic = Harness::diagnostic("unused", false);
    diagnostic.start_byte = Some(start);
    diagnostic.end_byte = Some(end);
    for label in labels {
        let result = harness.valid_result(vec![diagnostic.clone()], vec![]);
        harness.results.insert((*label).to_owned(), result);
    }
}

#[test]
fn json_finding_names_lines_count_and_origin() {
    let mut harness = Harness::new("json-finding-lines");
    harness.write_source("src/a.py", "x = 1\nyy = 2\n");
    finding(&mut harness, &["//test:corpus"], 6, 8);
    let (code, out, err) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let diagnostic = event(&events, "diagnostic");
    assert_eq!(diagnostic["location"], "lines");
    assert_eq!(diagnostic["count"], 1);
    assert!(diagnostic.get("baseline").is_none());
    let range = &diagnostic["range"];
    assert_eq!(range["start_byte"], 6);
    assert_eq!(range["end_byte"], 8);
    assert_eq!(range["start_line"], 2);
    assert_eq!(range["start_column"], 1);
    assert_eq!(range["end_line"], 2);
    assert_eq!(range["end_column"], 3);
    let origins = diagnostic["origins"].as_array().expect("origins");
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0]["producer"], "//test:corpus");
    assert_eq!(origins[0]["target"], "//test:corpus");
    assert!(origins[0]["configuration"].is_string(), "{diagnostic}");
}

#[test]
fn json_finding_from_an_unreadable_file_keeps_byte_location() {
    let mut harness = Harness::new("json-finding-bytes");
    harness.write_source("src/a.py", "x = 1\n");
    finding(&mut harness, &["//test:corpus"], 0, 1);
    harness.write_source("src/a.py", "changed = true\n");
    let (code, out, err) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let diagnostic = event(&events, "diagnostic");
    assert_eq!(diagnostic["location"], "bytes");
    assert!(
        diagnostic["range"].get("start_line").is_none(),
        "{diagnostic}"
    );
}

#[test]
fn json_identical_findings_from_two_targets_print_once() {
    let mut harness = Harness::new("json-finding-fold");
    harness.write_source("src/a.py", "x = 1\n");
    finding(&mut harness, &["//test:one", "//test:two"], 0, 1);
    let (code, out, err) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let diagnostics = events_of_kind(&events, "diagnostic");
    assert_eq!(diagnostics.len(), 1, "{out}");
    assert_eq!(diagnostics[0]["count"], 2);
    let targets: Vec<&str> = diagnostics[0]["origins"]
        .as_array()
        .expect("origins")
        .iter()
        .map(|origin| origin["target"].as_str().expect("target"))
        .collect();
    assert_eq!(targets, vec!["//test:one", "//test:two"]);
    assert_eq!(
        event(&events, "command_finished")["diagnostics"]["warning"],
        2
    );
}

#[test]
fn json_findings_at_different_places_stay_apart() {
    let mut harness = Harness::new("json-finding-apart");
    harness.write_source("src/a.py", "x = 1\n");
    finding(&mut harness, &["//test:one"], 0, 1);
    finding(&mut harness, &["//test:two"], 2, 3);
    let (_, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    let events = json_events(&out);
    let diagnostics = events_of_kind(&events, "diagnostic");
    assert_eq!(diagnostics.len(), 2, "{out}");
    for diagnostic in diagnostics {
        assert_eq!(diagnostic["count"], 1);
    }
}

#[test]
fn json_stream_stops_at_the_first_broken_write() {
    struct BreaksAfter {
        lines: Vec<u8>,
        writes: usize,
    }
    impl Write for BreaksAfter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.lines.contains(&b'\n') {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"));
            }
            self.lines.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut harness = Harness::new("json-broken-pipe");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    let inv = dx_plan::args::parse(&dx_testing::cli::strings(&[
        "lint",
        "--check",
        "--output=json",
    ]))
    .expect("parse");
    let runner = harness.runner();
    let mut out = BreaksAfter {
        lines: Vec::new(),
        writes: 0,
    };
    let mut err = Vec::new();
    assert_eq!(harness.run_streams(&inv, &runner, &mut out, &mut err), 141);
    let text = String::from_utf8(out.lines).expect("utf8");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("command_started"), "{text}");
    assert!(!text.contains("command_finished"), "{text}");
    assert!(out.writes >= 2, "the second event hit the closed pipe");
}
