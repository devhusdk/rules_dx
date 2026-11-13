use super::*;

fn inputs(invocation_id: &str) -> Inputs {
    Inputs {
        platform: "linux_x86_64".to_owned(),
        invocation_id: invocation_id.to_owned(),
        run: Run {
            sha: "abc123".to_owned(),
            server_url: "https://github.example".to_owned(),
            repository: "devhusdk/rules_dx".to_owned(),
            run_id: "1".to_owned(),
        },
        dx_rc: Some("0\n".to_owned()),
        events: String::new(),
        changed_lines_configured: false,
        changed_rc: None,
        changed_lines: String::new(),
        profiles: Vec::new(),
    }
}

const CASES: [(&str, &str); 4] = [
    ("", "<!-- dx-coverage-summary: coverage linux_x86_64 -->"),
    ("b", "<!-- dx-coverage-summary: coverage linux_x86_64 b -->"),
    (
        "bb",
        "<!-- dx-coverage-summary: coverage linux_x86_64 bb -->",
    ),
    ("c", "<!-- dx-coverage-summary: coverage linux_x86_64 c -->"),
];

#[test]
fn each_invocation_has_its_own_coverage_marker() {
    let bodies: Vec<String> = CASES
        .iter()
        .map(|(invocation, _)| render(&inputs(invocation)))
        .collect();
    for (index, &(invocation, marker)) in CASES.iter().enumerate() {
        assert_eq!(
            bodies[index].lines().next(),
            Some(marker),
            "marker for {invocation:?}"
        );
        for (other, body) in bodies.iter().enumerate() {
            assert_eq!(
                body.contains(marker),
                other == index,
                "{invocation:?} marker found in comment {other}"
            );
        }
    }
}

#[test]
fn a_clean_run_renders_the_fixed_body() {
    assert_eq!(
        render(&inputs("")),
        "<!-- dx-coverage-summary: coverage linux_x86_64 -->\n\
## Coverage summary — coverage linux_x86_64 — PASS\n\n\
Analyzed revision: `abc123` | Run: https://github.example/devhusdk/rules_dx/actions/runs/1 | Cell: `coverage linux_x86_64`.\n\n\
Gate exit: 0 (comment never changes this outcome).\n\n\
Changed-line gate exit: not configured\n\n\
LCOV: Bazel-owned combined report for this cell (see workflow run 1).\n\n\
Scope: per-cell exact counts only; no cross-cell union, no averaged percentages, no rounding up. Codecov stays opt-in only, never required.\n\n\
Findings outside the diff stay in counts only; no invented review locations.\n"
    );
}

#[test]
fn a_missing_coverage_exit_fails_as_unknown() {
    let mut cell = inputs("");
    cell.dx_rc = None;
    let body = render(&cell);
    assert!(body.contains("— FAIL\n"), "{body}");
    assert!(body.contains("Gate exit: unknown "), "{body}");
}

#[test]
fn the_below_minimum_error_wins_over_a_pass_event() {
    let mut cell = inputs("");
    cell.dx_rc = Some("1".to_owned());
    cell.events = [
        r#"{"event":"coverage","result":"pass","minimum":80,"covered_lines":9,"eligible_lines":10,"cell":"x"}"#,
        "not json",
        r#"{"event":"error","code":"coverage_below_minimum","message":"coverage 50% below 80%"}"#,
    ]
    .join("\n");
    let body = render(&cell);
    assert!(body.contains("\n\ncoverage 50% below 80%\n\n"), "{body}");
    assert!(!body.contains("meets minimum"), "{body}");
}

#[test]
fn a_pass_with_a_minimum_names_the_counts() {
    let mut cell = inputs("");
    cell.events = [
        r#"{"event":"coverage","result":"pass","minimum":null,"covered_lines":1,"eligible_lines":2,"cell":"y"}"#,
        r#"{"event":"coverage","result":"pass","minimum":80,"covered_lines":9,"eligible_lines":10,"cell":"x"}"#,
    ]
    .join("\n");
    let body = render(&cell);
    assert!(
        body.contains("\n\ncoverage 9/10 lines meets minimum 80% in cell x\n\n"),
        "{body}"
    );
}

#[test]
fn source_events_are_fenced_and_capped() {
    let mut cell = inputs("");
    let mut lines = vec![r#"{"code":"coverage_sources_incomplete","message":"m"}"#.to_owned()];
    lines.extend(
        (0..MAX_SOURCE_LINES).map(|n| format!(r#"{{"code":"coverage_sources","message":"s{n}"}}"#)),
    );
    cell.events = lines.join("\n");
    let body = render(&cell);
    assert!(
        body.contains("```text\ncoverage_sources_incomplete: m\ncoverage_sources: s0\n"),
        "{body}"
    );
    assert!(body.contains(&format!("coverage_sources: s{}\n", MAX_SOURCE_LINES - 2)));
    assert!(!body.contains(&format!("coverage_sources: s{}\n", MAX_SOURCE_LINES - 1)));
}

#[test]
fn profiles_with_cached_actions_render_one_line_each() {
    let mut cell = inputs("");
    cell.profiles = vec![
        (
            "build".to_owned(),
            r#"{"cache":{"actions":3,"hit_percent":66.7,"remote_hits":1,"disk_hits":1},"wall_micros":125500000,"critical_path_micros":5000000}"#.to_owned(),
        ),
        (
            "test".to_owned(),
            r#"{"cache":{"actions":0,"hit_percent":0,"remote_hits":0,"disk_hits":0},"wall_micros":1,"critical_path_micros":1}"#.to_owned(),
        ),
    ];
    let body = render(&cell);
    assert!(
        body.contains(
            "```text\nbuild: 66.7% cache hits (remote 33%, disk 33%), 2m05s, critical path 0m05s\n```\n\n"
        ),
        "{body}"
    );
    assert!(!body.contains("test:"), "{body}");
}

#[test]
fn a_failed_changed_line_gate_fails_the_cell() {
    let mut cell = inputs("");
    cell.changed_lines_configured = true;
    cell.changed_rc = Some("1\n".to_owned());
    cell.changed_lines = "\n  \nchanged-line coverage fail: 1/2 lines\nchanged_line_uncovered: a:1\n".to_owned();
    let body = render(&cell);
    assert!(body.contains("— FAIL\n"), "{body}");
    assert!(body.contains("Changed-line gate exit: 1\n\nchanged-line coverage fail: 1/2 lines\n\n"), "{body}");
    assert!(!body.contains("changed_line_uncovered"), "{body}");
}

#[test]
fn a_skipped_or_missing_changed_line_gate_does_not_fail_the_cell() {
    for (rc, shown) in [(Some("skipped"), "skipped"), (None, "not run")] {
        let mut cell = inputs("");
        cell.changed_lines_configured = true;
        cell.changed_rc = rc.map(str::to_owned);
        let body = render(&cell);
        assert!(body.contains("— PASS\n"), "{body}");
        assert!(body.contains(&format!("Changed-line gate exit: {shown}\n")), "{body}");
    }
}

#[test]
fn clock_rounds_down_to_whole_seconds() {
    assert_eq!(clock(0.0), "0m00s");
    assert_eq!(clock(59_999_999.0), "0m59s");
    assert_eq!(clock(3_661_000_000.0), "61m01s");
}
