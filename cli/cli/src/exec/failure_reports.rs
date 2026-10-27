use super::build_failures::to_event;
use super::common::*;
use dx_bep::{collect_build_failures, BuildFailure, StderrRef};
use dx_output::{build_failure_event, report_event, OutputMode};
use dx_plan::args::Invocation;
use dx_plan::reports::PlannedReport;
use serde_json::{json, Map, Value};
use std::io::{BufReader, Read, Write};
use std::path::Path;

const FAILURE_LIMIT: usize = 200;
const STDERR_LIMIT: u64 = 64 * 1024;
const TOTAL_LIMIT: u64 = 1024 * 1024;

struct Capture {
    status: &'static str,
    bytes: u64,
    truncated: bool,
    redacted_lines: u64,
    text: Option<String>,
}

fn is_redacted(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("(cd ") || line.starts_with("exec env") || line.contains(" exec env ")
}

fn unavailable(status: &'static str) -> Capture {
    Capture {
        status,
        bytes: 0,
        truncated: false,
        redacted_lines: 0,
        text: None,
    }
}

fn capture_stderr(failure: &BuildFailure, structured: bool, budget: &mut u64) -> Capture {
    let path = match &failure.stderr {
        StderrRef::Remote => return unavailable("remote"),
        StderrRef::Absent => return unavailable("absent"),
        StderrRef::Local(path) => path,
    };
    let Ok(file) = std::fs::File::open(path) else {
        return unavailable("unreadable");
    };
    let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if structured {
        return Capture {
            status: "structured",
            bytes: size,
            ..unavailable("structured")
        };
    }
    let allowed = STDERR_LIMIT.min(*budget);
    if allowed == 0 {
        return Capture {
            bytes: size,
            ..unavailable("omitted_limit")
        };
    }
    let mut bytes = Vec::new();
    if file.take(allowed).read_to_end(&mut bytes).is_err() {
        return unavailable("unreadable");
    }
    *budget -= bytes.len() as u64;
    let truncated = size > bytes.len() as u64;
    let lossy = String::from_utf8_lossy(&bytes);
    let mut redacted_lines = 0;
    let mut kept = Vec::new();
    for line in lossy.lines() {
        if is_redacted(line) {
            redacted_lines += 1;
        } else {
            kept.push(line);
        }
    }
    Capture {
        status: if truncated { "truncated" } else { "captured" },
        bytes: size,
        truncated,
        redacted_lines,
        text: Some(kept.join("\n")),
    }
}

fn record(failure: &BuildFailure, budget: &mut u64) -> Value {
    let event = to_event(failure);
    let structured = event.diagnostics_status == "parsed";
    let mut value = build_failure_event(&event).unwrap_or_else(|_| json!({}));
    let capture = capture_stderr(failure, structured, budget);
    if let Some(map) = value.as_object_mut() {
        map.remove("event");
        let mut stderr = Map::new();
        stderr.insert("status".to_owned(), Value::from(capture.status));
        stderr.insert("bytes".to_owned(), Value::from(capture.bytes));
        stderr.insert("truncated".to_owned(), Value::Bool(capture.truncated));
        stderr.insert(
            "redacted_lines".to_owned(),
            Value::from(capture.redacted_lines),
        );
        if let Some(text) = capture.text {
            stderr.insert("text".to_owned(), Value::String(text));
        }
        map.insert("stderr".to_owned(), Value::Object(stderr));
    }
    value
}

fn document(bep: &Path, bazel_code: i32) -> (String, bool) {
    let mut problem = None;
    let mut records: Vec<Value> = Vec::new();
    let mut omitted = 0usize;
    if bazel_code != 0 {
        match std::fs::File::open(bep) {
            Ok(file) => {
                let collected = collect_build_failures(BufReader::new(file));
                problem = collected.problem;
                let mut budget = TOTAL_LIMIT;
                for failure in &collected.failures {
                    if records.len() >= FAILURE_LIMIT {
                        omitted += 1;
                    } else {
                        records.push(record(failure, &mut budget));
                    }
                }
            }
            Err(error) => problem = Some(format!("build events are unreadable: {error}")),
        }
    }
    let complete = problem.is_none() && omitted == 0;
    let truncated = omitted > 0
        || records.iter().any(|record| {
            matches!(
                record["stderr"]["status"].as_str(),
                Some("truncated" | "omitted_limit")
            )
        });
    let mut root = Map::new();
    root.insert("record_version".to_owned(), Value::from(1));
    root.insert("exit_code".to_owned(), Value::from(bazel_code));
    root.insert("complete".to_owned(), Value::Bool(complete));
    root.insert("truncated".to_owned(), Value::Bool(truncated));
    root.insert("failures_omitted".to_owned(), Value::from(omitted));
    if let Some(problem) = problem {
        root.insert("problem".to_owned(), Value::String(problem));
    }
    root.insert("failures".to_owned(), Value::Array(records));
    let text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
    (text, complete)
}

/// Writes the planned failure reports and returns whether every one landed.
pub(crate) fn write_failure_reports(
    invocation: &Invocation,
    workspace: &Path,
    bep: &Path,
    bazel_code: i32,
    reports: &[PlannedReport],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    if reports.is_empty() {
        return Ok(true);
    }
    let (text, complete) = document(bep, bazel_code);
    let mut all_ok = true;
    for planned in reports {
        if let Err(error) = write_report_document(out, workspace, &planned.destination, &text) {
            all_ok = false;
            report_failed(
                out,
                err,
                invocation.output,
                &format!("failed to write failures report to {error}"),
            )?;
            continue;
        }
        if invocation.output == OutputMode::Json {
            emit_built(
                out,
                report_event("failures", planned.destination.display(), complete),
            )?;
        } else if matches!(invocation.output, OutputMode::Text { .. })
            && planned.destination != dx_plan::reports::Destination::Stdout
        {
            let _ = writeln!(
                out,
                "Wrote failures report to {}.",
                planned.destination.display()
            );
        }
    }
    Ok(all_ok)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use serde_json::{json, Value};

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
        let (code, out, _) =
            harness.run(&["test", "--report=junit=j.xml", "--report=failures=f.json"]);
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
}
