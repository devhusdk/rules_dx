use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::PathBuf;

use serde_json::{Map, Value};

use super::file_uri_to_path;

const MESSAGE_LIMIT: usize = 1000;

/// The build stage a failure belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    Analysis,
    ToolAcquisition,
    Compilation,
    Linking,
    TestExecution,
    Other,
    Unknown,
}

impl FailureStage {
    pub fn name(self) -> &'static str {
        match self {
            FailureStage::Analysis => "analysis",
            FailureStage::ToolAcquisition => "tool_acquisition",
            FailureStage::Compilation => "compilation",
            FailureStage::Linking => "linking",
            FailureStage::TestExecution => "test_execution",
            FailureStage::Other => "other",
            FailureStage::Unknown => "unknown",
        }
    }
}

/// Whether a failed target is a test target, as Bazel configured it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedTargetKind {
    Test,
    Other,
    Unknown,
}

impl FailedTargetKind {
    pub fn name(self) -> &'static str {
        match self {
            FailedTargetKind::Test => "test",
            FailedTargetKind::Other => "other",
            FailedTargetKind::Unknown => "unknown",
        }
    }
}

/// Where a failed action's stderr can be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StderrRef {
    Local(PathBuf),
    Remote,
    Absent,
}

/// One failure Bazel reported while building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFailure {
    pub target: Option<String>,
    pub configuration: Option<String>,
    pub target_kind: FailedTargetKind,
    pub stage: FailureStage,
    pub category: &'static str,
    pub mnemonic: Option<String>,
    pub primary_output: Option<String>,
    pub exit_code: Option<i64>,
    pub stderr: StderrRef,
    pub message: Option<String>,
}

/// The failures of one build event stream and whether the stream was readable to its end.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildFailures {
    pub failures: Vec<BuildFailure>,
    pub problem: Option<String>,
}

struct ActionFailure {
    label: Option<String>,
    configuration: Option<String>,
    mnemonic: Option<String>,
    primary_output: Option<String>,
    exit_code: Option<i64>,
    stderr: StderrRef,
    detail_stage: Option<(FailureStage, &'static str)>,
}

struct TargetFailure {
    label: String,
    configuration: Option<String>,
    stage: FailureStage,
    category: &'static str,
    message: Option<String>,
}

fn configuration_of(entry: &Map<String, Value>) -> Option<String> {
    entry
        .get("configuration")
        .and_then(Value::as_object)
        .and_then(|configuration| configuration.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn string_of(entry: &Map<String, Value>, field: &str) -> Option<String> {
    entry
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn first_line(text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    Some(line.chars().take(MESSAGE_LIMIT).collect())
}

fn detail_stage(detail: &Value) -> Option<(FailureStage, &'static str)> {
    let detail = detail.as_object()?;
    if detail.contains_key("analysis") {
        return Some((FailureStage::Analysis, "analysis_failed"));
    }
    if detail.contains_key("packageLoading") {
        return Some((FailureStage::Analysis, "loading_failed"));
    }
    if detail.contains_key("externalRepository") || detail.contains_key("fetch") {
        return Some((FailureStage::ToolAcquisition, "tool_unavailable"));
    }
    let spawn_code = detail
        .get("spawn")
        .and_then(Value::as_object)
        .and_then(|spawn| spawn.get("code"))
        .and_then(Value::as_str);
    let missing_tool = detail
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("No such file or directory"));
    if spawn_code == Some("EXECUTION_FAILED") && missing_tool {
        return Some((FailureStage::ToolAcquisition, "tool_missing"));
    }
    None
}

fn stderr_ref(action: &Map<String, Value>) -> StderrRef {
    let Some(uri) = action
        .get("stderr")
        .and_then(Value::as_object)
        .and_then(|file| file.get("uri"))
        .and_then(Value::as_str)
    else {
        return StderrRef::Absent;
    };
    if super::is_bytestream_uri(uri) {
        return StderrRef::Remote;
    }
    match file_uri_to_path(uri) {
        Ok(path) => StderrRef::Local(path),
        Err(_) => StderrRef::Absent,
    }
}

fn exit_code_of(action: &Map<String, Value>) -> Option<i64> {
    match action.get("exitCode") {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

fn classify_mnemonic(mnemonic: Option<&str>) -> (FailureStage, &'static str) {
    match mnemonic {
        Some(name) if name.starts_with("TestRunner") => {
            (FailureStage::TestExecution, "test_execution_failed")
        }
        Some(name) if name.contains("Link") || name == "CppArchive" => {
            (FailureStage::Linking, "link_failed")
        }
        Some(name)
            if name.starts_with("Rustc")
                || name == "Clippy"
                || name.contains("Compile")
                || name == "Javac" =>
        {
            (FailureStage::Compilation, "compile_failed")
        }
        Some(_) => (FailureStage::Other, "action_failed"),
        None => (FailureStage::Unknown, "action_failed"),
    }
}

fn aborted_class(reason: Option<&str>) -> Option<(FailureStage, &'static str)> {
    match reason {
        Some("ANALYSIS_FAILURE") => Some((FailureStage::Analysis, "analysis_failed")),
        Some("LOADING_FAILURE") => Some((FailureStage::Analysis, "loading_failed")),
        Some("USER_INTERRUPTED") => Some((FailureStage::Unknown, "cancelled")),
        Some("SKIPPED") => Some((FailureStage::Unknown, "skipped")),
        Some("NO_BUILD" | "NO_ANALYZE") => None,
        _ => Some((FailureStage::Unknown, "aborted")),
    }
}

fn label_of<'a>(id: &'a Map<String, Value>, keys: &[&str]) -> Option<(&'a str, Option<String>)> {
    for key in keys {
        if let Some(entry) = id.get(*key).and_then(Value::as_object)
            && let Some(label) = entry.get("label").and_then(Value::as_str)
        {
            return Some((label, configuration_of(entry)));
        }
    }
    None
}

/// Reads the failed actions, aborted targets, and unbuilt targets of a build event stream.
///
/// A line that is not a JSON object ends the read and is named in `problem`.
pub fn collect_build_failures(reader: impl BufRead) -> BuildFailures {
    let mut actions: Vec<ActionFailure> = Vec::new();
    let mut targets: Vec<TargetFailure> = Vec::new();
    let mut kinds: BTreeMap<String, FailedTargetKind> = BTreeMap::new();
    let mut problem = None;
    for (index, line) in reader.lines().enumerate() {
        let line_no = index + 1;
        let text = match line {
            Ok(text) => text,
            Err(error) => {
                problem = Some(format!("line {line_no}: unreadable stream line: {error}"));
                break;
            }
        };
        if text.trim().is_empty() {
            continue;
        }
        let event: Value = match serde_json::from_str(&text) {
            Ok(event) => event,
            Err(error) => {
                problem = Some(format!("line {line_no}: invalid JSON: {error}"));
                break;
            }
        };
        let Some(object) = event.as_object() else {
            problem = Some(format!("line {line_no}: event must be a JSON object"));
            break;
        };
        let Some(id) = object.get("id").and_then(Value::as_object) else {
            continue;
        };
        if let Some(configured) = object.get("configured").and_then(Value::as_object)
            && let Some((label, _)) = label_of(id, &["targetConfigured"])
        {
            let is_test = configured
                .get("targetKind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.ends_with("_test rule"))
                || configured.contains_key("testSize");
            let kind = if is_test {
                FailedTargetKind::Test
            } else {
                FailedTargetKind::Other
            };
            kinds.insert(label.to_owned(), kind);
        }
        if let Some(action) = object.get("action").and_then(Value::as_object) {
            if action.get("success").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let identity = id.get("actionCompleted").and_then(Value::as_object);
            let label = string_of(action, "label")
                .or_else(|| identity.and_then(|entry| string_of(entry, "label")));
            actions.push(ActionFailure {
                label,
                configuration: configuration_of(action)
                    .or_else(|| identity.and_then(configuration_of)),
                mnemonic: string_of(action, "type"),
                primary_output: identity.and_then(|entry| string_of(entry, "primaryOutput")),
                exit_code: exit_code_of(action),
                stderr: stderr_ref(action),
                detail_stage: action.get("failureDetail").and_then(detail_stage),
            });
            continue;
        }
        if let Some(aborted) = object.get("aborted").and_then(Value::as_object) {
            let Some((label, configuration)) = label_of(
                id,
                &[
                    "targetCompleted",
                    "configuredLabel",
                    "unconfiguredLabel",
                    "targetConfigured",
                ],
            ) else {
                continue;
            };
            let class = aborted
                .get("failureDetail")
                .and_then(detail_stage)
                .or_else(|| aborted_class(aborted.get("reason").and_then(Value::as_str)));
            let Some((stage, category)) = class else {
                continue;
            };
            targets.push(TargetFailure {
                label: label.to_owned(),
                configuration,
                stage,
                category,
                message: aborted
                    .get("description")
                    .and_then(Value::as_str)
                    .and_then(first_line),
            });
            continue;
        }
        if let Some(completed) = object.get("completed").and_then(Value::as_object) {
            if completed.get("success").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some((label, configuration)) = label_of(id, &["targetCompleted"]) else {
                continue;
            };
            let detail = completed.get("failureDetail");
            let (stage, category) = detail
                .and_then(detail_stage)
                .unwrap_or((FailureStage::Unknown, "target_not_built"));
            targets.push(TargetFailure {
                label: label.to_owned(),
                configuration,
                stage,
                category,
                message: detail
                    .and_then(Value::as_object)
                    .filter(|_| stage != FailureStage::Unknown)
                    .and_then(|detail| detail.get("message"))
                    .and_then(Value::as_str)
                    .and_then(first_line),
            });
        }
    }
    let kind_of = |label: Option<&str>| {
        label
            .and_then(|label| kinds.get(label).copied())
            .unwrap_or(FailedTargetKind::Unknown)
    };
    let mut failures = Vec::new();
    let mut seen_actions = BTreeSet::new();
    let mut labels_with_action: BTreeSet<&str> = BTreeSet::new();
    for action in &actions {
        let key = (
            action.label.clone(),
            action.configuration.clone(),
            action.primary_output.clone(),
        );
        if !seen_actions.insert(key) {
            continue;
        }
        if let Some(label) = &action.label {
            labels_with_action.insert(label);
        }
        let (stage, category) = action
            .detail_stage
            .unwrap_or_else(|| classify_mnemonic(action.mnemonic.as_deref()));
        failures.push(BuildFailure {
            target: action.label.clone(),
            configuration: action.configuration.clone(),
            target_kind: kind_of(action.label.as_deref()),
            stage,
            category,
            mnemonic: action.mnemonic.clone(),
            primary_output: action.primary_output.clone(),
            exit_code: action.exit_code,
            stderr: action.stderr.clone(),
            message: None,
        });
    }
    let mut seen_targets = BTreeSet::new();
    for target in targets {
        let explicit = !matches!(target.category, "target_not_built" | "skipped" | "aborted");
        if labels_with_action.contains(target.label.as_str()) && !explicit {
            continue;
        }
        if !seen_targets.insert((target.label.clone(), target.configuration.clone())) {
            continue;
        }
        failures.push(BuildFailure {
            target_kind: kind_of(Some(&target.label)),
            target: Some(target.label),
            configuration: target.configuration,
            stage: target.stage,
            category: target.category,
            mnemonic: None,
            primary_output: None,
            exit_code: None,
            stderr: StderrRef::Absent,
            message: target.message,
        });
    }
    BuildFailures { failures, problem }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(lines: &[&str]) -> BuildFailures {
        collect_build_failures(lines.join("\n").as_bytes())
    }

    fn action_line(label: &str, config: &str, mnemonic: &str, output: &str) -> String {
        let root = if cfg!(windows) { "C:/out" } else { "out" };
        let stderr = format!("file:///{root}/stderr-1");
        format!(
            r#"{{"id":{{"actionCompleted":{{"primaryOutput":"{output}","label":"{label}","configuration":{{"id":"{config}"}}}}}},"action":{{"success":false,"type":"{mnemonic}","exitCode":1,"stderr":{{"name":"stderr","uri":"{stderr}"}},"label":"{label}","configuration":{{"id":"{config}"}}}}}}"#
        )
    }

    #[test]
    fn a_failed_action_carries_its_identity() {
        let line = action_line("//a:lib", "c1", "Rustc", "bazel-out/k8/bin/a/liblib.rlib");
        let found = collect(&[&line]);
        assert_eq!(found.problem, None);
        assert_eq!(found.failures.len(), 1);
        let failure = &found.failures[0];
        assert_eq!(failure.target.as_deref(), Some("//a:lib"));
        assert_eq!(failure.configuration.as_deref(), Some("c1"));
        assert_eq!(failure.stage, FailureStage::Compilation);
        assert_eq!(failure.category, "compile_failed");
        assert_eq!(failure.mnemonic.as_deref(), Some("Rustc"));
        assert_eq!(failure.exit_code, Some(1));
        assert!(matches!(failure.stderr, StderrRef::Local(_)));
    }

    #[test]
    fn clippy_and_pipelined_rustc_actions_are_compile_failures() {
        for mnemonic in ["Clippy", "RustcMetadata"] {
            let line = action_line("//a:lib", "c1", mnemonic, "out/a");
            let found = collect(&[&line]);
            assert_eq!(found.failures.len(), 1, "{mnemonic}");
            assert_eq!(
                found.failures[0].stage,
                FailureStage::Compilation,
                "{mnemonic}"
            );
            assert_eq!(found.failures[0].category, "compile_failed", "{mnemonic}");
        }
    }

    #[test]
    fn two_configurations_of_one_target_stay_two_failures() {
        let one = action_line("//a:lib", "c1", "Rustc", "out/a");
        let two = action_line("//a:lib", "c2", "Rustc", "out/a");
        let found = collect(&[&one, &two, &one]);
        assert_eq!(found.failures.len(), 2);
    }

    #[test]
    fn link_and_compile_mnemonics_are_distinct_stages() {
        let link = action_line("//a:bin", "c", "CppLink", "out/bin");
        let compile = action_line("//a:lib", "c", "CppCompile", "out/lib.o");
        let found = collect(&[&link, &compile]);
        assert_eq!(found.failures[0].stage, FailureStage::Linking);
        assert_eq!(found.failures[1].stage, FailureStage::Compilation);
    }

    #[test]
    fn analysis_failure_has_no_action_and_unknown_facts() {
        let line = r#"{"id":{"targetCompleted":{"label":"//a:t"}},"aborted":{"reason":"ANALYSIS_FAILURE","description":"Analysis of target '//a:t' failed\nsecond"}}"#;
        let found = collect(&[line]);
        assert_eq!(found.failures.len(), 1);
        let failure = &found.failures[0];
        assert_eq!(failure.stage, FailureStage::Analysis);
        assert_eq!(failure.mnemonic, None);
        assert_eq!(failure.configuration, None);
        assert_eq!(failure.stderr, StderrRef::Absent);
        assert_eq!(
            failure.message.as_deref(),
            Some("Analysis of target '//a:t' failed")
        );
    }

    #[test]
    fn a_failed_test_target_is_marked_as_a_test() {
        let configured = r#"{"id":{"targetConfigured":{"label":"//a:t"}},"configured":{"targetKind":"rust_test rule","testSize":"SMALL"}}"#;
        let completed = r#"{"id":{"targetCompleted":{"label":"//a:t","configuration":{"id":"c"}}},"completed":{"success":false}}"#;
        let found = collect(&[configured, completed]);
        assert_eq!(found.failures.len(), 1);
        assert_eq!(found.failures[0].target_kind, FailedTargetKind::Test);
        assert_eq!(found.failures[0].category, "target_not_built");
    }

    #[test]
    fn a_target_with_a_failed_action_is_not_reported_twice() {
        let action = action_line("//a:t", "c", "Rustc", "out/t");
        let completed = r#"{"id":{"targetCompleted":{"label":"//a:t","configuration":{"id":"c"}}},"completed":{"success":false}}"#;
        let found = collect(&[&action, completed]);
        assert_eq!(found.failures.len(), 1);
    }

    #[test]
    fn non_ascii_paths_survive() {
        let line = action_line("//ü:ß", "c", "Rustc", "bazel-out/ü/日本.rlib");
        let found = collect(&[&line]);
        assert_eq!(
            found.failures[0].primary_output.as_deref(),
            Some("bazel-out/ü/日本.rlib")
        );
        assert_eq!(found.failures[0].target.as_deref(), Some("//ü:ß"));
    }

    #[test]
    fn a_truncated_stream_keeps_earlier_failures_and_names_the_line() {
        let line = action_line("//a:lib", "c", "Rustc", "out/a");
        let found = collect(&[&line, r#"{"id":{"actionComp"#]);
        assert_eq!(found.failures.len(), 1);
        let problem = found.problem.expect("problem");
        assert!(problem.starts_with("line 2:"), "{problem}");
    }

    #[test]
    fn a_non_object_event_is_a_problem() {
        let found = collect(&["[1]"]);
        assert!(found.failures.is_empty());
        assert!(found.problem.is_some());
    }

    #[test]
    fn a_missing_tool_is_tool_acquisition() {
        let line = r#"{"id":{"actionCompleted":{"primaryOutput":"o","label":"//a:b"}},"action":{"success":false,"type":"Rustc","failureDetail":{"message":"exec: No such file or directory","spawn":{"code":"EXECUTION_FAILED"}}}}"#;
        let found = collect(&[line]);
        assert_eq!(found.failures[0].stage, FailureStage::ToolAcquisition);
        assert_eq!(found.failures[0].category, "tool_missing");
        assert_eq!(found.failures[0].message, None);
    }

    #[test]
    fn remote_stderr_is_not_a_local_path() {
        let line = r#"{"id":{"actionCompleted":{"primaryOutput":"o","label":"//a:b"}},"action":{"success":false,"type":"Rustc","stderr":{"uri":"bytestream://host/blobs/1/2"}}}"#;
        let found = collect(&[line]);
        assert_eq!(found.failures[0].stderr, StderrRef::Remote);
    }

    #[test]
    fn cancellation_is_its_own_category() {
        let line = r#"{"id":{"targetCompleted":{"label":"//a:t"}},"aborted":{"reason":"USER_INTERRUPTED"}}"#;
        let found = collect(&[line]);
        assert_eq!(found.failures[0].category, "cancelled");
    }

    #[test]
    fn successful_actions_and_targets_report_nothing() {
        let ok = r#"{"id":{"targetCompleted":{"label":"//a:t"}},"completed":{"success":true}}"#;
        let action = r#"{"id":{"actionCompleted":{"label":"//a:t"}},"action":{"success":true,"type":"Rustc"}}"#;
        assert!(collect(&[ok, action]).failures.is_empty());
    }

    #[test]
    fn stage_and_kind_names_are_stable() {
        let stages = [
            (FailureStage::Analysis, "analysis"),
            (FailureStage::ToolAcquisition, "tool_acquisition"),
            (FailureStage::Compilation, "compilation"),
            (FailureStage::Linking, "linking"),
            (FailureStage::TestExecution, "test_execution"),
            (FailureStage::Other, "other"),
            (FailureStage::Unknown, "unknown"),
        ];
        for (stage, name) in stages {
            assert_eq!(stage.name(), name);
        }
        let kinds = [
            (FailedTargetKind::Test, "test"),
            (FailedTargetKind::Other, "other"),
            (FailedTargetKind::Unknown, "unknown"),
        ];
        for (kind, name) in kinds {
            assert_eq!(kind.name(), name);
        }
    }

    fn action_with(extra: &str) -> String {
        format!(
            r#"{{"id":{{"actionCompleted":{{"primaryOutput":"o","label":"//a:b"}}}},"action":{{"success":false,{extra}}}}}"#
        )
    }

    #[test]
    fn failure_details_classify_the_stage() {
        let cases = [
            (r#""failureDetail":{"analysis":{}}"#, "analysis_failed"),
            (r#""failureDetail":{"packageLoading":{}}"#, "loading_failed"),
            (r#""failureDetail":{"fetch":{}}"#, "tool_unavailable"),
            (
                r#""failureDetail":{"externalRepository":{}}"#,
                "tool_unavailable",
            ),
            (
                r#""failureDetail":"text","type":"Genrule""#,
                "action_failed",
            ),
            (
                r#""failureDetail":{"spawn":{}},"type":"Genrule""#,
                "action_failed",
            ),
        ];
        for (extra, category) in cases {
            let found = collect(&[&action_with(extra)]);
            assert_eq!(found.failures[0].category, category, "{extra}");
        }
    }

    #[test]
    fn mnemonics_pick_the_stage() {
        let cases = [
            (r#""type":"TestRunner""#, FailureStage::TestExecution),
            (r#""type":"CppArchive""#, FailureStage::Linking),
            (r#""type":"Javac""#, FailureStage::Compilation),
            (r#""type":"Genrule""#, FailureStage::Other),
            (r#""exitCode":2"#, FailureStage::Unknown),
        ];
        for (extra, stage) in cases {
            let found = collect(&[&action_with(extra)]);
            assert_eq!(found.failures[0].stage, stage, "{extra}");
        }
    }

    #[test]
    fn exit_codes_and_stderr_uris_are_read_leniently() {
        let text = collect(&[&action_with(r#""exitCode":"7""#)]);
        assert_eq!(text.failures[0].exit_code, Some(7));
        let junk = collect(&[&action_with(r#""exitCode":true"#)]);
        assert_eq!(junk.failures[0].exit_code, None);
        let bad_uri = collect(&[&action_with(r#""stderr":{"uri":"https://host/x"}"#)]);
        assert_eq!(bad_uri.failures[0].stderr, StderrRef::Absent);
    }

    #[test]
    fn aborted_targets_are_classified_by_reason() {
        let aborted = |reason: &str| {
            format!(
                r#"{{"id":{{"targetCompleted":{{"label":"//a:t"}}}},"aborted":{{"reason":"{reason}"}}}}"#
            )
        };
        let cases = [
            ("LOADING_FAILURE", Some("loading_failed")),
            ("SKIPPED", Some("skipped")),
            ("OTHER", Some("aborted")),
            ("NO_BUILD", None),
            ("NO_ANALYZE", None),
        ];
        for (reason, category) in cases {
            let found = collect(&[&aborted(reason)]);
            assert_eq!(
                found.failures.first().map(|failure| failure.category),
                category,
                "{reason}"
            );
        }
    }

    #[test]
    fn unreadable_blank_and_idless_lines_are_handled() {
        let mut bytes = b"\n{\"note\":1}\n".to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe, b'\n']);
        let found = collect_build_failures(bytes.as_slice());
        assert!(found.failures.is_empty());
        let problem = found.problem.expect("problem");
        assert!(problem.starts_with("line 3: unreadable"), "{problem}");
    }

    #[test]
    fn configured_kinds_label_failed_targets() {
        let other = r#"{"id":{"targetConfigured":{"label":"//a:lib"}},"configured":{"targetKind":"rust_library rule"}}"#;
        let kind = r#"{"id":{"targetConfigured":{"label":"//a:t"}},"configured":{"targetKind":"rust_test rule"}}"#;
        let failed = |label: &str| {
            format!(
                r#"{{"id":{{"targetCompleted":{{"label":"{label}"}}}},"completed":{{"success":false}}}}"#
            )
        };
        let found = collect(&[other, kind, &failed("//a:lib"), &failed("//a:t")]);
        assert_eq!(found.failures[0].target_kind, FailedTargetKind::Other);
        assert_eq!(found.failures[1].target_kind, FailedTargetKind::Test);
    }

    #[test]
    fn events_without_labels_or_classes_are_dropped() {
        let nameless_abort = r#"{"id":{"progress":{}},"aborted":{"reason":"SKIPPED"}}"#;
        let silent_abort =
            r#"{"id":{"targetCompleted":{"label":"//a:t"}},"aborted":{"reason":"NO_BUILD"}}"#;
        let nameless_completion = r#"{"id":{"progress":{}},"completed":{"success":false}}"#;
        let found = collect(&[nameless_abort, silent_abort, nameless_completion]);
        assert!(found.failures.is_empty());
    }

    #[test]
    fn a_target_failure_repeats_once() {
        let failed =
            r#"{"id":{"targetCompleted":{"label":"//a:t"}},"completed":{"success":false}}"#;
        assert_eq!(collect(&[failed, failed]).failures.len(), 1);
    }

    #[test]
    fn identity_less_actions_fall_back_to_the_event_id() {
        let line = r#"{"id":{"actionCompleted":{"label":"//a:b","configuration":{"id":"c9"}}},"action":{"success":false,"type":"Rustc"}}"#;
        let found = collect(&[line]);
        assert_eq!(found.failures[0].target.as_deref(), Some("//a:b"));
        assert_eq!(found.failures[0].configuration.as_deref(), Some("c9"));
    }

    #[test]
    fn odd_ids_and_descriptions_leave_no_message() {
        let blank = r#"{"id":{"targetCompleted":{},"configuredLabel":{"label":"//a:t"}},"aborted":{"reason":"SKIPPED","description":"  \n"}}"#;
        let found = collect(&[blank]);
        assert_eq!(found.failures[0].message, None);
        let other_id = r#"{"id":{"progress":{}},"configured":{"targetKind":"rust_test rule"}}"#;
        assert!(collect(&[other_id]).failures.is_empty());
    }
}
