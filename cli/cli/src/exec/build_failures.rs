use super::common::{emit_built, CODE_BUILD_EVENTS_INCOMPLETE};
use super::rustc_stderr::{self, ActionDiagnostic};
use dx_bep::{collect_build_failures, BuildFailure, FailureStage, StderrRef};
use dx_output::{
    build_failure_event, notice_event, BuildFailureEvent, FailureDiagnostic, FailureRelated,
    NoticeEvent, OutputMode,
};
use std::io::{BufReader, Read, Write};
use std::path::Path;

const DIAGNOSTIC_LIMIT: usize = 50;
const STDERR_LIMIT: u64 = 8 * 1024 * 1024;
const LINK_PREFIX: &str = "linking with";
const NOT_BUILT: &str = "target_not_built";

struct Diagnostics {
    status: &'static str,
    total: u64,
    items: Vec<ActionDiagnostic>,
}

fn read_stderr(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(STDERR_LIMIT + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > STDERR_LIMIT {
        return None;
    }
    Some(bytes)
}

fn rustc_diagnostics(failure: &BuildFailure) -> Diagnostics {
    let StderrRef::Local(path) = &failure.stderr else {
        return Diagnostics {
            status: "unavailable",
            total: 0,
            items: Vec::new(),
        };
    };
    let parsed = read_stderr(path).map(|bytes| rustc_stderr::parse(&bytes));
    match parsed {
        None => Diagnostics {
            status: "unavailable",
            total: 0,
            items: Vec::new(),
        },
        Some(items) if items.is_empty() => Diagnostics {
            status: "none",
            total: 0,
            items,
        },
        Some(items) => Diagnostics {
            status: "parsed",
            total: items.len() as u64,
            items,
        },
    }
}

pub(super) fn to_event(failure: &BuildFailure) -> BuildFailureEvent {
    let tool = match failure.mnemonic.as_deref() {
        Some("Clippy") => Some("clippy"),
        Some(mnemonic) if mnemonic.starts_with("Rustc") => Some("rustc"),
        _ => None,
    };
    let is_rustc = tool.is_some();
    let diagnostics = if is_rustc {
        rustc_diagnostics(failure)
    } else {
        Diagnostics {
            status: "unknown",
            total: 0,
            items: Vec::new(),
        }
    };
    let (mut stage, mut category) = (failure.stage, failure.category);
    if is_rustc
        && stage == FailureStage::Compilation
        && diagnostics
            .items
            .iter()
            .any(|item| item.message.starts_with(LINK_PREFIX))
    {
        stage = FailureStage::Linking;
        category = "link_failed";
    }
    BuildFailureEvent {
        target: failure.target.clone(),
        configuration: failure.configuration.clone(),
        target_kind: failure.target_kind.name().to_owned(),
        stage: stage.name().to_owned(),
        category: category.to_owned(),
        mnemonic: failure.mnemonic.clone(),
        primary_output: failure.primary_output.clone(),
        exit_code: failure.exit_code,
        tool: tool.map(str::to_owned),
        message: failure.message.clone(),
        diagnostics_status: diagnostics.status.to_owned(),
        diagnostics_total: diagnostics.total,
        diagnostics: diagnostics
            .items
            .into_iter()
            .take(DIAGNOSTIC_LIMIT)
            .map(|item| FailureDiagnostic {
                severity: item.severity.to_owned(),
                code: item.code,
                message: item.message,
                path: item.file,
                line: item.line,
                column: item.column,
                related: item
                    .related
                    .into_iter()
                    .map(|related| FailureRelated {
                        role: related.role.to_owned(),
                        message: related.message,
                        path: related.file,
                        line: related.line,
                        column: related.column,
                        replacement: related.replacement,
                        applicability: related.applicability,
                    })
                    .collect(),
            })
            .collect(),
        count: 1,
        targets: Vec::new(),
    }
}

const TARGETS_LISTED: usize = 20;

/// The facts that make two failures the same problem across targets and configurations.
fn identity(event: &BuildFailureEvent) -> String {
    let items: Vec<String> = event
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}",
                d.severity, d.code, d.message, d.path, d.line, d.column
            )
        })
        .collect();
    let owner = if event.diagnostics.is_empty() {
        event.target.clone()
    } else {
        None
    };
    format!(
        "{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{owner:?}|{items:?}",
        event.target_kind,
        event.stage,
        event.category,
        event.mnemonic,
        event.tool,
        event.message,
        event.exit_code,
        event.diagnostics_status,
    )
}

/// Merges identical failures into one event with a count and the other targets that hit it.
fn dedupe(events: Vec<BuildFailureEvent>) -> Vec<BuildFailureEvent> {
    let mut merged: Vec<BuildFailureEvent> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for event in events {
        let key = identity(&event);
        let Some(&at) = seen.get(&key) else {
            seen.insert(key, merged.len());
            merged.push(event);
            continue;
        };
        let first = &mut merged[at];
        first.count += 1;
        if let Some(target) = event.target {
            let known =
                first.target.as_deref() == Some(target.as_str()) || first.targets.contains(&target);
            if !known && first.targets.len() < TARGETS_LISTED {
                first.targets.push(target);
            }
        }
    }
    merged
}

fn describe(event: &BuildFailureEvent) -> String {
    let unknown = "unknown";
    let mut listed = vec![event.target.as_deref().unwrap_or(unknown)];
    listed.extend(event.targets.iter().map(String::as_str));
    let mut text = format!(
        "dx: build failure: {} ({}/{}",
        listed.join(", "),
        event.stage,
        event.category
    );
    if let Some(mnemonic) = &event.mnemonic {
        text.push_str(&format!(", {mnemonic}"));
    }
    text.push(')');
    if event.count > 1 {
        text.push_str(&format!(" x{}", event.count));
    }
    if let Some(message) = &event.message {
        text.push_str(&format!(": {message}"));
    }
    for diagnostic in &event.diagnostics {
        text.push_str(&format!("\n  {}", diagnostic.severity));
        if let Some(code) = &diagnostic.code {
            text.push_str(&format!("[{code}]"));
        }
        text.push_str(&format!(": {}", diagnostic.message));
        if let Some(path) = &diagnostic.path {
            text.push_str(&format!(" ({path}"));
            if let Some(line) = diagnostic.line {
                text.push_str(&format!(":{line}"));
                if let Some(column) = diagnostic.column {
                    text.push_str(&format!(":{column}"));
                }
            }
            text.push(')');
        }
        for related in &diagnostic.related {
            text.push_str(&format!("\n    {}: {}", related.role, related.message));
            if let Some(path) = &related.path {
                text.push_str(&format!(" ({path}"));
                if let Some(line) = related.line {
                    text.push_str(&format!(":{line}"));
                    if let Some(column) = related.column {
                        text.push_str(&format!(":{column}"));
                    }
                }
                text.push(')');
            }
            if let Some(replacement) = &related.replacement {
                text.push_str(&format!(" suggests `{replacement}`"));
                if let Some(applicability) = &related.applicability {
                    text.push_str(&format!(" [{applicability}]"));
                }
            }
        }
    }
    text
}

fn unavailable(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    detail: &str,
) -> Result<(), i32> {
    match output {
        OutputMode::Json => {
            let notice = NoticeEvent::warning(CODE_BUILD_EVENTS_INCOMPLETE, detail.to_owned());
            emit_built(out, notice_event(&notice))
        }
        OutputMode::Text { .. } => {
            let _ = writeln!(err, "dx: build_events_incomplete: {detail}");
            Ok(())
        }
        _ => Ok(()),
    }
}

/// What report_build_failures found in one event file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReportedFailures {
    pub(crate) count: usize,
    pub(crate) compile_only: bool,
    /// A non-analysis failure (compile, link, ...) was recorded and no analysis failure was.
    pub(crate) build_stage: bool,
}

/// Reports the failures Bazel recorded in the event file and summarizes them.
pub(crate) fn report_build_failures(
    bep: &Path,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<ReportedFailures, i32> {
    let file = match std::fs::File::open(bep) {
        Ok(file) => file,
        Err(error) => {
            unavailable(
                output,
                out,
                err,
                &format!("build failures are unknown: {error}"),
            )?;
            return Ok(ReportedFailures::default());
        }
    };
    let collected = collect_build_failures(BufReader::new(file));
    let events = dedupe(collected.failures.iter().map(to_event).collect());
    for event in &events {
        match output {
            OutputMode::Json => {
                emit_built(out, build_failure_event(event))?;
            }
            OutputMode::Text { .. } => {
                let _ = writeln!(err, "{}", describe(event));
            }
            _ => {}
        }
    }
    if let Some(problem) = &collected.problem {
        unavailable(
            output,
            out,
            err,
            &format!("build events end early, so later failures may be missing: {problem}"),
        )?;
    }
    let compiling = |event: &BuildFailureEvent| event.stage == FailureStage::Compilation.name();
    Ok(ReportedFailures {
        count: events.len(),
        build_stage: events
            .iter()
            .any(|event| event.stage != FailureStage::Analysis.name() && event.category != NOT_BUILT)
            && !events
                .iter()
                .any(|event| event.stage == FailureStage::Analysis.name()),
        compile_only: collected.problem.is_none()
            && events.iter().any(compiling)
            && events
                .iter()
                .all(|event| compiling(event) || event.category == NOT_BUILT),
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::{dedupe, BuildFailureEvent, FailureDiagnostic, FailureRelated};
    use serde_json::{json, Value};

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
    fn events_that_differ_only_in_related_still_dedupe() {
        let mut a = to_failure_event_for_test();
        let mut b = a.clone();
        a.diagnostics[0].related = Vec::new();
        b.diagnostics[0].related = vec![FailureRelated {
            role: "note".to_owned(),
            message: "n".to_owned(),
            path: None,
            line: None,
            column: None,
            replacement: None,
            applicability: None,
        }];
        let merged = dedupe(vec![a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].count, 2);
        assert!(merged[0].diagnostics[0].related.is_empty());
    }

    fn to_failure_event_for_test() -> BuildFailureEvent {
        BuildFailureEvent {
            target: Some("//a:lib".to_owned()),
            configuration: None,
            target_kind: "other".to_owned(),
            stage: "compilation".to_owned(),
            category: "compile_failed".to_owned(),
            mnemonic: Some("Rustc".to_owned()),
            primary_output: None,
            exit_code: Some(1),
            tool: Some("rustc".to_owned()),
            message: None,
            diagnostics_status: "parsed".to_owned(),
            diagnostics_total: 1,
            diagnostics: vec![FailureDiagnostic {
                severity: "error".to_owned(),
                code: None,
                message: "m".to_owned(),
                path: None,
                line: None,
                column: None,
                related: Vec::new(),
            }],
            count: 1,
            targets: Vec::new(),
        }
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
        assert!(failures_of(&out).is_empty());
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
        assert!(failures_of(&out).is_empty());
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
        assert!(failures_of(&out).is_empty());
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

    fn summary_of(harness: &Harness, lines: &[String]) -> super::ReportedFailures {
        let bep = harness.temp.join("summary.bep.json");
        std::fs::write(&bep, lines.join("\n")).expect("bep");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        super::report_build_failures(&bep, dx_output::OutputMode::Json, &mut out, &mut err)
            .expect("reported")
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
        assert_eq!(
            summary_of(&harness, &[]),
            super::ReportedFailures::default()
        );
        let rustc = compile_action(&harness, "//a:lib", "Rustc", RENDERED_E0308);
        let cut = summary_of(&harness, &[rustc, "{not json".to_owned()]);
        assert_eq!(cut.count, 1);
        assert!(!cut.compile_only);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let missing = super::report_build_failures(
            &harness.temp.join("absent.json"),
            dx_output::OutputMode::Json,
            &mut out,
            &mut err,
        );
        assert_eq!(missing, Ok(super::ReportedFailures::default()));
    }

    struct BrokenPipeWriter;

    impl std::io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn unavailable_returns_the_stdout_code_when_its_notice_cannot_land() {
        let mut err = Vec::new();
        assert_eq!(
            super::unavailable(
                dx_output::OutputMode::Json,
                &mut BrokenPipeWriter,
                &mut err,
                "gone",
            ),
            Err(128 + 13)
        );
        assert!(err.is_empty());
    }

    #[test]
    fn a_notice_that_fails_to_build_ends_the_stream_with_invalid_result() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = super::unavailable(dx_output::OutputMode::Json, &mut out, &mut err, "");
        assert_eq!(exit, Err(dx_process::operational_code()));
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0]["event"], "error");
        assert_eq!(lines[0]["code"], "invalid_result");
        assert_eq!(lines[1]["event"], "command_finished");
        assert_eq!(lines[1]["exit_code"], dx_process::operational_code());
    }
}
