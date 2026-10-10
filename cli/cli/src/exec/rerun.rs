use super::common::*;
use super::execute;
use crate::args::{Command, Invocation};
use dx_output::{
    command_finished, command_started, error_event, notice_event, with_correlation, write_event,
    FinishedCounts, NoticeEvent, OutputMode, TestOutcome,
};
use std::io::Write;
use std::path::Path;

pub(crate) const CODE_RERUN_RECEIPT_INVALID: &str = "rerun_receipt_invalid";
pub(crate) const CODE_RERUN_RECEIPT_UNSUPPORTED: &str = "rerun_receipt_unsupported";
pub(crate) const CODE_RERUN_RECEIPT_INCOMPLETE: &str = "rerun_receipt_incomplete";
pub(crate) const CODE_NOTHING_TO_RERUN: &str = "nothing_to_rerun";
pub(crate) const CODE_RERUN_WORKSPACE_CHANGED: &str = "rerun_workspace_changed";
pub(crate) const CODE_RERUN_INPUTS_CHANGED: &str = "rerun_inputs_changed";
pub(crate) const CODE_RERUN_OPTIONS_WITHHELD: &str = "rerun_options_withheld";
pub(crate) const CODE_RERUN_TOOL_CHANGED: &str = "rerun_tool_changed";

pub(crate) const RECEIPT_VERSION: u64 = 1;
pub(crate) const RECEIPT_FILE: &str = "receipt.json";

/// Option names whose values never reach a receipt: the value after `=` is
/// dropped, and a following non-flag word is dropped as its value.
fn secret_option_names() -> &'static [&'static str] {
    &[
        "password",
        "passwd",
        "token",
        "secret",
        "apikey",
        "api_key",
        "credential",
        "auth",
        "private_key",
        "oauth",
    ]
}

fn option_name(option: &str) -> &str {
    let bare = option.strip_prefix("--").unwrap_or(option);
    bare.split('=').next().unwrap_or(bare)
}

/// Splits Bazel options into replayable options and withheld secret names.
///
/// A `--name=value` option whose name is secret-bearing keeps its name in the
/// withheld list and is dropped. A bare secret-bearing flag also withholds its
/// value when the next word is not another flag. Everything else replays.
pub(crate) fn partition_options(options: &[String]) -> (Vec<String>, Vec<String>) {
    let mut safe = Vec::new();
    let mut withheld = Vec::new();
    let mut skip_value = false;
    for option in options {
        if skip_value {
            skip_value = false;
            if option.starts_with('-') {
                safe.push(option.clone());
            } else {
                withheld.push("(value)".to_owned());
            }
            continue;
        }
        let name = option_name(option).to_lowercase();
        let secret = secret_option_names()
            .iter()
            .any(|secret| name == *secret || name.ends_with(&format!("_{secret}")));
        if !secret {
            safe.push(option.clone());
            continue;
        }
        withheld.push(format!("--{}", option_name(option)));
        if !option.contains('=') {
            skip_value = true;
        }
    }
    (safe, withheld)
}

/// Best-effort workspace identity: the recorded HEAD and dirty flag, or
/// nothing when git cannot answer. Identity stays partial by design: it never
/// snapshots sources.
fn git_identity(workspace: &Path) -> (Option<String>, Option<bool>) {
    let head = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|head| !head.is_empty());
    let dirty = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .arg("status")
        .arg("--porcelain=v1")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.iter().all(|byte| byte.is_ascii_whitespace()));
    (head, dirty)
}

fn workspace_pin(workspace: &Path) -> Option<String> {
    dx_adopt::read_version_pin(workspace).ok()
}

fn receipt_outcome(outcome: &TestOutcome) -> serde_json::Value {
    let mut entry = serde_json::Map::new();
    entry.insert(
        "target".to_owned(),
        serde_json::Value::String(outcome.target.clone()),
    );
    entry.insert(
        "outcome".to_owned(),
        serde_json::Value::String(outcome.outcome.clone()),
    );
    if let Some(status) = outcome.status.as_deref() {
        entry.insert(
            "status".to_owned(),
            serde_json::Value::String(status.to_owned()),
        );
    }
    for (field, value) in [
        ("run", outcome.run.map(u64::from)),
        ("shard", outcome.shard.map(u64::from)),
        ("attempt", outcome.attempt.map(u64::from)),
    ] {
        if let Some(value) = value {
            entry.insert(field.to_owned(), serde_json::Value::from(value));
        }
    }
    entry.insert(
        "evidence_complete".to_owned(),
        serde_json::Value::Bool(outcome.evidence_complete),
    );
    serde_json::Value::Object(entry)
}

/// True for a recorded outcome that needs no rerun: an ordinary pass or a
/// pass after retry. Every other token reruns at the target level.
fn outcome_passed(outcome: &str) -> bool {
    outcome == "passed" || outcome == "passed_after_retry"
}

pub(crate) fn failed_targets(outcomes: &[TestOutcome]) -> Vec<String> {
    let mut failed: Vec<String> = outcomes
        .iter()
        .filter(|outcome| !outcome_passed(&outcome.outcome))
        .map(|outcome| outcome.target.clone())
        .collect();
    failed.sort();
    failed.dedup();
    failed
}

/// Writes `receipt.json` beside the run-output manifest: the request and
/// selection, the workspace and input identity, the safe configured options,
/// the observed tools, the per-target outcomes, and the manifest reference.
/// Secret-bearing option values are withheld, never persisted.
pub(crate) fn write_receipt(
    child: &Path,
    invocation: &Invocation,
    workspace: &Path,
    verb: crate::plan::WorkflowVerb,
    bazel_code: i32,
    results_complete: bool,
    outcomes: &[TestOutcome],
) -> Result<std::path::PathBuf, String> {
    let (safe_options, withheld_options) = partition_options(&invocation.bazel_options);
    let (safe_startup, withheld_startup) = partition_options(&invocation.bazel_startup_options);
    let mut withheld = withheld_options;
    withheld.extend(withheld_startup);
    withheld.sort();
    withheld.dedup();
    let (git_head, git_dirty) = git_identity(workspace);
    let mut workspace_value = serde_json::Map::new();
    workspace_value.insert(
        "path".to_owned(),
        serde_json::Value::String(workspace.display().to_string()),
    );
    workspace_value.insert(
        "git_head".to_owned(),
        git_head.map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    workspace_value.insert(
        "git_dirty".to_owned(),
        git_dirty.map_or(serde_json::Value::Null, serde_json::Value::Bool),
    );
    workspace_value.insert(
        "identity".to_owned(),
        serde_json::Value::String("partial".to_owned()),
    );
    let mut tools = serde_json::Map::new();
    tools.insert(
        "dx_pin".to_owned(),
        workspace_pin(workspace).map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    let receipt = serde_json::json!({
        "version": RECEIPT_VERSION,
        "command": verb.name(),
        "scopes": invocation.targets,
        "profile": if verb == crate::plan::WorkflowVerb::Coverage { serde_json::Value::Null } else { serde_json::Value::String(invocation.profile().config().to_owned()) },
        "strict_evidence": invocation.strict_evidence,
        "min_coverage": invocation.min_coverage.map_or(serde_json::Value::Null, serde_json::Value::from),
        "bazel_options": safe_options,
        "bazel_startup_options": safe_startup,
        "withheld_options": withheld,
        "workspace": workspace_value,
        "tools": tools,
        "outcomes": outcomes.iter().map(receipt_outcome).collect::<Vec<_>>(),
        "failed_targets": failed_targets(outcomes),
        "validation_performed": true,
        "bazel_exit": bazel_code,
        "results_complete": results_complete,
        "manifest": "manifest.json",
    });
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("cannot encode run-output receipt: {err}"))?;
    let receipt_path = child.join(RECEIPT_FILE);
    std::fs::write(&receipt_path, format!("{text}\n"))
        .map_err(|err| format!("cannot write {}: {err}", receipt_path.display()))?;
    Ok(receipt_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) command: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) profile: Option<String>,
    pub(crate) strict_evidence: bool,
    pub(crate) bazel_options: Vec<String>,
    pub(crate) withheld_options: Vec<String>,
    pub(crate) workspace_path: String,
    pub(crate) git_head: Option<String>,
    pub(crate) dx_pin: Option<String>,
    pub(crate) failed_targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReceiptError {
    Invalid(String),
    Unsupported(String),
    Incomplete(String),
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReceiptError::Invalid(message) => write!(formatter, "{message}"),
            ReceiptError::Unsupported(message) => write!(formatter, "{message}"),
            ReceiptError::Incomplete(message) => write!(formatter, "{message}"),
        }
    }
}

fn receipt_string(value: &serde_json::Value, field: &str) -> Result<String, ReceiptError> {
    value[field]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| ReceiptError::Invalid(format!("receipt has no string field {field:?}")))
}

fn receipt_strings(value: &serde_json::Value, field: &str) -> Result<Vec<String>, ReceiptError> {
    value[field]
        .as_array()
        .ok_or_else(|| ReceiptError::Invalid(format!("receipt has no array field {field:?}")))?
        .iter()
        .map(|entry| {
            entry.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                ReceiptError::Invalid(format!("receipt field {field:?} holds a non-string entry"))
            })
        })
        .collect()
}

/// Reads and validates one run-output receipt. A receipt describes an
/// observed run; rerunning it is new evidence, never a historical
/// reproduction.
pub(crate) fn read_receipt(path: &Path) -> Result<Receipt, ReceiptError> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        ReceiptError::Invalid(format!("cannot read receipt {}: {err}", path.display()))
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        ReceiptError::Invalid(format!("receipt {} is not JSON: {err}", path.display()))
    })?;
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(RECEIPT_VERSION) {
        return Err(ReceiptError::Unsupported(format!(
            "receipt {} has an unsupported version (want {RECEIPT_VERSION})",
            path.display()
        )));
    }
    let command = receipt_string(&value, "command")?;
    if command != "test" && command != "coverage" {
        return Err(ReceiptError::Invalid(format!(
            "receipt {} records command {command:?} (want \"test\" or \"coverage\")",
            path.display()
        )));
    }
    if value
        .get("validation_performed")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(ReceiptError::Incomplete(format!(
            "receipt {} records an incomplete run with no outcomes to rerun",
            path.display()
        )));
    }
    Ok(Receipt {
        command,
        scopes: receipt_strings(&value, "scopes").unwrap_or_default(),
        profile: value["profile"].as_str().map(ToOwned::to_owned),
        strict_evidence: value
            .get("strict_evidence")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        bazel_options: receipt_strings(&value, "bazel_options")?,
        withheld_options: receipt_strings(&value, "withheld_options").unwrap_or_default(),
        workspace_path: value["workspace"]["path"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        git_head: value["workspace"]["git_head"]
            .as_str()
            .map(ToOwned::to_owned),
        dx_pin: value["tools"]["dx_pin"].as_str().map(ToOwned::to_owned),
        failed_targets: receipt_strings(&value, "failed_targets")?,
    })
}

fn receipt_path(workspace: &Path, raw: &str) -> std::path::PathBuf {
    let rel = Path::new(raw);
    if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        workspace.join(rel)
    }
}

fn notice(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    code: &str,
    level: &str,
    message: &str,
) {
    if output == OutputMode::Json {
        let notice = NoticeEvent {
            level: level.to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
            related_command: Some("rerun".to_owned()),
            scope: None,
            path: None,
            language: None,
            import: None,
        };
        if let Ok(event) = notice_event(&notice) {
            let correlated = with_correlation(event.clone(), "rerun/receipt").unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    } else {
        let _ = writeln!(err, "dx: {code}: {message}");
    }
}

fn receipt_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    error: &ReceiptError,
) -> i32 {
    let (code, exit) = match error {
        ReceiptError::Invalid(_) => (CODE_RERUN_RECEIPT_INVALID, 2),
        ReceiptError::Unsupported(_) => (CODE_RERUN_RECEIPT_UNSUPPORTED, 2),
        ReceiptError::Incomplete(_) => (CODE_RERUN_RECEIPT_INCOMPLETE, 1),
    };
    let message = error.to_string();
    let _ = writeln!(err, "dx: {code}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(code, &message, None, None, None) {
            let _ = write_event(out, &event);
        }
        let _ = write_event(
            out,
            &command_finished(
                exit,
                &FinishedCounts {
                    results_complete: Some(false),
                    ..FinishedCounts::default()
                },
            ),
        );
    }
    exit
}

fn rerun_invocation(invocation: &Invocation, receipt: &Receipt) -> Invocation {
    let mut bazel_options = receipt.bazel_options.clone();
    bazel_options.extend(invocation.bazel_options.iter().cloned());
    let (debug, release) = match receipt.profile.as_deref() {
        Some("dx_debug") => (true, false),
        Some("dx_release") => (false, true),
        _ => (false, false),
    };
    Invocation {
        command: Command::Test,
        check: false,
        strict_evidence: receipt.strict_evidence,
        run_output: None,
        apply: false,
        debug,
        release,
        workspace: invocation.workspace.clone(),
        dry_run: invocation.dry_run,
        quiet: invocation.quiet,
        verbose: invocation.verbose,
        log_level: invocation.log_level,
        color: invocation.color,
        output: invocation.output,
        reports: Vec::new(),
        fail_on: invocation.fail_on,
        min_coverage: None,
            baseline: None,
        targets: receipt.failed_targets.clone(),
        bazel_options,
        bazel_clean: false,
        prune_unobserved: false,
        pin: None,
        rollback: false,
        configured: false,
        from: None,
        to: None,
        here: false,
        serve: false,
        port: None,
        host: None,
        open: false,
        offline: false,
        frozen: false,
        workspace_capabilities: false,
        cases: false,
        bazel_startup_options: invocation.bazel_startup_options.clone(),
    }
}

pub(crate) fn execute_rerun(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid: _,
        nonce,
        out,
        err,
        ci,
    } = env;
    if invocation.output == OutputMode::Json {
        let mode = if invocation.check { "check" } else { "default" };
        if let Ok(event) = command_started("rerun", invocation.dry_run, mode) {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
    }
    let raw = invocation.targets.first().cloned().unwrap_or_default();
    let path = receipt_path(workspace, &raw);
    let receipt = match read_receipt(&path) {
        Ok(receipt) => receipt,
        Err(error) => return receipt_failure(out, err, invocation.output, &error),
    };
    if receipt.failed_targets.is_empty() {
        let message = format!(
            "receipt {} records no failed test targets; nothing to rerun",
            path.display()
        );
        if invocation.output == OutputMode::Json {
            notice(
                out,
                err,
                invocation.output,
                CODE_NOTHING_TO_RERUN,
                "info",
                &message,
            );
            let _ = write_event(
                out,
                &command_finished(
                    0,
                    &FinishedCounts {
                        results_complete: Some(false),
                        ..FinishedCounts::default()
                    },
                ),
            );
        } else if invocation.chatty() {
            let _ = writeln!(
                out,
                "Rerun {}: no failed test targets; nothing to rerun.",
                path.display()
            );
        }
        return 0;
    }
    let current_workspace = workspace.display().to_string();
    if !receipt.workspace_path.is_empty() && receipt.workspace_path != current_workspace {
        notice(
            out,
            err,
            invocation.output,
            CODE_RERUN_WORKSPACE_CHANGED,
            "warning",
            &format!(
                "receipt records workspace {}; rerunning on {current_workspace} as new evidence, not a reproduction",
                receipt.workspace_path
            ),
        );
    }
    let (current_head, _) = git_identity(workspace);
    if let (Some(recorded), Some(current)) = (receipt.git_head.as_deref(), current_head.as_deref())
    {
        if recorded != current {
            notice(
                out,
                err,
                invocation.output,
                CODE_RERUN_INPUTS_CHANGED,
                "warning",
                &format!(
                    "workspace inputs changed since the recorded run ({recorded} now {current}); this rerun is new evidence"
                ),
            );
        }
    }
    if !receipt.withheld_options.is_empty() {
        notice(
            out,
            err,
            invocation.output,
            CODE_RERUN_OPTIONS_WITHHELD,
            "warning",
            &format!(
                "receipt withholds redacted options ({}); resupply them after `--` if the rerun needs them",
                receipt.withheld_options.join(", ")
            ),
        );
    }
    if let (Some(recorded), Some(current)) = (
        receipt.dx_pin.as_deref(),
        workspace_pin(workspace).as_deref(),
    ) {
        if recorded != current {
            notice(
                out,
                err,
                invocation.output,
                CODE_RERUN_TOOL_CHANGED,
                "warning",
                &format!(
                    "dx pin changed since the recorded run ({recorded} now {current}); this rerun is new evidence"
                ),
            );
        }
    }
    let test_invocation = rerun_invocation(invocation, &receipt);
    let mut rerun_out = Vec::new();
    let mut rerun_err = Vec::new();
    let code = {
        let test_env = Env {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid: std::process::id(),
            nonce,
            out: &mut rerun_out,
            err: &mut rerun_err,
            ci,
        };
        execute(&test_invocation, test_env)
    };
    if check_stdout_write(out.write_all(&rerun_out))
        .and_then(|()| check_stdout_write(err.write_all(&rerun_err)))
        .is_err()
    {
        return dx_process::operational_code();
    }
    if invocation.output == OutputMode::Json {
        let _ = write_event(
            out,
            &command_finished(
                code,
                &FinishedCounts {
                    results_complete: Some(code == 0),
                    ..FinishedCounts::default()
                },
            ),
        );
    }
    code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use dx_process::{ChildStatus, Runner};
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;

    struct Recording {
        codes: RefCell<Vec<Option<i32>>>,
        seen: Rc<RefCell<Vec<Vec<String>>>>,
        bep_lines: Vec<String>,
    }

    impl Recording {
        fn run_with(
            harness: &Harness,
            inv: &Invocation,
            codes: Vec<Option<i32>>,
            bep_lines: Vec<String>,
        ) -> RecordedRun {
            let runner = Recording {
                codes: RefCell::new(codes),
                seen: Rc::new(RefCell::new(Vec::new())),
                bep_lines,
            };
            let (code, out, err) = harness.execute_with(inv, &runner);
            let argv = runner.seen.borrow().clone();
            RecordedRun {
                code,
                out,
                err,
                argv,
            }
        }
    }

    struct RecordedRun {
        code: i32,
        out: String,
        err: String,
        argv: Vec<Vec<String>>,
    }

    impl Runner for Recording {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let launch = self.seen.borrow().len();
            self.seen.borrow_mut().push(argv.to_vec());
            if let Some(bep) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
            {
                std::fs::write(bep, self.bep_lines.join("\n")).expect("BEP file");
            }
            let codes = self.codes.borrow();
            let code = if codes.is_empty() {
                Some(0)
            } else if launch < codes.len() {
                codes[launch]
            } else {
                codes[codes.len() - 1]
            };
            Ok(ChildStatus { code })
        }
    }

    fn passing_bep(harness: &Harness, label: &str) -> Vec<String> {
        let xml = write_bep_artifact(harness, "pass.xml", MINIMAL_XML.as_bytes());
        vec![test_result_line(label, &[(String::from("test.xml"), xml)])]
    }

    fn failing_bep(harness: &Harness, label: &str) -> Vec<String> {
        let xml = write_bep_artifact(harness, "fail.xml", MINIMAL_XML.as_bytes());
        vec![test_result_full_line(
            label,
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), xml)],
        )]
    }

    fn base_receipt(failed: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "command": "test",
            "scopes": ["//..."],
            "profile": "dx_dev",
            "strict_evidence": false,
            "bazel_options": ["--jobs=2"],
            "bazel_startup_options": [],
            "withheld_options": [],
            "workspace": {
                "path": "/elsewhere",
                "git_head": serde_json::Value::Null,
                "git_dirty": serde_json::Value::Null,
                "identity": "partial",
            },
            "tools": {"dx_pin": serde_json::Value::Null},
            "outcomes": [],
            "failed_targets": failed,
            "validation_performed": true,
            "bazel_exit": 1,
            "results_complete": false,
            "manifest": "manifest.json",
        })
    }

    fn write_receipt(harness: &Harness, name: &str, body: &serde_json::Value) -> String {
        let rel = format!("receipts/{name}");
        harness.write_source(&rel, &serde_json::to_string_pretty(body).expect("receipt"));
        rel
    }

    #[test]
    fn partition_options_redacts_secret_values() {
        let (safe, withheld) = partition_options(&[
            "--jobs=2".to_owned(),
            "--token=abc".to_owned(),
            "--test_arg=--exact".to_owned(),
        ]);
        assert_eq!(
            safe,
            vec!["--jobs=2".to_owned(), "--test_arg=--exact".to_owned()]
        );
        assert_eq!(withheld, vec!["--token".to_owned()]);
    }

    #[test]
    fn partition_options_withholds_bare_flag_values() {
        let (safe, withheld) = partition_options(&[
            "--password".to_owned(),
            "hunter2".to_owned(),
            "--jobs=2".to_owned(),
            "--auth".to_owned(),
        ]);
        assert_eq!(safe, vec!["--jobs=2".to_owned()]);
        assert_eq!(
            withheld,
            vec![
                "--password".to_owned(),
                "(value)".to_owned(),
                "--auth".to_owned()
            ]
        );
    }

    #[test]
    fn failed_targets_keep_only_non_passes() {
        let outcomes = [
            "passed",
            "passed_after_retry",
            "failed",
            "timeout",
            "unknown",
        ]
        .iter()
        .enumerate()
        .map(|(index, outcome)| TestOutcome {
            target: format!("//a:t{index}"),
            outcome: (*outcome).to_owned(),
            ..TestOutcome::default()
        })
        .collect::<Vec<_>>();
        assert_eq!(
            failed_targets(&outcomes),
            vec![
                "//a:t2".to_owned(),
                "//a:t3".to_owned(),
                "//a:t4".to_owned()
            ]
        );
    }

    #[test]
    fn rerun_rejects_missing_receipt() {
        let harness = Harness::new("rerun-missing");
        let (code, _, err) = harness.run(&["rerun", "receipts/gone.json"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains(CODE_RERUN_RECEIPT_INVALID), "{err}");
    }

    #[test]
    fn rerun_rejects_corrupt_receipt() {
        let harness = Harness::new("rerun-corrupt");
        harness.write_source("receipts/bad.json", "not json {");
        let (code, _, err) = harness.run(&["rerun", "receipts/bad.json"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains(CODE_RERUN_RECEIPT_INVALID), "{err}");
    }

    #[test]
    fn rerun_rejects_unsupported_version() {
        let harness = Harness::new("rerun-version");
        let mut body = base_receipt(&["//a:bad"]);
        body["version"] = serde_json::json!(2);
        let rel = write_receipt(&harness, "v2.json", &body);
        let (code, _, err) = harness.run(&["rerun", rel.as_str()]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains(CODE_RERUN_RECEIPT_UNSUPPORTED), "{err}");
    }

    #[test]
    fn rerun_rejects_foreign_command() {
        let harness = Harness::new("rerun-command");
        let mut body = base_receipt(&["//a:bad"]);
        body["command"] = serde_json::json!("build");
        let rel = write_receipt(&harness, "build.json", &body);
        let (code, _, err) = harness.run(&["rerun", rel.as_str()]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains(CODE_RERUN_RECEIPT_INVALID), "{err}");
    }

    #[test]
    fn rerun_rejects_incomplete_receipt() {
        let harness = Harness::new("rerun-incomplete");
        let mut body = base_receipt(&["//a:bad"]);
        body["validation_performed"] = serde_json::json!(false);
        let rel = write_receipt(&harness, "truncated.json", &body);
        let (code, _, err) = harness.run(&["rerun", rel.as_str()]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_RERUN_RECEIPT_INCOMPLETE), "{err}");
    }

    #[test]
    fn rerun_needs_exactly_one_receipt() {
        let harness = Harness::new("rerun-arity");
        let (code, _, err) = harness.run(&["rerun"]);
        assert_eq!(code, 2, "{err}");
        let (code, _, err) = harness.run(&["rerun", "a.json", "b.json"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn rerun_reports_nothing_to_rerun_text() {
        let harness = Harness::new("rerun-empty");
        let rel = write_receipt(&harness, "empty.json", &base_receipt(&[]));
        let (code, out, _) = harness.run(&["rerun", rel.as_str(), "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("nothing to rerun"), "{out}");
    }

    #[test]
    fn rerun_reports_nothing_to_rerun_json() {
        let harness = Harness::new("rerun-empty-json");
        let rel = write_receipt(&harness, "empty.json", &base_receipt(&[]));
        let (code, out, _) = harness.run(&["rerun", rel.as_str(), "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let notices = events_of_kind(&events, "notice");
        assert!(
            notices
                .iter()
                .any(|notice| notice["code"] == serde_json::json!(CODE_NOTHING_TO_RERUN)),
            "{out}"
        );
        let finished = events_of_kind(&events, "command_finished");
        assert_eq!(finished.len(), 1, "{out}");
        assert_eq!(
            finished[0]["results_complete"],
            serde_json::json!(false),
            "{out}"
        );
    }

    #[test]
    fn rerun_replays_recorded_then_trailing_options() {
        let harness = Harness::new("rerun-argv");
        let rel = write_receipt(&harness, "failed.json", &base_receipt(&["//a:bad"]));
        let inv = invocation(&["rerun", rel.as_str(), "--", "--jobs=4"]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(0)],
            passing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 1, "{:?}", run.argv);
        let argv = &run.argv[0];
        assert!(argv.iter().any(|arg| arg == "//a:bad"), "{argv:?}");
        let recorded = argv
            .iter()
            .position(|arg| arg == "--jobs=2")
            .expect("recorded {argv:?}");
        let trailing = argv
            .iter()
            .position(|arg| arg == "--jobs=4")
            .expect("trailing {argv:?}");
        assert!(recorded < trailing, "{argv:?}");
    }

    #[test]
    fn rerun_keeps_the_delegated_test_code() {
        let harness = Harness::new("rerun-fail");
        let rel = write_receipt(&harness, "failed.json", &base_receipt(&["//a:bad"]));
        let inv = invocation(&["rerun", rel.as_str()]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(1)],
            failing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 1, "{}{}", run.out, run.err);
    }

    #[test]
    fn rerun_delegated_json_carries_both_lifecycles() {
        let harness = Harness::new("rerun-json");
        let rel = write_receipt(&harness, "failed.json", &base_receipt(&["//a:bad"]));
        let inv = invocation(&["rerun", rel.as_str(), "--output=json"]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(0)],
            passing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        let events = json_events(&run.out);
        let started = events_of_kind(&events, "command_started");
        assert_eq!(started.len(), 2, "{}", run.out);
        assert_eq!(started[0]["command"], serde_json::json!("rerun"));
        let finished = events_of_kind(&events, "command_finished");
        assert_eq!(
            finished.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    #[test]
    fn rerun_warns_when_workspace_changes() {
        let harness = Harness::new("rerun-moved");
        let rel = write_receipt(&harness, "failed.json", &base_receipt(&["//a:bad"]));
        let inv = invocation(&["rerun", rel.as_str()]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(0)],
            passing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(
            run.err.contains(CODE_RERUN_WORKSPACE_CHANGED),
            "{}",
            run.err
        );
    }

    #[test]
    fn rerun_warns_about_withheld_options() {
        let harness = Harness::new("rerun-withheld");
        let mut body = base_receipt(&["//a:bad"]);
        body["withheld_options"] = serde_json::json!(["--token"]);
        let rel = write_receipt(&harness, "failed.json", &body);
        let inv = invocation(&["rerun", rel.as_str()]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(0)],
            passing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(run.err.contains(CODE_RERUN_OPTIONS_WITHHELD), "{}", run.err);
        assert!(run.err.contains("--token"), "{}", run.err);
        for argv in &run.argv {
            assert!(
                !argv.iter().any(|arg| arg.contains("abc")),
                "withheld values never launch: {argv:?}"
            );
        }
    }

    #[test]
    fn rerun_preserves_the_recorded_profile() {
        let harness = Harness::new("rerun-profile");
        let mut body = base_receipt(&["//a:bad"]);
        body["profile"] = serde_json::json!("dx_release");
        let rel = write_receipt(&harness, "failed.json", &body);
        let inv = invocation(&["rerun", rel.as_str()]);
        let run = Recording::run_with(
            &harness,
            &inv,
            vec![Some(0)],
            passing_bep(&harness, "//a:bad"),
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 1, "{:?}", run.argv);
        assert!(
            run.argv[0].iter().any(|arg| arg == "--config=dx_release"),
            "recorded profile reaches Bazel: {:?}",
            run.argv[0]
        );
    }

    #[test]
    fn rerun_dry_run_plans_without_launching() {
        let harness = Harness::new("rerun-dry");
        let rel = write_receipt(&harness, "failed.json", &base_receipt(&["//a:bad"]));
        let inv = invocation(&["rerun", rel.as_str(), "--dry-run"]);
        let run = Recording::run_with(&harness, &inv, vec![Some(0)], Vec::new());
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(
            run.argv.is_empty(),
            "dry run launches nothing: {:?}",
            run.argv
        );
    }
}
