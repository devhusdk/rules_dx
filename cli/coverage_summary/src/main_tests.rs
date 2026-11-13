use super::*;

fn cli(temp: &Path, invocation_id: &str) -> Cli {
    let temp = temp.display().to_string();
    let summary = format!("{temp}/summary.md");
    Cli::try_parse_from([
        "coverage_summary",
        "--runner-temp",
        &temp,
        "--step-summary",
        &summary,
        "--platform",
        "linux_x86_64",
        "--invocation-id",
        invocation_id,
        "--min-changed-line-coverage",
        "",
        "--sha",
        "abc123",
        "--server-url",
        "https://github.example",
        "--repository",
        "devhusdk/rules_dx",
        "--run-id",
        "1",
    ])
    .expect("parses")
}

#[test]
fn the_comment_is_written_and_appended_to_the_step_summary() {
    let scratch = dx_test_scratch::scratch("coverage-summary");
    let temp = scratch.path();
    fs::create_dir_all(temp.join("dx-reports/coverage")).expect("reports dir");
    fs::create_dir_all(temp.join("dx-reports/test")).expect("profile dir");
    fs::write(temp.join("dx_coverage.rc"), "0\n").expect("rc");
    fs::write(
        temp.join("dx-reports/coverage/coverage.ndjson"),
        r#"{"code":"coverage_sources","message":"m"}"#,
    )
    .expect("events");
    fs::write(
        temp.join("dx-reports/test/profile-test.json"),
        r#"{"cache":{"actions":1,"hit_percent":100,"remote_hits":1,"disk_hits":0},"wall_micros":0,"critical_path_micros":0}"#,
    )
    .expect("profile");
    fs::write(temp.join("summary.md"), "before\n").expect("summary");
    let cli = cli(temp, "b");
    write(&cli).expect("writes");
    let body = fs::read_to_string(temp.join("coverage_comment.md")).expect("comment");
    assert_eq!(
        body.lines().next(),
        Some("<!-- dx-coverage-summary: coverage linux_x86_64 b -->")
    );
    assert!(body.contains("— PASS\n"), "{body}");
    assert!(body.contains("coverage_sources: m\n"), "{body}");
    assert!(body.contains("test: 100% cache hits (remote 100%, disk 0%)"), "{body}");
    assert!(body.contains("Changed-line gate exit: not configured\n"), "{body}");
    let summary = fs::read_to_string(temp.join("summary.md")).expect("summary");
    assert_eq!(summary, format!("before\n{body}"));
}

#[test]
fn an_empty_runner_temp_renders_unknown_and_not_configured() {
    let scratch = dx_test_scratch::scratch("coverage-summary-empty");
    let cli = cli(scratch.path(), "");
    let body = render(&inputs(&cli));
    assert!(body.contains("Gate exit: unknown "), "{body}");
    assert!(body.contains("— FAIL\n"), "{body}");
    assert!(!body.contains("```"), "{body}");
}

#[test]
fn every_required_flag_is_required() {
    assert!(Cli::try_parse_from(["coverage_summary", "--platform", "x"]).is_err());
}
