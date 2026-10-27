use super::super::test_support::*;
use crate::resolve::QueryResult;
use quality_result::proto::Convergence;
use quality_result::{decode_validated, encode_validated};

fn located(source: &str, start: u64, end: u64) -> Harness {
    let mut harness = Harness::new("text-location");
    harness.write_source("src/a.py", source);
    let mut diagnostic = Harness::diagnostic("unused", false);
    diagnostic.start_byte = Some(start);
    diagnostic.end_byte = Some(end);
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![diagnostic], vec![]),
    );
    harness
}

fn clean() -> Harness {
    let mut harness = Harness::new("text-clean");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    harness
}

fn first_error(out: &str) -> serde_json::Value {
    let events = json_events(out);
    events_of_kind(&events, "error")[0].clone()
}

fn finished_code(out: &str) -> i64 {
    let events = json_events(out);
    events_of_kind(&events, "command_finished")[0]["exit_code"]
        .as_i64()
        .expect("exit_code")
}

#[test]
fn unicode_locations_count_scalar_values_in_text_and_json() {
    let source = "\u{fc}=\u{1f496}x\n";
    let (code, _, err) = located(source, 7, 8).run(&["lint", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("src/a.py:1:4: warning: unused [lint-tool/rule]") || err.is_empty(),
        "{err}"
    );
    let (code, out, _) = located(source, 7, 8).run(&["lint", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let range = &events_of_kind(&events, "diagnostic")[0]["range"];
    assert_eq!(range["start_line"], serde_json::json!(1));
    assert_eq!(range["start_column"], serde_json::json!(4));
}

#[test]
fn crlf_sources_locate_the_second_line() {
    let source = "a = 1\r\nb = 2\r\n";
    let (code, out, err) = located(source, 7, 8).run(&["lint", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    let text = format!("{out}{err}");
    assert!(
        text.contains("src/a.py:2:1: warning: unused [lint-tool/rule]"),
        "{text}"
    );
    let (_, json, _) = located(source, 7, 8).run(&["lint", "--output=json"]);
    let events = json_events(&json);
    let range = &events_of_kind(&events, "diagnostic")[0]["range"];
    assert_eq!(range["start_line"], serde_json::json!(2));
    assert_eq!(range["start_column"], serde_json::json!(1));
}

#[test]
fn unlocated_diagnostics_say_so_and_never_invent_a_position() {
    let mut harness = Harness::new("text-unlocated");
    harness.write_source("src/a.py", "x = 1\n");
    let mut diagnostic = Harness::diagnostic("config is stale", false);
    diagnostic.path = String::new();
    diagnostic.start_byte = None;
    diagnostic.end_byte = None;
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![diagnostic], vec![]),
    );
    let (code, out, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1, "{err}");
    let text = format!("{out}{err}");
    assert!(
        text.contains("(no file): warning: config is stale [lint-tool/rule]"),
        "{text}"
    );
    assert!(!text.contains(":1:1"), "{text}");
}

#[test]
fn managed_changes_print_the_exact_quoted_apply_command() {
    let mut harness = Harness::new("text-apply-hint");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    let (code, _, err) = harness.run(&["lint", "--output=text", "--", "--config=ci"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("1 file(s) need changes. Apply them with: dx lint --apply -- --config=ci\n"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    let (_, json_out, json_err) = harness.run(&["lint", "--output=json"]);
    assert!(!json_err.contains("Apply them"), "{json_err}");
    assert!(!json_out.contains("Apply them"), "{json_out}");
}

#[test]
fn tool_failure_text_and_json_agree_on_cause_and_status() {
    let build = || {
        let mut harness = Harness {
            bazel_code: 1,
            ..Harness::new("text-tool-failure")
        };
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness
    };
    let (text_code, _, err) = build().run(&["lint", "--output=text"]);
    let (json_code, out, _) = build().run(&["lint", "--output=json"]);
    assert_eq!(text_code, json_code);
    assert!(
        err.contains("dx: analysis_failed: lint analysis failed with Bazel exit 1"),
        "{err}"
    );
    assert!(
        err.contains("next: fix the Bazel error above, then run: dx lint\n"),
        "{err}"
    );
    assert!(!err.contains("--apply"), "{err}");
    let events = json_events(&out);
    let codes: Vec<&str> = events_of_kind(&events, "error")
        .iter()
        .filter_map(|event| event["code"].as_str())
        .collect();
    assert!(codes.contains(&"analysis_failed"), "{codes:?}");
    assert_eq!(finished_code(&out), i64::from(json_code));
}

#[test]
fn a_failed_clippy_build_lists_the_rustc_error_in_text_and_json() {
    let build = || {
        let harness = Harness::new("text-clippy-compile");
        harness.write_source("app/src/main.rs", "fn main() {}\n");
        let uri = stderr_file(&harness, "clippy", RENDERED_E0308);
        let action = failed_action(
            "//app:bin",
            "c1",
            "Clippy",
            "out/bin.clippy.diagnostics",
            &uri,
        );
        Harness {
            bazel_code: 1,
            raw_bep: Some(vec![action]),
            ..harness
        }
    };
    let (text_code, _, err) = build().run(&["lint", "--output=text"]);
    let (json_code, out, _) = build().run(&["lint", "--output=json"]);
    assert_eq!(text_code, 1, "{err}");
    assert_eq!(json_code, 1, "{out}");
    assert!(
        err.contains("dx: build failure: //app:bin (compilation/compile_failed, Clippy)"),
        "{err}"
    );
    assert!(
        err.contains("error[E0308]: mismatched types (app/src/main.rs:4:26)"),
        "{err}"
    );
    let events = json_events(&out);
    let failures = events_of_kind(&events, "build_failure");
    assert_eq!(failures.len(), 1, "{out}");
    let failure = &failures[0];
    assert_eq!(failure["category"], "compile_failed");
    assert_eq!(failure["tool"], "clippy");
    let item = &failure["diagnostics"]["items"][0];
    assert_eq!(item["message"], "mismatched types");
    assert_eq!(item["path"], "app/src/main.rs");
    assert_eq!(item["line"], 4);
    assert!(
        err.contains(
            "dx: bazel_failed: lint build failed with Bazel exit 1; results are incomplete"
        ),
        "{err}"
    );
    assert!(
        err.contains("next: fix the compile errors above, then run: dx lint\n"),
        "{err}"
    );
    assert!(!err.contains("analysis_failed"), "{err}");
    let errors = events_of_kind(&events, "error");
    assert!(
        errors
            .iter()
            .any(|event| event["code"] == "bazel_failed" && event["cause"] == "bazel"),
        "{out}"
    );
    assert!(!out.contains("analysis_failed"), "{out}");
    assert_eq!(finished_code(&out), 1);
}

#[test]
fn disk_full_text_and_json_agree_on_host_cause_and_exit_36() {
    let build = || {
        let mut harness = Harness {
            bazel_code: 36,
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("text-disk-full")
        };
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness
    };
    let (text_code, _, err) = build().run(&["lint", "--output=text"]);
    let (json_code, out, _) = build().run(&["lint", "--output=json"]);
    assert_eq!(text_code, 36);
    assert_eq!(json_code, 36);
    assert!(
        err.contains("dx: disk_full: Bazel exited 36 with 1.0 MiB free at "),
        "{err}"
    );
    assert!(err.contains("dx worktree prune --apply"), "{err}");
    assert!(!err.contains("analysis_failed"), "{err}");
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(errors[0]["code"], serde_json::json!("disk_full"));
    assert_eq!(errors[0]["cause"], serde_json::json!("host"));
    assert_eq!(errors[0]["status"], serde_json::json!(36));
    assert_eq!(finished_code(&out), 36);
}

#[test]
fn disk_full_from_a_query_text_and_json_agree_on_host_cause_and_exit_36() {
    let build = || {
        let harness = Harness {
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("text-query-disk-full")
        };
        harness.write_source("src/a.py", "x = 1\n");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(36),
            stdout: Vec::new(),
            stderr: b"ERROR: no space left\n".to_vec(),
        });
        harness
    };
    let (text_code, _, err) = build().run(&["list-tests", "//...", "--output=text"]);
    let (json_code, out, _) = build().run(&["list-tests", "//...", "--output=json"]);
    assert_eq!(text_code, 36, "{err}");
    assert_eq!(json_code, 36);
    assert!(err.contains("dx: disk_full: query "), "{err}");
    assert!(err.contains("dx worktree prune --apply"), "{err}");
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(errors[0]["code"], serde_json::json!("disk_full"));
    assert_eq!(errors[0]["cause"], serde_json::json!("host"));
    assert_eq!(errors[0]["status"], serde_json::json!(36));
    assert_eq!(finished_code(&out), 36);
}

#[test]
fn nonconvergence_text_and_json_agree_and_suggest_a_check_rerun() {
    let build = || {
        let mut harness = Harness::new("text-nonconvergence");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = harness.valid_result(vec![], vec![]);
        let mut result = decode_validated(&bytes).expect("valid");
        result.convergence = Convergence::Oscillation as i32;
        result.completed_rounds = 3;
        harness.results.insert(
            "//test:corpus".to_owned(),
            encode_validated(&result).expect("encode"),
        );
        harness
    };
    let (text_code, _, err) = build().run(&["lint", "--output=text"]);
    let (json_code, out, _) = build().run(&["lint", "--output=json"]);
    assert_eq!(text_code, json_code);
    assert!(
        err.contains("dx: not_converged: //test:corpus did not converge"),
        "{err}"
    );
    assert!(
        err.contains("next: fix the conflicting tool configuration, then run: dx lint\n"),
        "{err}"
    );
    assert_eq!(
        first_error(&out)["code"],
        serde_json::json!("not_converged")
    );
    assert_eq!(finished_code(&out), i64::from(json_code));
}

#[test]
fn incomplete_collection_text_and_json_agree_on_the_cause() {
    let build = || {
        let mut harness = clean();
        harness.skip_bep = true;
        harness
    };
    let (text_code, _, err) = build().run(&["lint", "--output=text"]);
    let (json_code, out, _) = build().run(&["lint", "--output=json"]);
    assert_eq!(text_code, json_code);
    assert_ne!(text_code, 0);
    let error = first_error(&out);
    let code = error["code"].as_str().expect("code");
    assert!(err.contains(&format!("dx: {code}:")), "{err}");
    assert_eq!(error["cause"], serde_json::json!("collection"));
    assert!(err.contains("next: "), "{err}");
}

#[test]
fn report_write_failure_text_and_json_agree_and_name_the_next_step() {
    let args = |output: &'static str| ["lint", output, "--report=sarif=nodir/out.sarif"];
    let (text_code, _, err) = clean().run(&args("--output=text"));
    let (json_code, out, _) = clean().run(&args("--output=json"));
    assert_eq!(text_code, json_code);
    assert_eq!(text_code, 1);
    assert!(
        err.contains("dx: report_failed: failed to write sarif report to "),
        "{err}"
    );
    assert!(
        err.contains("next: make the report destination writable and run the command again.\n"),
        "{err}"
    );
    let error = first_error(&out);
    assert_eq!(error["code"], serde_json::json!("report_failed"));
    assert_eq!(error["cause"], serde_json::json!("report"));
}

#[test]
fn presentation_flags_change_neither_findings_nor_exit() {
    let render = |extra: &[&str]| {
        let mut words = vec!["lint", "--output=text"];
        words.extend_from_slice(extra);
        let (code, out, err) = located("x = 1\n", 0, 1).run(&words);
        let finding: Vec<String> = format!("{out}{err}")
            .lines()
            .filter(|line| line.contains("warning: unused"))
            .map(str::to_owned)
            .collect();
        (code, finding)
    };
    let plain = render(&[]);
    assert_eq!(plain.0, 1);
    assert_eq!(plain.1.len(), 1, "{plain:?}");
    assert!(
        plain.1[0].starts_with("src/a.py:1:1: warning: unused"),
        "{plain:?}"
    );
    for extra in [&["--quiet"][..], &["--verbose"][..], &["--color=never"][..]] {
        assert_eq!(render(extra), plain, "{extra:?}");
    }
}

#[test]
fn suggested_commands_never_name_removed_or_destructive_actions() {
    let mut harness = Harness {
        bazel_code: 1,
        ..Harness::new("text-banned")
    };
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    let (_, out, err) = harness.run(&["lint", "--output=text"]);
    let text = format!("{out}{err}");
    for banned in [
        "dry-run",
        "dx bazel",
        "dx verify",
        "dx rerun",
        "dx hooks",
        "dx setup",
        "--bazel",
        "--cwd",
        "--log-level",
        "rm -",
        "reset --hard",
        "clean --expunge",
    ] {
        assert!(!text.contains(banned), "{banned}: {text}");
    }
}
