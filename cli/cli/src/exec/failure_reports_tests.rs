use super::test_support::*;
use serde_json::{Value, json};

const ERROR_LINE: &str = r#"{"$message_type":"diagnostic","message":"cannot find value `y` in this scope","code":{"code":"E0425","explanation":null},"level":"error","spans":[{"file_name":"app/src/main.rs","byte_start":30,"byte_end":31,"line_start":3,"line_end":3,"column_start":5,"column_end":6,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}"#;

fn stderr_uri(harness: &Harness, name: &str, text: &str) -> String {
    let dir = harness.temp.join("stderr");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("stderr");
    format!("file://{}", path.to_string_lossy())
}

fn action(label: &str, mnemonic: &str, uri: Option<&str>) -> String {
    let mut body = json!({
        "success": false,
        "type": mnemonic,
        "exitCode": 1,
        "label": label,
        "configuration": {"id": "c1"},
        "failureDetail": {
            "message": "failed: exec env - SECRET_TOKEN=hunter2 /bin/tool",
            "spawn": {"code": "NON_ZERO_EXIT"},
        },
    });
    if let Some(uri) = uri {
        body["stderr"] = json!({"name": "stderr", "uri": uri});
    }
    json!({
        "id": {"actionCompleted": {
            "primaryOutput": format!("bazel-out/bin/{label}"),
            "label": label,
            "configuration": {"id": "c1"},
        }},
        "action": body,
    })
    .to_string()
}

fn with_bep(name: &str, code: i32, lines: Vec<String>) -> Harness {
    Harness {
        bazel_code: code,
        raw_bep: Some(lines),
        ..Harness::new(name)
    }
}

fn read_report(harness: &Harness, name: &str) -> Value {
    let text = std::fs::read_to_string(harness.workspace.join(name)).expect("report file");
    serde_json::from_str(&text).expect("report is JSON")
}

#[test]
fn a_rust_failure_is_written_with_diagnostics_and_no_raw_text() {
    let harness = Harness::new("fr-rust");
    let uri = stderr_uri(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let harness = with_bep("fr-rust", 1, vec![action("//app:bin", "Rustc", Some(&uri))]);
    let (code, out, _) = harness.run(&["build", "--report=failures=f.json"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("Wrote failures report to f.json."), "{out}");
    let report = read_report(&harness, "f.json");
    assert_eq!(report["record_version"], 1);
    assert_eq!(report["exit_code"], 1);
    assert_eq!(report["complete"], true);
    let failure = &report["failures"][0];
    assert_eq!(failure["target"], "//app:bin");
    assert_eq!(failure["diagnostics"]["items"][0]["code"], "E0425");
    assert_eq!(failure["diagnostics"]["items"][0]["line"], 3);
    assert_eq!(failure["stderr"]["status"], "structured");
    assert!(failure["stderr"].get("text").is_none());
}

#[test]
fn other_tools_keep_bounded_raw_text_without_positions() {
    let harness = Harness::new("fr-raw");
    let uri = stderr_uri(&harness, "e1", "ld: undefined symbol: main\n");
    let harness = with_bep("fr-raw", 1, vec![action("//c:bin", "CppLink", Some(&uri))]);
    let (code, _, _) = harness.run(&["test", "--report=failures=f.json"]);
    assert_eq!(code, 1);
    let failure = &read_report(&harness, "f.json")["failures"][0];
    assert_eq!(failure["stderr"]["status"], "captured");
    assert_eq!(failure["stderr"]["text"], "ld: undefined symbol: main");
    assert_eq!(failure["diagnostics"]["status"], "unknown");
}

#[test]
fn large_stderr_is_cut_and_marked() {
    let harness = Harness::new("fr-trunc");
    let big = "x".repeat(200 * 1024);
    let uri = stderr_uri(&harness, "e1", &big);
    let harness = with_bep(
        "fr-trunc",
        1,
        vec![action("//c:bin", "CppLink", Some(&uri))],
    );
    let (_, _, _) = harness.run(&["coverage", "--report=failures=f.json"]);
    let report = read_report(&harness, "f.json");
    let stderr = &report["failures"][0]["stderr"];
    assert_eq!(stderr["status"], "truncated");
    assert_eq!(stderr["truncated"], true);
    assert_eq!(stderr["bytes"], 200 * 1024);
    assert_eq!(stderr["text"].as_str().expect("text").len(), 64 * 1024);
    assert_eq!(report["truncated"], true);
}

#[test]
fn the_total_budget_marks_later_failures_omitted() {
    let harness = Harness::new("fr-budget");
    let big = "x".repeat(60 * 1024);
    let mut lines = Vec::new();
    for index in 0..20 {
        let uri = stderr_uri(&harness, &format!("e{index}"), &big);
        lines.push(action(&format!("//c:t{index}"), "CppLink", Some(&uri)));
    }
    let harness = with_bep("fr-budget", 1, lines);
    harness.run(&["build", "--report=failures=f.json"]);
    let report = read_report(&harness, "f.json");
    let failures = report["failures"].as_array().expect("failures");
    assert_eq!(failures.len(), 20);
    assert_eq!(failures[19]["stderr"]["status"], "omitted_limit");
    assert_eq!(report["truncated"], true);
}

#[test]
fn remote_missing_and_absent_stderr_are_marked_unavailable() {
    let scratch = tempfile::tempdir().expect("scratch");
    let gone = dx_path::uri(&scratch.path().join("gone").join("stderr")).expect("uri");
    let harness = with_bep(
        "fr-missing",
        1,
        vec![
            action(
                "//a:remote",
                "CppLink",
                Some("bytestream://cache/blobs/ab/1"),
            ),
            action("//a:gone", "CppLink", Some(&gone)),
            action("//a:none", "CppLink", None),
        ],
    );
    harness.run(&["build", "--report=failures=f.json"]);
    let failures = read_report(&harness, "f.json")["failures"].clone();
    assert_eq!(failures[0]["stderr"]["status"], "remote");
    assert_eq!(failures[1]["stderr"]["status"], "unreadable");
    assert_eq!(failures[2]["stderr"]["status"], "absent");
    for index in 0..3 {
        assert!(failures[index]["stderr"].get("text").is_none());
    }
}

#[test]
fn secrets_and_command_lines_never_reach_the_report() {
    let harness = Harness::new("fr-redact");
    let uri = stderr_uri(
        &harness,
        "e1",
        "(cd /x && exec env - SECRET_TOKEN=hunter2 /bin/tool)\nreal error\n",
    );
    let harness = with_bep(
        "fr-redact",
        1,
        vec![action("//c:bin", "CppLink", Some(&uri))],
    );
    harness.run(&["build", "--report=failures=f.json"]);
    let text = std::fs::read_to_string(harness.workspace.join("f.json")).expect("file");
    assert!(!text.contains("hunter2"), "{text}");
    assert!(!text.contains("SECRET_TOKEN"), "{text}");
    let failure = &serde_json::from_str::<Value>(&text).expect("json")["failures"][0];
    assert_eq!(failure["stderr"]["text"], "real error");
    assert_eq!(failure["stderr"]["redacted_lines"], 1);
}

#[test]
fn a_successful_run_writes_an_empty_complete_report() {
    let harness = Harness::new("fr-ok");
    let (code, _, _) = harness.run(&["build", "--report=failures=f.json"]);
    assert_eq!(code, 0);
    let report = read_report(&harness, "f.json");
    assert_eq!(report["failures"], json!([]));
    assert_eq!(report["complete"], true);
}

#[test]
fn a_truncated_event_stream_marks_the_report_incomplete() {
    let harness = with_bep(
        "fr-incomplete",
        7,
        vec![
            action("//c:bin", "CppLink", None),
            r#"{"id":{"actionCompleted":{"prim"#.to_owned(),
        ],
    );
    let (code, _, _) = harness.run(&["build", "--report=failures=f.json"]);
    assert_eq!(code, 7);
    let report = read_report(&harness, "f.json");
    assert_eq!(report["complete"], false);
    assert!(report["problem"].is_string());
    assert_eq!(report["failures"].as_array().expect("list").len(), 1);
}

#[test]
fn a_cancelled_build_is_a_cancelled_record_and_keeps_bazels_code() {
    let line = json!({
        "id": {"targetCompleted": {"label": "//a:t"}},
        "aborted": {"reason": "USER_INTERRUPTED"},
    })
    .to_string();
    let harness = with_bep("fr-cancel", 8, vec![line]);
    let (code, _, _) = harness.run(&["build", "--report=failures=f.json"]);
    assert_eq!(code, 8);
    let report = read_report(&harness, "f.json");
    assert_eq!(report["exit_code"], 8);
    assert_eq!(report["failures"][0]["category"], "cancelled");
}

#[test]
fn a_failed_write_keeps_bazels_code_and_the_old_bytes() {
    let harness = with_bep("fr-write", 3, vec![action("//c:bin", "CppLink", None)]);
    std::fs::write(harness.workspace.join("f.json"), "old").expect("old");
    let (code, out, err) = harness.run(&["build", "--report=failures=missing/f.json"]);
    assert_eq!(code, 3, "{out}");
    assert!(err.contains("report_failed"), "{err}");
    let (code, _, err) = harness.run(&["build", "--report=failures=."]);
    assert_eq!(code, 3, "{err}");
    assert_eq!(
        std::fs::read_to_string(harness.workspace.join("f.json")).expect("old"),
        "old"
    );
}

#[test]
fn a_failed_write_fails_a_run_bazel_passed() {
    let harness = Harness::new("fr-write-ok");
    let (code, _, err) = harness.run(&["build", "--report=failures=missing/f.json"]);
    assert_eq!(code, 1, "{err}");
    let (code, out, _) =
        harness.run(&["test", "--output=json", "--report=failures=missing/f.json"]);
    assert_eq!(code, 1, "{out}");
}

#[test]
fn the_report_shares_a_run_with_junit_and_may_go_to_stdout() {
    let harness = with_bep("fr-both", 1, vec![action("//c:bin", "CppLink", None)]);
    let (code, out, _) = harness.run(&["test", "--report=junit=j.xml", "--report=failures=f.json"]);
    assert_eq!(code, 1, "{out}");
    assert!(harness.workspace.join("j.xml").exists());
    assert_eq!(
        read_report(&harness, "f.json")["failures"][0]["target"],
        "//c:bin"
    );
    let (code, out, _) = harness.run(&["build", "--report=failures=-"]);
    assert_eq!(code, 1);
    let report: Value = serde_json::from_str(out.trim()).expect("stdout is the report");
    assert_eq!(report["failures"][0]["target"], "//c:bin");
    let (code, _, err) = harness.run(&["build", "--output=json", "--report=failures=-"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = harness.run(&["build", "--report=sarif=s.sarif"]);
    assert_eq!(code, 2, "{err}");
}

struct BrokenPipe;

impl std::io::Write for BrokenPipe {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

#[test]
fn a_broken_stdout_keeps_bazels_code() {
    let harness = with_bep("fr-broken", 5, vec![action("//c:bin", "CppLink", None)]);
    for words in [
        &["build", "--report=failures=-"][..],
        &["test", "--report=failures=-"][..],
    ] {
        let inv = invocation(words);
        let runner = harness.runner();
        let code = harness.run_streams(&inv, &runner, &mut BrokenPipe, &mut Vec::new());
        assert_eq!(code, 5, "{words:?}");
    }
}

#[test]
fn a_broken_stdout_fails_the_announcement_only_when_bazel_passed() {
    use super::delivery_sink::{Fault, Sink};
    for (bazel_code, expected) in [(0, dx_process::broken_pipe_code()), (5, 5)] {
        let lines = if bazel_code == 0 {
            Vec::new()
        } else {
            vec![action("//c:bin", "CppLink", None)]
        };
        let harness = with_bep("fr-broken-announce", bazel_code, lines);
        let runner = harness.runner();
        let mut before = Sink::new(Fault::None, 0);
        harness.run_streams(
            &invocation(&["build"]),
            &runner,
            &mut before,
            &mut Vec::new(),
        );
        let inv = invocation(&["build", "--report=failures=f.json"]);
        let mut out = Sink::new(
            Fault::WriteFails(std::io::ErrorKind::BrokenPipe),
            before.writes() + 1,
        );
        let code = harness.run_streams(&inv, &runner, &mut out, &mut Vec::new());
        assert_eq!(out.text(), before.text(), "bazel exit {bazel_code}");
        assert_eq!(code, expected, "bazel exit {bazel_code}");
        assert!(harness.workspace.join("f.json").is_file());
    }
}
