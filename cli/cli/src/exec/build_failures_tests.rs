use super::test_support::*;
use dx_exec_reports::build_failures::{ReportedFailures, report_build_failures};
use serde_json::{Value, json};

const ERROR_LINE: &str = r#"{"$message_type":"diagnostic","message":"cannot find value `y` in this scope","code":{"code":"E0425","explanation":null},"level":"error","spans":[{"file_name":"app/src/main.rs","byte_start":30,"byte_end":31,"line_start":3,"line_end":3,"column_start":5,"column_end":6,"is_primary":true,"text":[],"label":"not found in this scope","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[{"message":"a local variable with a similar name exists: `x`","level":"help","spans":[{"file_name":"app/src/main.rs","line_start":3,"column_start":5,"is_primary":true,"label":null,"suggested_replacement":"x","suggestion_applicability":"MaybeIncorrect"}],"children":[]}],"rendered":null}"#;
const LINK_LINE: &str = r#"{"$message_type":"diagnostic","message":"linking with `cc` failed: exit status: 1","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

fn with_bep(name: &str, bazel_code: i32, lines: Vec<String>) -> Harness {
    Harness {
        bazel_code,
        raw_bep: Some(lines),
        ..Harness::new(name)
    }
}

fn failures_of(out: &str) -> Vec<Value> {
    json_events(out)
        .into_iter()
        .filter(|event| event["event"] == "build_failure")
        .collect()
}

#[test]
fn a_rust_compile_error_names_the_action_and_its_diagnostics() {
    let harness = Harness::new("bf-compile");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//app:bin", "c1", "Rustc", "bazel-out/bin/app/bin", &uri);
    let harness = with_bep("bf-compile", 1, vec![line]);
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 1, "{out}");
    let failure = &failures[0];
    assert_eq!(failure["stage"], "compilation");
    assert_eq!(failure["category"], "compile_failed");
    assert_eq!(failure["target"], "//app:bin");
    assert_eq!(failure["configuration"], "c1");
    assert_eq!(failure["tool"], "rustc");
    assert_eq!(failure["action"]["mnemonic"], "Rustc");
    assert_eq!(failure["action"]["exit_code"], 1);
    assert_eq!(failure["diagnostics"]["status"], "parsed");
    let item = &failure["diagnostics"]["items"][0];
    assert_eq!(item["code"], "E0425");
    assert_eq!(item["path"], "app/src/main.rs");
    assert_eq!(item["line"], 3);
    assert_eq!(item["column"], 5);
    let related = &item["related"];
    assert_eq!(related[0]["role"], "label");
    assert_eq!(related[0]["message"], "not found in this scope");
    assert_eq!(related[0]["line"], 3);
    assert_eq!(related[1]["role"], "help");
    assert_eq!(related[1]["replacement"], "x");
    assert_eq!(related[1]["applicability"], "MaybeIncorrect");
    assert!(!out.contains("hunter2"), "{out}");
    assert!(!out.contains("SECRET_TOKEN"), "{out}");
}

#[test]
fn a_cc_compile_error_reports_structured_diagnostics() {
    let harness = Harness::new("bf-cc");
    let uri = stderr_file(&harness, "e1", "src/a.c:3:5: error: 'x' undeclared\n");
    let line = failed_action("//app:lib", "c1", "CppCompile", "bazel-out/bin/a.o", &uri);
    let harness = with_bep("bf-cc", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["tool"], "cc", "{out}");
    assert_eq!(failures[0]["diagnostics"]["status"], "parsed");
    let first = &failures[0]["diagnostics"]["items"][0];
    assert_eq!(first["path"], "src/a.c", "{out}");
    assert_eq!(first["line"], 3);
    assert_eq!(first["column"], 5);
}

#[test]
fn a_javac_error_reports_structured_diagnostics() {
    let harness = Harness::new("bf-javac");
    let uri = stderr_file(&harness, "e1", "A.java:4: error: cannot find symbol\n");
    let line = failed_action("//app:lib", "c1", "Javac", "bazel-out/bin/a.jar", &uri);
    let harness = with_bep("bf-javac", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["tool"], "javac", "{out}");
    assert_eq!(failures[0]["diagnostics"]["status"], "parsed");
}

#[test]
fn a_failed_clippy_action_reads_rendered_rustc_errors() {
    let harness = Harness::new("bf-clippy");
    let uri = stderr_file(&harness, "e1", RENDERED_E0308);
    let line = failed_action(
        "//app:bin",
        "c1",
        "Clippy",
        "bazel-out/bin/app/bin.clippy",
        &uri,
    );
    let harness = with_bep("bf-clippy", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 1, "{out}");
    assert_eq!(failures[0]["category"], "compile_failed");
    assert_eq!(failures[0]["tool"], "clippy");
    assert_eq!(failures[0]["diagnostics"]["status"], "parsed");
    let item = &failures[0]["diagnostics"]["items"][0];
    assert_eq!(item["severity"], "error");
    assert_eq!(item["code"], "E0308");
    assert_eq!(item["message"], "mismatched types");
    assert_eq!(item["path"], "app/src/main.rs");
    assert_eq!(item["line"], 4);
    assert_eq!(item["column"], 26);
    let related = &item["related"];
    assert_eq!(related[0]["role"], "label");
    assert_eq!(related[0]["message"], "expected `u32`, found `&str`");
    assert_eq!(related[0]["path"], "app/src/main.rs");
    assert_eq!(related[0]["line"], 4);
    assert!(related[0].get("column").is_none());
}

#[test]
fn a_failed_pipelined_metadata_action_is_a_rustc_compile_failure() {
    let harness = Harness::new("bf-rmeta");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//app:lib", "c1", "RustcMetadata", "out/lib.rmeta", &uri);
    let harness = with_bep("bf-rmeta", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["stage"], "compilation", "{out}");
    assert_eq!(failures[0]["category"], "compile_failed");
    assert_eq!(failures[0]["tool"], "rustc");
    assert_eq!(failures[0]["diagnostics"]["status"], "parsed");
}

#[test]
fn a_link_failure_is_not_a_compile_failure() {
    let harness = Harness::new("bf-link");
    let uri = stderr_file(&harness, "e1", &format!("{LINK_LINE}\n"));
    let line = failed_action("//app:bin", "c1", "Rustc", "bazel-out/bin/app/bin", &uri);
    let harness = with_bep("bf-link", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["stage"], "linking", "{out}");
    assert_eq!(failures[0]["category"], "link_failed");
    let items = failures[0]["diagnostics"]["items"]
        .as_array()
        .expect("items");
    assert!(
        items.iter().all(|item| item.get("related").is_none()),
        "{out}"
    );
}

#[test]
fn a_test_target_that_fails_to_build_emits_no_test_case() {
    let harness = Harness::new("bf-test-build");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let configured = json!({
        "id": {"targetConfigured": {"label": "//app:unit"}},
        "configured": {"targetKind": "rust_test rule", "testSize": "SMALL"},
    })
    .to_string();
    let action = failed_action("//app:unit", "c1", "Rustc", "bazel-out/bin/app/unit", &uri);
    let harness = with_bep("bf-test-build", 1, vec![configured, action]);
    let (code, out, _) = harness.run(&["test", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 1, "{out}");
    assert_eq!(failures[0]["target_kind"], "test");
    assert_eq!(failures[0]["stage"], "compilation");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
}

#[test]
fn a_failed_test_build_writes_no_infrastructure_case_to_junit() {
    let harness = Harness::new("bf-junit");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let action = failed_action("//app:unit", "c1", "Rustc", "out/unit", &uri);
    let harness = with_bep("bf-junit", 1, vec![action]);
    let (code, _, _) = harness.run(&["test", "--report=junit=out.xml"]);
    assert_eq!(code, 1);
    let text = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
    assert!(!text.contains("<testcase"), "{text}");
}

#[test]
fn coverage_reports_build_failures_too() {
    let harness = Harness::new("bf-coverage");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let action = failed_action("//app:unit", "c1", "Rustc", "out/unit", &uri);
    let harness = with_bep("bf-coverage", 3, vec![action]);
    let (code, out, _) = harness.run(&["coverage", "--output=json"]);
    assert_eq!(code, 3, "{out}");
    assert_eq!(failures_of(&out).len(), 1, "{out}");
}

#[test]
fn several_actions_and_configurations_each_get_a_record() {
    let harness = Harness::new("bf-many");
    let one = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let two = stderr_file(&harness, "e2", &format!("{LINK_LINE}\n"));
    let lines = vec![
        failed_action("//a:lib", "c1", "Rustc", "out/a1", &one),
        failed_action("//a:lib", "c2", "Rustc", "out/a2", &two),
        failed_action("//b:bin", "c1", "CppLink", "out/b", &one),
    ];
    let harness = with_bep("bf-many", 1, lines);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 3, "{out}");
    assert_eq!(failures[0]["stage"], "compilation");
    assert_eq!(failures[1]["stage"], "linking");
    assert_eq!(failures[1]["configuration"], "c2");
    assert_eq!(failures[2]["stage"], "linking");
    assert_eq!(failures[2]["diagnostics"]["status"], "unknown");
    assert_eq!(failures[2]["tool"], "unknown");
}

#[test]
fn identical_failures_across_targets_and_configurations_are_counted_once() {
    let harness = Harness::new("bf-dedupe");
    let one = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let lines = vec![
        failed_action("//a:lib", "c1", "Rustc", "out/a1", &one),
        failed_action("//a:lib", "c2", "Rustc", "out/a2", &one),
        failed_action("//b:lib", "c1", "Rustc", "out/b1", &one),
        failed_action("//c:bin", "c1", "CppLink", "out/c", &one),
        failed_action("//d:bin", "c1", "CppLink", "out/d", &one),
    ];
    let harness = with_bep("bf-dedupe", 1, lines);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 3, "{out}");
    assert_eq!(failures[0]["target"], "//a:lib");
    assert_eq!(failures[0]["count"], 3);
    assert_eq!(failures[0]["targets"], json!(["//b:lib"]));
    assert!(failures[1].get("count").is_none());
    assert_eq!(failures[1]["target"], "//c:bin");
    assert_eq!(failures[2]["target"], "//d:bin");
}

#[test]
fn text_output_lists_every_merged_target_in_the_header() {
    let harness = Harness::new("bf-text-merge");
    let one = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let lines = vec![
        failed_action("//a:lib", "c1", "Rustc", "out/a1", &one),
        failed_action("//a:lib", "c2", "Rustc", "out/a2", &one),
        failed_action("//b:lib", "c1", "Rustc", "out/b1", &one),
    ];
    let harness = with_bep("bf-text-merge", 1, lines);
    let (_, _, err) = harness.run(&["build", "--output=text"]);
    assert!(
        err.contains("build failure: //a:lib, //b:lib (compilation/compile_failed, Rustc) x3"),
        "{err}"
    );
}

#[test]
fn text_output_for_one_target_in_two_configurations_has_no_comma() {
    let harness = Harness::new("bf-text-cfg");
    let one = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let lines = vec![
        failed_action("//a:lib", "c1", "Rustc", "out/a1", &one),
        failed_action("//a:lib", "c2", "Rustc", "out/a2", &one),
    ];
    let harness = with_bep("bf-text-cfg", 1, lines);
    let (_, _, err) = harness.run(&["build", "--output=text"]);
    assert!(
        err.contains("build failure: //a:lib (compilation/compile_failed, Rustc) x2"),
        "{err}"
    );
}

#[test]
fn non_ascii_paths_reach_the_record() {
    let harness = Harness::new("bf-unicode");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//ünï:bin", "c1", "Rustc", "bazel-out/ünï/日本", &uri);
    let harness = with_bep("bf-unicode", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["target"], "//ünï:bin");
    assert_eq!(
        failures[0]["action"]["primary_output"],
        "bazel-out/ünï/日本"
    );
}

#[test]
fn analysis_failure_has_no_action_and_leaves_unknown_facts_out() {
    let line = json!({
        "id": {"targetCompleted": {"label": "//a:t"}},
        "aborted": {"reason": "ANALYSIS_FAILURE", "description": "Analysis of target '//a:t' failed"},
    })
    .to_string();
    let harness = with_bep("bf-analysis", 1, vec![line]);
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 1);
    let failures = failures_of(&out);
    assert_eq!(failures.len(), 1, "{out}");
    assert_eq!(failures[0]["stage"], "analysis");
    assert_eq!(failures[0]["configuration"], "unknown");
    assert_eq!(failures[0]["action"]["mnemonic"], "unknown");
    assert_eq!(failures[0]["tool"], "unknown");
    assert_eq!(failures[0]["diagnostics"]["status"], "unknown");
}

#[test]
fn a_truncated_stream_keeps_the_status_and_says_it_is_incomplete() {
    let harness = Harness::new("bf-truncated");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let good = failed_action("//a:lib", "c1", "Rustc", "out/a", &uri);
    let harness = with_bep(
        "bf-truncated",
        7,
        vec![good, r#"{"id":{"actionCompleted":{"prim"#.to_owned()],
    );
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 7, "{out}");
    assert_eq!(failures_of(&out).len(), 1, "{out}");
    let events = json_events(&out);
    let notices = events_of_kind(&events, "notice");
    assert_eq!(notices[0]["code"], "build_events_incomplete");
}

#[test]
fn a_malformed_stream_never_changes_the_bazel_status() {
    let harness = with_bep("bf-malformed", 9, vec!["not json".to_owned()]);
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 9, "{out}");
    assert!(failures_of(&out).is_empty(), "{:?}", failures_of(&out));
    let events = json_events(&out);
    assert_eq!(event(&events, "command_finished")["exit_code"], 9);
}

#[test]
fn a_missing_event_file_is_said_not_guessed() {
    let mut harness = Harness::new("bf-missing");
    harness.bazel_code = 2;
    harness.skip_bep = true;
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 2, "{out}");
    assert!(failures_of(&out).is_empty(), "{:?}", failures_of(&out));
    let events = json_events(&out);
    assert_eq!(
        events_of_kind(&events, "notice")[0]["code"],
        "build_events_incomplete"
    );
}

#[test]
fn an_interrupted_build_is_a_cancellation_record() {
    let line = json!({
        "id": {"targetCompleted": {"label": "//a:t"}},
        "aborted": {"reason": "USER_INTERRUPTED"},
    })
    .to_string();
    let harness = with_bep("bf-cancel", 8, vec![line]);
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 8);
    assert_eq!(failures_of(&out)[0]["category"], "cancelled");
}

#[test]
fn a_signalled_bazel_reports_no_failures() {
    let mut harness = Harness::new("bf-signal");
    harness.signalled = true;
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    assert!(failures_of(&out).is_empty(), "{out}");
}

#[test]
fn a_successful_build_reports_no_failures() {
    let harness = Harness::new("bf-ok");
    let (code, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 0);
    assert!(failures_of(&out).is_empty(), "{:?}", failures_of(&out));
}

#[test]
fn json_stdout_holds_only_events() {
    let harness = Harness::new("bf-json-only");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//a:lib", "c1", "Rustc", "out/a", &uri);
    let harness = with_bep("bf-json-only", 1, vec![line]);
    let (_, out, err) = harness.run(&["build", "--output=json"]);
    assert_eq!(json_events(&out).len(), out.lines().count());
    assert!(!err.contains("build failure"), "{err}");
}

#[test]
fn text_output_lists_the_failure_on_stderr() {
    let harness = Harness::new("bf-text");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//a:lib", "c1", "Rustc", "out/a", &uri);
    let harness = with_bep("bf-text", 1, vec![line]);
    let (code, _, err) = harness.run(&["build", "--output=text"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("build failure: //a:lib (compilation/compile_failed, Rustc)"),
        "{err}"
    );
    assert!(
        err.contains("error[E0425]: cannot find value `y` in this scope (app/src/main.rs:3:5)"),
        "{err}"
    );
    assert!(
        err.contains("\n    label: not found in this scope (app/src/main.rs:3:5)"),
        "{err}"
    );
    assert!(
        err.contains("\n    help: a local variable with a similar name exists: `x` (app/src/main.rs:3:5) suggests `x` [MaybeIncorrect]"),
        "{err}"
    );
}

struct BrokenPipe {
    writes: usize,
}

impl std::io::Write for BrokenPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writes += 1;
        if self.writes == 1 {
            return Ok(buf.len());
        }
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

#[test]
fn a_broken_stdout_keeps_the_bazel_status() {
    let harness = Harness::new("bf-broken");
    let uri = stderr_file(&harness, "e1", &format!("{ERROR_LINE}\n"));
    let line = failed_action("//a:lib", "c1", "Rustc", "out/a", &uri);
    let harness = with_bep("bf-broken", 5, vec![line]);
    let inv = invocation(&["build", "--output=json"]);
    let runner = harness.runner();
    let code = harness.run_streams(
        &inv,
        &runner,
        &mut BrokenPipe { writes: 0 },
        &mut Vec::new(),
    );
    assert_eq!(code, 5);
}

#[test]
fn stderr_that_is_not_json_is_none_not_invented() {
    let harness = Harness::new("bf-plain");
    let uri = stderr_file(&harness, "e1", "  error: indented build script text\n");
    let line = failed_action("//a:lib", "c1", "Rustc", "out/a", &uri);
    let harness = with_bep("bf-plain", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    let failures = failures_of(&out);
    assert_eq!(failures[0]["diagnostics"]["status"], "none");
    assert_eq!(failures[0]["diagnostics"]["items"], json!([]));
}

#[test]
fn remote_stderr_is_unavailable() {
    let line = failed_action(
        "//a:lib",
        "c1",
        "Rustc",
        "out/a",
        "bytestream://cache/blobs/ab/1",
    );
    let harness = with_bep("bf-remote", 1, vec![line]);
    let (_, out, _) = harness.run(&["build", "--output=json"]);
    assert_eq!(failures_of(&out)[0]["diagnostics"]["status"], "unavailable");
}

#[test]
fn the_event_file_reaches_bazel_for_every_workflow_verb() {
    for verb in ["build", "test", "coverage"] {
        let harness = Harness::new("bf-flag");
        let inv = invocation(&[verb]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert!(
            run.argv[0]
                .iter()
                .any(|arg| arg.starts_with("--build_event_json_file=")),
            "{verb}: {:?}",
            run.argv
        );
        assert!(
            run.argv[0]
                .iter()
                .any(|arg| arg == "--build_event_json_file_path_conversion=false"),
            "{verb}: {:?}",
            run.argv
        );
    }
}

fn summary_of(harness: &Harness, lines: &[String]) -> ReportedFailures {
    let bep = harness.temp.join("summary.bep.json");
    std::fs::write(&bep, lines.join("\n")).expect("bep");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    report_build_failures(&bep, dx_output::OutputMode::Json, &mut out, &mut err).expect("reported")
}

fn compile_action(harness: &Harness, label: &str, mnemonic: &str, text: &str) -> String {
    let uri = stderr_file(harness, &label.replace(['/', ':'], "_"), text);
    failed_action(label, "c1", mnemonic, "out/bin", &uri)
}

#[test]
fn compile_only_failures_are_summarized_as_compile_only() {
    let harness = Harness::new("bf-sum-compile");
    let rustc = compile_action(&harness, "//a:lib", "Rustc", RENDERED_E0308);
    let clippy = compile_action(&harness, "//b:lib", "Clippy", RENDERED_E0308);
    let not_built = json!({
        "id": {"targetCompleted": {"label": "//c:top"}},
        "completed": {"success": false},
    })
    .to_string();
    let summary = summary_of(&harness, &[rustc, clippy, not_built.clone()]);
    assert_eq!(summary.count, 3);
    assert!(summary.compile_only);
    assert!(!summary_of(&harness, &[not_built]).compile_only);
}

#[test]
fn a_link_or_analysis_failure_is_not_compile_only() {
    let harness = Harness::new("bf-sum-mixed");
    let rustc = compile_action(&harness, "//a:lib", "Rustc", RENDERED_E0308);
    let link = compile_action(&harness, "//b:bin", "Rustc", &format!("{LINK_LINE}\n"));
    let summary = summary_of(&harness, &[rustc.clone(), link.clone()]);
    assert_eq!(summary.count, 2);
    assert!(!summary.compile_only);
    assert!(summary.build_stage);
    assert!(!summary_of(&harness, &[link]).compile_only);
    let analysis = json!({
        "id": {"targetCompleted": {"label": "//c:t"}},
        "aborted": {"reason": "ANALYSIS_FAILURE", "description": "Analysis of target '//c:t' failed"},
    })
    .to_string();
    let summary = summary_of(&harness, &[rustc, analysis]);
    assert_eq!(summary.count, 2);
    assert!(!summary.compile_only);
    assert!(!summary.build_stage);
}

#[test]
fn an_empty_or_cut_off_stream_is_not_compile_only() {
    let harness = Harness::new("bf-sum-cut");
    assert_eq!(summary_of(&harness, &[]), ReportedFailures::default());
    let rustc = compile_action(&harness, "//a:lib", "Rustc", RENDERED_E0308);
    let cut = summary_of(&harness, &[rustc, "{not json".to_owned()]);
    assert_eq!(cut.count, 1);
    assert!(!cut.compile_only);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let missing = report_build_failures(
        &harness.temp.join("absent.json"),
        dx_output::OutputMode::Json,
        &mut out,
        &mut err,
    );
    assert_eq!(missing, Ok(ReportedFailures::default()));
}
