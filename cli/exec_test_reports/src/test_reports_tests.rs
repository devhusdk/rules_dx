use super::stable_exec_path;
use std::path::Path;

#[test]
fn log_tail_keeps_last_forty_lines() {
    let text: String = (1..=50)
        .map(|n| format!("line {n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let want: Vec<String> = (11..=50).map(|n| format!("line {n}")).collect();
    assert_eq!(
        super::account::log_tail(text.as_bytes()),
        Some(want.join("\n"))
    );
}

#[test]
fn log_tail_returns_short_logs_whole() {
    assert_eq!(
        super::account::log_tail(b"a\nb\nc\n"),
        Some("a\nb\nc".to_owned())
    );
}

#[test]
fn log_tail_drops_partial_first_line_when_capped() {
    let lines: Vec<String> = (0..40)
        .map(|n| format!("{n:02}{}", "x".repeat(298)))
        .collect();
    let tail = super::account::log_tail(lines.join("\n").as_bytes()).unwrap();
    assert!(tail.len() <= super::account::LOG_TAIL_BYTES);
    let got: Vec<&str> = tail.split('\n').collect();
    assert!(got.iter().all(|l| l.len() == 300));
    assert_eq!(
        got,
        lines[40 - got.len()..]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(!tail.starts_with('\n'));
}

#[test]
fn log_tail_cuts_one_oversize_multibyte_line_at_char_boundary() {
    let tail = super::account::log_tail("日".repeat(3500).as_bytes()).unwrap();
    assert!(!tail.is_empty() && tail.len() <= super::account::LOG_TAIL_BYTES);
}

#[test]
fn log_tail_is_none_for_blank_input() {
    assert_eq!(super::account::log_tail(b""), None);
    assert_eq!(super::account::log_tail(b"  \n\t\n\n"), None);
}

#[test]
fn log_tail_tolerates_invalid_utf8() {
    assert!(super::account::log_tail(b"ok\n\xff\xfe bad\n").is_some());
}

#[test]
fn coverage_sources_returns_the_write_code_when_stdout_fails() {
    let account = dx_lcov::CellAccount::default();
    let got = super::coverage_sources(
        &super::Sources::default(),
        &account,
        dx_output::OutputMode::Json,
        false,
        &mut BrokenPipe,
        &mut Vec::new(),
    );
    assert_eq!(got, Err(dx_process::broken_pipe_code()));
}

#[test]
fn an_exec_path_reads_the_same_on_every_host() {
    let execroot = "/home/someone/.cache/bazel/_bazel_x/abc123/execroot/_main";
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/k8-fastbuild/bin/env/shard/doctor_test/test.xml"
        ))),
        "env/shard/doctor_test/test.xml"
    );
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/x64_windows-fastbuild/bin/env/shard/doctor_test/test.xml"
        ))),
        "env/shard/doctor_test/test.xml",
        "the configuration does not reach the report"
    );
    assert_eq!(
        stable_exec_path(Path::new(&format!("{execroot}/bin/env/shard/test.xml"))),
        "env/shard/test.xml"
    );
}

#[test]
fn a_windows_exec_path_keeps_its_forward_slashes() {
    assert_eq!(
        stable_exec_path(Path::new(
            r"C:/bz/out/execroot/_main/bazel-out/x64_windows-fastbuild/bin/env/shard/test.xml"
        )),
        "env/shard/test.xml"
    );
}

#[test]
fn a_path_outside_the_build_directory_keeps_its_file_name() {
    assert_eq!(
        stable_exec_path(Path::new("/tmp/somewhere/test.xml")),
        "test.xml"
    );
    assert_eq!(stable_exec_path(Path::new("test.xml")), "test.xml");
}

const MINIMAL_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;

#[test]
fn a_testlogs_path_keeps_its_package_and_identity() {
    let execroot = "/home/someone/.cache/bazel/_bazel_x/abc123/execroot/_main";
    assert_eq!(
        stable_exec_path(Path::new(&format!(
            "{execroot}/bazel-out/k8-fastbuild/testlogs/cli/bep/dx_bep_test/run_2_of_2/test.xml"
        ))),
        "cli/bep/dx_bep_test/run_2_of_2/test.xml"
    );
    assert_eq!(
        stable_exec_path(Path::new("/ws/bazel-testlogs/a/t/shard_1_of_2/test.xml")),
        "a/t/shard_1_of_2/test.xml"
    );
}

fn evidence(problems: &[&str]) -> super::Evidence {
    let mut evidence = super::Evidence::default();
    for problem in problems {
        evidence.record((*problem).to_owned());
    }
    evidence
}

#[test]
fn evidence_gap_names_what_made_results_incomplete() {
    assert_eq!(
        super::evidence_gap(true, &evidence(&[]), "none reported"),
        Some("none reported".to_owned())
    );
    assert_eq!(
        super::evidence_gap(true, &evidence(&["a", "b"]), "none reported"),
        Some("a".to_owned())
    );
    assert_eq!(
        super::evidence_gap(false, &evidence(&["a", "b"]), "none reported"),
        Some("2 missing or invalid results: a".to_owned())
    );
    assert_eq!(
        super::evidence_gap(false, &evidence(&[]), "none reported"),
        None
    );
}

fn located(xml: &str) -> Vec<(Option<String>, Option<u64>)> {
    let cases = dx_plan::reports::parse_test_xml(xml.as_bytes(), 0, 0).expect("cases");
    cases
        .iter()
        .map(|case| {
            let found = super::account::test_case(case);
            (found.file, found.line)
        })
        .collect()
}

#[test]
fn a_case_takes_file_and_line_from_the_report_attributes() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a" file="t/a_test.py" line="7"><failure message="x:9: no">trace</failure></testcase></testsuite></testsuites>"#;
    assert_eq!(located(xml), [(Some("t/a_test.py".to_owned()), Some(7))]);
}

#[test]
fn a_case_takes_file_and_line_from_the_failure_message() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a"><failure message="foo_test.go:42: got 1">t</failure></testcase><testcase name="b"><error>at Foo.method(Foo.java:17)</error></testcase></testsuite></testsuites>"#;
    assert_eq!(
        located(xml),
        [
            (Some("foo_test.go".to_owned()), Some(42)),
            (Some("Foo.java".to_owned()), Some(17)),
        ]
    );
}

#[test]
fn a_case_without_a_location_leaves_file_and_line_unset() {
    let xml = r#"<testsuites><testsuite name="s"><testcase name="a"><failure message="took 12:30:45 at /abs/x.rs:3">t</failure></testcase><testcase name="b"/></testsuite></testsuites>"#;
    assert_eq!(located(xml), [(None, None), (None, None)]);
}

#[test]
fn parse_cases_stamps_run_and_configuration() {
    let result = dx_bep::TestResultRecord {
        label: "//a:t".to_owned(),
        configuration: Some("cfg".to_owned()),
        run: 2,
        shard: 3,
        attempt: 1,
        status: Some("PASSED".to_owned()),
        cached_locally: None,
        duration_millis: None,
        strategy: None,
        files: Vec::new(),
    };
    let cases = super::parse_cases(&result, MINIMAL_TEST_XML.as_bytes()).expect("cases");
    assert!(!cases.is_empty(), "expected non-empty");
    for case in &cases {
        assert_eq!(case.run, 1);
        assert_eq!(case.configuration.as_deref(), Some("cfg"));
    }
    assert!(super::parse_cases(&result, b"no elements").is_err());
}

#[test]
fn coverage_summary_prints_the_total_and_the_worst_files() {
    let mut account = dx_lcov::CellAccount {
        eligible_lines: 20,
        covered_lines: 13,
        ..dx_lcov::CellAccount::default()
    };
    for (path, counts) in [("a.rs", (10, 10)), ("b.rs", (3, 6)), ("c.rs", (0, 4))] {
        account.file_lines.insert(path.to_owned(), counts);
    }
    let text = dx_output::OutputMode::Text { quiet: false };
    let mut err = Vec::new();
    super::coverage_summary(&account, true, text, &mut err);
    let got = String::from_utf8(err).unwrap();
    assert!(got.contains("dx: coverage 65.0% (13/20 lines)"), "{got}");
    assert!(!got.contains("a.rs"), "{got}");
    assert!(
        got.find("c.rs").unwrap() < got.find("b.rs").unwrap(),
        "{got}"
    );
    let mut err = Vec::new();
    super::coverage_summary(&account, false, text, &mut err);
    assert!(!String::from_utf8(err).unwrap().contains("dx: coverage 65"));
    let mut err = Vec::new();
    super::coverage_summary(&account, true, dx_output::OutputMode::Json, &mut err);
    assert!(err.is_empty(), "{err:?}");
}

#[test]
fn coverage_threshold_passes_without_minimum_and_fails_on_incomplete_evidence() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let sources = super::Sources::default();
    let text = dx_output::OutputMode::Text { quiet: false };
    assert_eq!(
        super::coverage_threshold(None, None, &sources, text, &mut out, &mut err),
        Ok(true)
    );
    assert_eq!(
        super::coverage_threshold(Some(80), None, &sources, text, &mut out, &mut err),
        Ok(false)
    );
    assert!(out.is_empty() && err.is_empty());
}

struct BrokenPipe;

impl std::io::Write for BrokenPipe {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "broken pipe",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "broken pipe",
        ))
    }
}

#[test]
fn announce_incomplete_returns_the_stdout_code_when_stdout_breaks() {
    let mut err = Vec::new();
    assert_eq!(
        super::announce_incomplete(
            dx_output::OutputMode::Json,
            &mut BrokenPipe,
            &mut err,
            "no test.xml"
        ),
        Err(dx_process::broken_pipe_code())
    );
}

#[test]
fn coverage_threshold_returns_the_stdout_code_when_stdout_breaks() {
    let mut err = Vec::new();
    let account = dx_lcov::CellAccount::default();
    assert_eq!(
        super::coverage_threshold(
            Some(80),
            Some(&account),
            &super::Sources::default(),
            dx_output::OutputMode::Json,
            &mut BrokenPipe,
            &mut err,
        ),
        Err(dx_process::broken_pipe_code())
    );
}

#[test]
fn a_text_report_announcement_returns_the_stdout_code_when_stdout_breaks() {
    let dir = dx_test_scratch::scratch("dx-exec-test-report-sink-broken-").expect("scratch");
    let mut err = Vec::new();
    let mut sink = super::ReportSink {
        out: &mut BrokenPipe,
        err: &mut err,
        output: dx_output::OutputMode::Text { quiet: false },
        workspace: dir.path(),
        stdout_report: false,
    };
    let got = sink.write(
        "junit",
        &dx_plan::reports::Destination::File("junit.xml".to_owned()),
        "<testsuites/>",
        true,
    );
    assert_eq!(got, Err(dx_process::broken_pipe_code()));
    assert!(dir.path().join("junit.xml").is_file());
}

#[test]
fn results_document_fails_on_an_outcome_without_a_target() {
    let outcome = dx_output::TestOutcome {
        outcome: "passed".to_owned(),
        ..dx_output::TestOutcome::default()
    };
    assert!(super::results_document(&[outcome], true, 0, "run").is_err());
}

#[test]
fn an_outcome_that_fails_to_build_ends_the_stream_once() {
    let outcome = dx_output::TestOutcome {
        outcome: "failed".to_owned(),
        ..dx_output::TestOutcome::default()
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let ended = super::emit_outcomes(
        dx_output::OutputMode::Json,
        &mut out,
        &mut err,
        &[outcome],
        &[],
    );
    assert_eq!(ended.err(), Some(dx_process::operational_code()));
    let events: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["error", "command_finished"], "{events:?}");
    assert_eq!(events[0]["code"], "invalid_result");
}
