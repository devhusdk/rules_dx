use super::common::*;
use crate::args::Invocation;
use crate::plan::WorkflowVerb;
use crate::reports::{
    coverage_line_rate, junit_infrastructure_case, parse_test_xml, render_junit, validate_lcov,
    JunitCase, PlannedReport,
};
use crate::resolve::QueryRunner;
use dx_bep::{
    collect_test_events, ArtifactReader, OutputLocations, TestResultFile, TestResultRecord,
};
use dx_output::{
    command_finished, report_event, test_outcome_event, write_event, FinishedCounts, OutputMode,
    TestOutcome,
};
use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::path::Path;

/// Returns one exec path spelled without the build directory, so a report reads the same on every host.
fn stable_exec_path(exec_path: &Path) -> String {
    let text = exec_path.to_string_lossy().replace('\\', "/");
    if let Some(out) = text.find("/bazel-testlogs/") {
        return text[out + "/bazel-testlogs/".len()..].to_owned();
    }
    if let Some(out) = text.find("/testlogs/") {
        return text[out + "/testlogs/".len()..].to_owned();
    }
    if let Some(out) = text.find("/bazel-out/") {
        let rest = &text[out + "/bazel-out/".len()..];
        return match rest.find("/bin/") {
            Some(bin) => rest[bin + "/bin/".len()..].to_owned(),
            None => rest.to_owned(),
        };
    }
    if let Some(bin) = text.find("/bin/") {
        return text[bin + "/bin/".len()..].to_owned();
    }
    exec_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or(text)
}

fn unusable_detail(count: usize, first: &str) -> String {
    if count > 1 {
        format!("{count} missing or invalid results: {first}")
    } else {
        first.to_owned()
    }
}

/// True when Bazel passed and at most a quarter of the reported test results are unusable.
fn partial_results_tolerated(bazel_code: i32, usable: usize, unusable: usize) -> bool {
    bazel_code == 0 && unusable * 3 <= usable
}

/// True for the names Bazel reports a test's coverage result under.
fn is_coverage_output(name: &str) -> bool {
    name == "coverage.dat" || name == "test.lcov"
}

/// True for result files that carry coverage evidence.
fn is_coverage_file(file: &TestResultFile) -> bool {
    is_coverage_output(&file.name)
}

/// The latest attempt Bazel reported for each result's named coverage output.
fn final_coverage_attempts(results: &[TestResultRecord]) -> BTreeMap<(&str, u32, u32, &str), u32> {
    let mut finals = BTreeMap::new();
    for result in results {
        for file in &result.files {
            if !is_coverage_output(&file.name) {
                continue;
            }
            let key = (
                result.label.as_str(),
                result.run,
                result.shard,
                file.name.as_str(),
            );
            let seen = finals.entry(key).or_insert(0);
            *seen = (*seen).max(result.attempt);
        }
    }
    finals
}

pub(crate) struct TestReportsRequest<'a> {
    pub(crate) invocation: &'a Invocation,
    pub(crate) workspace: &'a Path,
    pub(crate) out: &'a mut dyn Write,
    pub(crate) err: &'a mut dyn Write,
    pub(crate) verb: WorkflowVerb,
    pub(crate) bep: &'a Path,
    pub(crate) planned_reports: &'a [PlannedReport],
    pub(crate) stdout_report: bool,
    pub(crate) bazel_code: i32,
    pub(crate) query_runner: &'a dyn QueryRunner,
    pub(crate) pid: u32,
    pub(crate) nonce: u64,
}

pub(crate) const CODE_RUN_OUTPUT_FAILED: &str = "run_output_failed";

fn sanitize_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if out.len() > 120 {
        out.truncate(120);
    }
    out
}

fn is_safe_artifact_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    if name.starts_with('/') {
        return false;
    }
    if name.contains("..") {
        return false;
    }
    if name.as_bytes().contains(&0) {
        return false;
    }
    true
}

fn resolve_run_output_parent(workspace: &Path, raw: &str) -> std::path::PathBuf {
    let rel = Path::new(raw);
    if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        workspace.join(rel)
    }
}

struct RunOutputRequest<'a> {
    workspace: &'a Path,
    verb: WorkflowVerb,
    parent_raw: &'a str,
    pid: u32,
    nonce: u64,
    events: &'a dx_bep::TestEvents,
    invocation: &'a Invocation,
    bazel_code: i32,
    results_complete: bool,
    outcomes: &'a [TestOutcome],
}

fn export_run_outputs(
    request: &RunOutputRequest<'_>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let RunOutputRequest {
        workspace,
        verb,
        parent_raw,
        pid,
        nonce,
        events,
        invocation,
        bazel_code,
        results_complete,
        outcomes,
    } = request;
    let reader = FsArtifacts;
    let parent = resolve_run_output_parent(workspace, parent_raw);
    std::fs::create_dir_all(&parent).map_err(|err| {
        format!(
            "cannot create run-output parent {}: {err}",
            parent.display()
        )
    })?;
    let mut child: Option<std::path::PathBuf> = None;
    let mut last_err = String::new();
    for attempt in 0..100 {
        let name = if attempt == 0 {
            format!("dx-run-{pid}-{nonce}")
        } else {
            format!("dx-run-{pid}-{nonce}-{attempt}")
        };
        let candidate = parent.join(name);
        match std::fs::create_dir(&candidate) {
            Ok(()) => {
                child = Some(candidate);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = err.to_string();
                continue;
            }
            Err(err) => {
                return Err(format!(
                    "cannot create run-output directory {}: {err}",
                    candidate.display()
                ));
            }
        }
    }
    let child = child.ok_or_else(|| {
        format!(
            "cannot claim a unique run-output directory under {}: {last_err}",
            parent.display()
        )
    })?;
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for result in &events.results {
        for file in &result.files {
            if !is_safe_artifact_name(&file.name) {
                entries.push(serde_json::json!({
                    "target": result.label,
                    "run": result.run,
                    "shard": result.shard,
                    "attempt": result.attempt,
                    "status": result.status,
                    "name": file.name,
                    "available": false,
                    "reason": "unsafe artifact name",
                }));
                continue;
            }
            let bytes = match reader.read_artifact(&file.exec_path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    entries.push(serde_json::json!({
                        "target": result.label,
                        "run": result.run,
                        "shard": result.shard,
                        "attempt": result.attempt,
                        "status": result.status,
                        "name": file.name,
                        "available": false,
                        "reason": format!("unreadable artifact: {err}"),
                    }));
                    continue;
                }
            };
            let stem = format!(
                "{}__run{}_shard{}_attempt{}__{}",
                sanitize_component(&result.label),
                result.run,
                result.shard,
                result.attempt,
                sanitize_component(&file.name),
            );
            let mut retained = stem.clone();
            let mut dup = 0;
            while !used.insert(retained.clone()) {
                dup += 1;
                retained = format!("{stem}__dup{dup}");
            }
            let dest = child.join(&retained);
            if let Err(err) = std::fs::write(&dest, &bytes) {
                return Err(format!(
                    "cannot retain {} for {}: {err}",
                    file.name, result.label
                ));
            }
            entries.push(serde_json::json!({
                "target": result.label,
                "run": result.run,
                "shard": result.shard,
                "attempt": result.attempt,
                "status": result.status,
                "name": file.name,
                "path": retained,
                "available": true,
                "bytes": bytes.len(),
            }));
        }
    }
    entries.sort_by(|a, b| {
        let key = |v: &serde_json::Value| {
            (
                v["target"].as_str().unwrap_or_default().to_owned(),
                v["run"].as_u64().unwrap_or(0),
                v["shard"].as_u64().unwrap_or(0),
                v["attempt"].as_u64().unwrap_or(0),
                v["name"].as_str().unwrap_or_default().to_owned(),
            )
        };
        key(a).cmp(&key(b))
    });
    let manifest = serde_json::json!({
        "version": 1,
        "command": verb.name(),
        "artifacts": entries,
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("cannot encode run-output manifest: {err}"))?;
    let manifest_path = child.join("manifest.json");
    std::fs::write(&manifest_path, format!("{text}\n"))
        .map_err(|err| format!("cannot write {}: {err}", manifest_path.display()))?;
    super::rerun::write_receipt(
        &child,
        invocation,
        workspace,
        *verb,
        *bazel_code,
        *results_complete,
        outcomes,
    )?;
    Ok((child, manifest_path))
}

/// Reads the output roots `bazel info` reports for this run, or workspace-only locations.
fn output_locations(
    invocation: &Invocation,
    workspace: &Path,
    verb: WorkflowVerb,
    query_runner: &dyn QueryRunner,
) -> OutputLocations {
    let mut locations = OutputLocations::new(workspace);
    let mut argv = dx_process::startup_argv(&invocation.bazel_startup_options);
    argv.push("info".to_owned());
    if verb != WorkflowVerb::Coverage {
        argv.push(invocation.profile().config_flag());
    }
    argv.extend(invocation.bazel_options.iter().cloned());
    argv.push("bazel-testlogs".to_owned());
    argv.push("execution_root".to_owned());
    if let Ok(result) = query_runner.run_info(&argv, workspace) {
        if result.code == Some(0) {
            locations.apply_bazel_info(&result.stdout);
        }
    }
    locations
}

/// The machine outcome for one BEP test status: Bazel stays authoritative.
///
/// Passing after retry is distinct from a clean pass, and any other reported
/// status keeps its own lowercased token so new Bazel statuses stay distinct.
/// A missing status is unknown, never a fabricated zero value.
fn status_outcome(status: Option<&str>) -> String {
    match status {
        None => "unknown".to_owned(),
        Some("PASSED") => "passed".to_owned(),
        Some("FLAKY") => "passed_after_retry".to_owned(),
        Some(other) => other.to_lowercase(),
    }
}

/// The machine outcome for one test result beside its target summary.
///
/// Bazel reports per-attempt `testResult` statuses, so a retried attempt that
/// passes reads `PASSED` on its own line; only the `testSummary`
/// `overallStatus` says the target passed after retry. A passed attempt under
/// a `FLAKY` summary is `passed_after_retry`, never an ordinary pass.
fn result_outcome(status: Option<&str>, summary: Option<&str>) -> String {
    if status == Some("PASSED") && summary == Some("FLAKY") {
        return "passed_after_retry".to_owned();
    }
    status_outcome(status)
}

/// Indexes `testSummary` overall statuses by target and configuration.
fn summary_statuses(events: &dx_bep::TestEvents) -> BTreeMap<(&str, Option<&str>), &str> {
    let mut statuses = BTreeMap::new();
    for summary in &events.summaries {
        if let Some(status) = summary.overall_status.as_deref() {
            statuses.insert(
                (summary.label.as_str(), summary.configuration.as_deref()),
                status,
            );
        }
    }
    statuses
}

/// Names one test result's file so a missing artifact is traceable to its run, shard, and attempt.
fn result_where(label: &str, name: &str, run: u32, shard: u32, attempt: u32) -> String {
    format!("{name} for {label} (run {run}, shard {shard}, attempt {attempt})")
}

pub(crate) fn execute_test_reports(request: TestReportsRequest<'_>) -> i32 {
    let TestReportsRequest {
        invocation,
        workspace,
        out,
        err,
        verb,
        bep,
        planned_reports,
        stdout_report,
        bazel_code,
        query_runner,
        pid,
        nonce,
    } = request;
    let events = match std::fs::File::open(bep).map_err(|err| {
        (
            CODE_UNREADABLE_BEP.to_owned(),
            format!("failed to read build events: {err}"),
        )
    }) {
        Ok(file) => {
            let locations = output_locations(invocation, workspace, verb, query_runner);
            match collect_test_events(BufReader::new(file), Some(&locations)) {
                Ok(events) => events,
                Err(error) => {
                    let _ = std::fs::remove_file(bep);
                    return operational(
                        invocation,
                        out,
                        err,
                        CODE_INVALID_BEP,
                        &format!("invalid build events: {error}"),
                    );
                }
            }
        }
        Err((code, message)) => {
            let _ = std::fs::remove_file(bep);
            return operational(invocation, out, err, &code, &message);
        }
    };
    let _ = std::fs::remove_file(bep);
    let reader = FsArtifacts;
    let mut complete = bazel_code == 0;
    let mut detail = String::new();
    let mut strict_detail = String::new();
    let mut suites: Vec<(String, Vec<JunitCase>)> = Vec::new();
    let mut lcov_documents: Vec<String> = Vec::new();
    let mut outcomes: Vec<TestOutcome> = Vec::new();
    let summaries = summary_statuses(&events);
    if verb == WorkflowVerb::Test {
        let mut grouped: BTreeMap<String, Vec<JunitCase>> = BTreeMap::new();
        let mut first_error = String::new();
        let mut error_count = 0usize;
        let mut usable_count = 0usize;
        for result in &events.results {
            let summary = summaries
                .get(&(result.label.as_str(), result.configuration.as_deref()))
                .copied();
            let mut outcome = TestOutcome {
                target: result.label.clone(),
                configuration: result.configuration.clone(),
                outcome: result_outcome(result.status.as_deref(), summary),
                status: result.status.clone(),
                cached: result.cached_locally,
                run: Some(result.run),
                shard: Some(result.shard),
                attempt: Some(result.attempt),
                duration_millis: result.duration_millis,
                ..TestOutcome::default()
            };
            for file in &result.files {
                if file.name != "test.xml" {
                    continue;
                }
                let where_ = result_where(
                    &result.label,
                    &file.name,
                    result.run,
                    result.shard,
                    result.attempt,
                );
                let bytes = match reader.read_artifact(&file.exec_path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        error_count += 1;
                        outcome.artifacts_missing += 1;
                        if first_error.is_empty() {
                            first_error = format!(
                                "unreadable {where_} at {}: {error}",
                                stable_exec_path(&file.exec_path)
                            );
                        }
                        continue;
                    }
                };
                let shard = result.shard.saturating_sub(1);
                let attempt = result.attempt.saturating_sub(1);
                match parse_test_xml(&bytes, shard, attempt) {
                    Ok(cases) => {
                        usable_count += 1;
                        outcome.artifacts_collected += 1;
                        outcome.cases_total =
                            Some(outcome.cases_total.unwrap_or(0) + cases.len() as u64);
                        for case in &cases {
                            if case.failure.is_some() {
                                outcome.cases_failed = Some(outcome.cases_failed.unwrap_or(0) + 1);
                            }
                            if case.error.is_some() {
                                outcome.cases_error = Some(outcome.cases_error.unwrap_or(0) + 1);
                            }
                            if case.skipped.is_some() {
                                outcome.cases_skipped =
                                    Some(outcome.cases_skipped.unwrap_or(0) + 1);
                            }
                        }
                        grouped
                            .entry(result.label.clone())
                            .or_default()
                            .extend(cases);
                    }
                    Err(error) => {
                        error_count += 1;
                        outcome.artifacts_invalid += 1;
                        if first_error.is_empty() {
                            first_error = format!(
                                "invalid {where_} at {}: {error}",
                                stable_exec_path(&file.exec_path)
                            );
                        }
                    }
                }
            }
            outcome.evidence_complete = outcome.artifacts_collected > 0
                && outcome.artifacts_missing == 0
                && outcome.artifacts_invalid == 0;
            outcomes.push(outcome);
        }
        if grouped.is_empty() {
            complete = false;
            if detail.is_empty() {
                detail = if first_error.is_empty() {
                    "no test.xml artifacts were reported".to_owned()
                } else {
                    first_error.clone()
                };
            }
        } else if !first_error.is_empty() {
            if partial_results_tolerated(bazel_code, usable_count, error_count) {
                if invocation.strict_evidence {
                    strict_detail = unusable_detail(error_count, &first_error);
                } else {
                    let _ = writeln!(
                        err,
                        "dx: incomplete_results (tolerated): {}",
                        unusable_detail(error_count, &first_error)
                    );
                }
            } else {
                complete = false;
                if detail.is_empty() {
                    detail = unusable_detail(error_count, &first_error);
                }
            }
        }
        suites = grouped.into_iter().collect();
    } else {
        let mut first_error = String::new();
        let mut error_count = 0usize;
        let finals = final_coverage_attempts(&events.results);
        for result in &events.results {
            let mut required = Vec::new();
            for file in &result.files {
                if !is_coverage_output(&file.name) {
                    continue;
                }
                let key = (
                    result.label.as_str(),
                    result.run,
                    result.shard,
                    file.name.as_str(),
                );
                let Some(final_attempt) = finals.get(&key) else {
                    continue;
                };
                if result.attempt < *final_attempt {
                    continue;
                }
                required.push(file);
            }
            if result.files.iter().any(is_coverage_file) && required.is_empty() {
                continue;
            }
            let mut outcome = TestOutcome {
                target: result.label.clone(),
                configuration: result.configuration.clone(),
                outcome: result_outcome(
                    result.status.as_deref(),
                    summaries
                        .get(&(result.label.as_str(), result.configuration.as_deref()))
                        .copied(),
                ),
                status: result.status.clone(),
                cached: result.cached_locally,
                run: Some(result.run),
                shard: Some(result.shard),
                attempt: Some(result.attempt),
                duration_millis: result.duration_millis,
                ..TestOutcome::default()
            };
            for file in required {
                let where_ = result_where(
                    &result.label,
                    &file.name,
                    result.run,
                    result.shard,
                    result.attempt,
                );
                let bytes = match reader.read_artifact(&file.exec_path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        error_count += 1;
                        outcome.artifacts_missing += 1;
                        if first_error.is_empty() {
                            first_error = format!(
                                "unreadable {where_} at {}: {error}",
                                stable_exec_path(&file.exec_path)
                            );
                        }
                        continue;
                    }
                };
                match validate_lcov(&bytes) {
                    Ok(()) => {
                        outcome.artifacts_collected += 1;
                        lcov_documents.push(String::from_utf8_lossy(&bytes).into_owned());
                    }
                    Err(error) => {
                        if bytes.iter().all(|b| b.is_ascii_whitespace()) {
                            continue;
                        }
                        error_count += 1;
                        outcome.artifacts_invalid += 1;
                        if first_error.is_empty() {
                            first_error = format!(
                                "invalid {where_} at {}: {error}",
                                stable_exec_path(&file.exec_path)
                            );
                        }
                    }
                }
            }
            outcome.evidence_complete = (outcome.artifacts_collected > 0
                && outcome.artifacts_missing == 0
                && outcome.artifacts_invalid == 0)
                || !result.files.iter().any(is_coverage_file);
            outcomes.push(outcome);
        }
        if lcov_documents.is_empty() {
            complete = false;
            if detail.is_empty() {
                detail = if first_error.is_empty() {
                    "no coverage.dat artifacts were reported".to_owned()
                } else {
                    first_error.clone()
                };
            }
        } else if !first_error.is_empty() {
            complete = false;
            if detail.is_empty() {
                detail = unusable_detail(error_count, &first_error);
            }
        }
    }
    let evidence_complete =
        !events.results.is_empty() && outcomes.iter().all(|outcome| outcome.evidence_complete);
    let strict_fail = invocation.strict_evidence && bazel_code == 0 && !evidence_complete;
    if verb == WorkflowVerb::Test && (!complete || strict_fail) {
        let reason = if detail.is_empty() {
            &strict_detail
        } else {
            &detail
        };
        suites.push(junit_infrastructure_case(reason));
    }
    if invocation.strict_evidence && strict_fail && !strict_detail.is_empty() {
        let _ = writeln!(err, "dx: incomplete_results (strict): {strict_detail}");
    }
    if invocation.output == OutputMode::Json {
        for outcome in &outcomes {
            if let Ok(event) = test_outcome_event(outcome) {
                let _ = write_event(out, &event);
            }
        }
    } else if matches!(invocation.output, OutputMode::Text { .. }) {
        for outcome in &outcomes {
            if outcome.outcome == "passed_after_retry" {
                match outcome.attempt {
                    Some(attempt) => {
                        let _ = writeln!(
                            err,
                            "dx: flaky success (passed after retry): {} (attempt {})",
                            outcome.target, attempt
                        );
                    }
                    None => {
                        let _ = writeln!(
                            err,
                            "dx: flaky success (passed after retry): {}",
                            outcome.target
                        );
                    }
                }
            }
        }
    }
    let mut run_output_dir: Option<std::path::PathBuf> = None;
    let mut run_output_manifest: Option<std::path::PathBuf> = None;
    if let Some(parent) = invocation.run_output.as_deref() {
        let request = RunOutputRequest {
            workspace,
            verb,
            parent_raw: parent,
            pid,
            nonce,
            events: &events,
            invocation,
            bazel_code,
            results_complete: complete,
            outcomes: &outcomes,
        };
        match export_run_outputs(&request) {
            Ok((child, manifest)) => {
                run_output_dir = Some(child);
                run_output_manifest = Some(manifest);
            }
            Err(message) => {
                return operational(invocation, out, err, CODE_RUN_OUTPUT_FAILED, &message);
            }
        }
    }
    let mut reports_ok = true;
    for planned in planned_reports {
        let document: Option<String> = match verb {
            WorkflowVerb::Test => match render_junit(&suites) {
                Ok(document) => Some(document),
                Err(error) => {
                    reports_ok = false;
                    report_failed(
                        out,
                        err,
                        invocation.output,
                        &format!("failed to render {} report: {error}", planned.format.name()),
                    );
                    continue;
                }
            },
            WorkflowVerb::Coverage => {
                if lcov_documents.is_empty() {
                    None
                } else {
                    let mut combined = lcov_documents.join("\n");
                    if !combined.ends_with('\n') {
                        combined.push('\n');
                    }
                    Some(combined)
                }
            }
            _ => None, // LCOV_EXCL_LINE - reason: defensive arm, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        };
        let Some(document) = document else {
            reports_ok = false;
            report_failed(
                out,
                err,
                invocation.output,
                &format!(
                    "failed to render {} report: {detail}",
                    planned.format.name()
                ),
            );
            continue;
        };
        let written = write_report_document(out, workspace, &planned.destination, &document);
        if let Err(error) = written {
            reports_ok = false;
            report_failed(
                out,
                err,
                invocation.output,
                &format!(
                    "failed to write {} report to {error}",
                    planned.format.name(),
                ),
            );
            continue;
        }
        if invocation.output == OutputMode::Json {
            if let Ok(event) = report_event(
                planned.format.name(),
                planned.destination.display(),
                complete && reports_ok && !strict_fail,
            ) {
                let _ = write_event(out, &event);
            }
        } else if matches!(invocation.output, OutputMode::Text { .. }) && !stdout_report {
            let _ = writeln!(
                out,
                "Wrote {} report to {}.",
                planned.format.name(),
                planned.destination.display()
            );
        }
    }
    if let (Some(dir), Some(manifest)) = (run_output_dir.as_ref(), run_output_manifest.as_ref()) {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = report_event(
                "run-output",
                &manifest.display().to_string(),
                complete && reports_ok && !strict_fail,
            ) {
                let _ = write_event(out, &event);
            }
        } else if matches!(invocation.output, OutputMode::Text { .. }) && !stdout_report {
            let _ = writeln!(out, "Wrote run outputs to {}.", dir.display());
        }
    }
    let evidence_detail = if detail.is_empty() {
        strict_detail.as_str()
    } else {
        detail.as_str()
    };
    if (!complete || strict_fail)
        && !evidence_detail.is_empty()
        && (verb == WorkflowVerb::Coverage || planned_reports.is_empty())
    {
        if invocation.output == OutputMode::Json {
            if let Ok(event) =
                dx_output::error_event("incomplete_results", evidence_detail, None, None, None)
            {
                let _ = write_event(out, &event);
            }
        } else if !complete {
            let _ = writeln!(err, "dx: incomplete_results: {evidence_detail}");
        }
    }
    let mut threshold_ok = true;
    if verb == WorkflowVerb::Coverage {
        if let Some(minimum) = invocation.min_coverage {
            if !detail.is_empty() {
                threshold_ok = false;
            } else {
                let summary = match coverage_line_rate(&lcov_documents, &|path| {
                    std::fs::read_to_string(workspace.join(path)).ok()
                }) {
                    Ok((covered, eligible)) if eligible > 0 => {
                        let percent = 100.0 * covered as f64 / eligible as f64;
                        let passed = covered * 100 >= u64::from(minimum) * eligible;
                        threshold_ok = passed;
                        if passed {
                            format!(
                                "coverage {percent:.2}% ({covered}/{eligible} lines) meets minimum {minimum}%"
                            )
                        } else {
                            format!(
                                "coverage_below_minimum: coverage {percent:.2}% ({covered}/{eligible} lines) below minimum {minimum}%"
                            )
                        }
                    }
                    Ok(_) => {
                        threshold_ok = false;
                        "coverage_below_minimum: no executable lines in the collected LCOV"
                            .to_owned()
                    }
                    Err(error) => {
                        threshold_ok = false;
                        format!("coverage_below_minimum: {error}")
                    }
                };
                if invocation.output == OutputMode::Json {
                    if !threshold_ok {
                        if let Ok(event) = dx_output::error_event(
                            CODE_COVERAGE_BELOW_MINIMUM,
                            &summary,
                            None,
                            None,
                            None,
                        ) {
                            let _ = write_event(out, &event);
                        }
                    }
                } else {
                    let _ = writeln!(err, "dx: {summary}");
                }
            }
        }
    }
    let code = if bazel_code != 0 {
        bazel_code
    } else if strict_fail {
        1
    } else if complete && reports_ok && threshold_ok {
        0
    } else {
        1
    };
    if invocation.output == OutputMode::Json {
        if bazel_code != 0 {
            if let Ok(event) = dx_output::error_event(
                "bazel_failed",
                &format!(
                    "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                    invocation.command.name(),
                ),
                None,
                None,
                Some("execute"),
            ) {
                let _ = write_event(out, &event);
            }
        }
        let _ = write_event(
            out,
            &command_finished(
                code,
                &FinishedCounts {
                    results_complete: Some(complete && reports_ok && !strict_fail),
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
    use super::stable_exec_path;
    use std::path::Path;

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

    /// A local `file://` URI for a path that is not there, spelled the way this host spells one.
    ///
    /// Windows only reads a local `file://` URI back as a path when it carries a drive letter, so
    /// the rooted spelling every other host accepts would read as a remote host there.
    fn missing_uri(name: &str) -> String {
        if cfg!(windows) {
            format!("file:///C:/nonexistent/{name}")
        } else {
            format!("file:///nonexistent/{name}")
        }
    }

    const MINIMAL_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;

    const MINIMAL_LCOV: &str = "SF:src/a.py\nDA:1,1\nend_of_record\n";

    #[test]
    fn test_junit_file_report_succeeds() {
        let harness = Harness::new("test-junit");
        let uri = write_bep_artifact(&harness, "test.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Wrote junit report to out.xml."));
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("<testsuites name=\"dx\""), "{text}");
        assert!(text.contains("<testsuite name=\"//a:t\""), "{text}");
    }

    #[test]
    fn test_stdout_report_owns_stdout() {
        let harness = Harness::new("test-stdout");
        let uri = write_bep_artifact(&harness, "test-stdout.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=text", "--report=junit=-"]);
        assert_eq!(code, 0);
        assert!(out.contains("<testsuites name=\"dx\""), "{out}");
        assert!(!out.contains("Running test"), "{out}");
    }

    #[test]
    fn test_missing_artifacts_are_incomplete() {
        let harness = Harness {
            raw_bep: Some(vec![String::from(
                "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
            )]),
            ..Harness::new("test-empty")
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1);
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn test_preserves_bazel_failure_code() {
        let harness = Harness::new("test-bazel-fails");
        let uri = write_bep_artifact(&harness, "fail.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            bazel_code: 4,
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, _) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 4);
    }

    #[test]
    fn coverage_lcov_file_report_succeeds() {
        let harness = Harness::new("cov-ok");
        let uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 0, "{out}");
        let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
        assert_eq!(document, MINIMAL_LCOV.as_bytes());
    }

    #[test]
    fn coverage_invalid_tracefile_fails() {
        let harness = Harness::new("cov-bad");
        let uri = write_bep_artifact(&harness, "bad.dat", b"not lcov");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1);
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_empty_tracefile_skipped_when_valid_present() {
        let harness = Harness::new("cov-empty-skipped");
        let empty_uri = write_bep_artifact(&harness, "empty.dat", b"");
        let valid_uri = write_bep_artifact(&harness, "valid.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:empty", &[(String::from("test.lcov"), empty_uri)]),
                test_result_line("//a:valid", &[(String::from("test.lcov"), valid_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    #[test]
    fn coverage_whitespace_only_tracefile_skipped() {
        let harness = Harness::new("cov-ws-skipped");
        let ws_uri = write_bep_artifact(&harness, "ws.dat", b"  \n\t\n");
        let valid_uri = write_bep_artifact(&harness, "valid.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ws", &[(String::from("test.lcov"), ws_uri)]),
                test_result_line("//a:valid", &[(String::from("test.lcov"), valid_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    const HALF_LCOV: &str = "SF:src/a.py\nDA:1,1\nDA:2,0\nend_of_record\n";

    #[test]
    fn coverage_min_coverage_passes_at_threshold() {
        let harness = coverage_harness("cov-threshold-ok", MINIMAL_LCOV.as_bytes());
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    #[test]
    fn coverage_min_coverage_fails_below_threshold() {
        let harness = coverage_harness("cov-threshold-low", HALF_LCOV.as_bytes());
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("coverage_below_minimum"), "{err}");
        assert!(err.contains("50.00% (1/2 lines)"), "{err}");
    }

    #[test]
    fn coverage_min_coverage_failure_reports_json_event() {
        let harness = coverage_harness("cov-threshold-json", HALF_LCOV.as_bytes());
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=80"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("coverage_below_minimum"), "{out}");
    }

    #[test]
    fn coverage_min_coverage_fails_without_executable_lines() {
        let harness = coverage_harness("cov-threshold-empty", b"SF:src/a.py\nend_of_record\n");
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no executable lines"), "{err}");
    }

    #[test]
    fn coverage_min_coverage_rejects_invalid_markers() {
        let harness = Harness::new("cov-threshold-markers");
        let source = harness.workspace.join("src/lib.rs");
        std::fs::create_dir_all(source.parent().expect("parent")).expect("mkdir");
        std::fs::write(&source, "// LCOV_EXCL_LINE\nfn a() {}\n").expect("write");
        let uri = write_bep_artifact(
            &harness,
            "coverage.dat",
            b"SF:src/lib.rs\nDA:1,1\nDA:2,1\nend_of_record\n",
        );
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("coverage_below_minimum"), "{err}");
    }

    #[test]
    fn test_unreadable_bep_is_operational() {
        let mut harness = Harness::new("test-nobep");
        harness.skip_bep = true;
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("unreadable_bep"), "{err}");
    }

    #[test]
    fn test_invalid_bep_is_operational() {
        let harness = Harness {
            raw_bep: Some(vec![String::from("not json")]),
            ..Harness::new("test-badbep")
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("invalid_bep"), "{err}");
    }

    #[test]
    fn test_ignores_non_xml_entries() {
        let harness = Harness::new("test-ignore-log");
        let xml = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        std::fs::write(harness.temp.join("ok.log"), b"log").expect("log");
        let log_uri = format!("file://{}", harness.temp.join("ok.log").display());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[
                    (String::from("test.log"), log_uri),
                    (String::from("test.xml"), xml),
                ],
            )]),
            ..harness
        };
        let (code, _, _) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0);
    }

    #[test]
    fn test_unreadable_artifact_is_incomplete() {
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), missing_uri("a.xml"))],
            )]),
            ..Harness::new("test-unreadable")
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn test_single_missing_xml_tolerated_when_bazel_passed() {
        let harness = Harness::new("test-partial-tolerated");
        let raw = (0..4)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok7-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("test.xml"), missing_uri("missing.xml"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("tolerated"), "{err}");
    }

    #[test]
    fn test_small_missing_fraction_tolerated_when_bazel_passed() {
        let harness = Harness::new("test-partial-fraction");
        let raw = (0..6)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok5-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        for index in 0..2 {
            lines.push(test_result_line(
                &format!("//a:missing{index}"),
                &[(
                    String::from("test.xml"),
                    missing_uri(&format!("fraction{index}.xml")),
                )],
            ));
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("incomplete_results (tolerated): 2 missing or invalid results"),
            "{err}"
        );
    }

    #[test]
    fn test_large_missing_fraction_fails_when_bazel_passed() {
        let harness = Harness::new("test-partial-majority");
        let raw = (0..2)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok6-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        for index in 0..5 {
            lines.push(test_result_line(
                &format!("//a:missing{index}"),
                &[(
                    String::from("test.xml"),
                    missing_uri(&format!("majority{index}.xml")),
                )],
            ));
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("5 missing or invalid results"), "{err}");
    }

    #[test]
    fn test_one_missing_and_one_invalid_xml_tolerated_when_bazel_passed() {
        let harness = Harness::new("test-mixed-two");
        let raw = (0..6)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok4-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:bad",
            &[(
                String::from("test.xml"),
                write_bep_artifact(&harness, "bad2.xml", b"not xml"),
            )],
        ));
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("test.xml"), missing_uri("missing5.xml"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("incomplete_results (tolerated): 2 missing or invalid results"),
            "{err}"
        );
    }

    #[test]
    fn test_single_missing_xml_fails_when_bazel_failed() {
        let harness = Harness::new("test-partial-bazelfail");
        let uri = write_bep_artifact(&harness, "ok2.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("test.xml"), uri)]),
                test_result_line(
                    "//a:missing",
                    &[(String::from("test.xml"), missing_uri("missing2.xml"))],
                ),
            ]),
            bazel_code: 1,
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn test_invalid_xml_is_incomplete() {
        let harness = Harness::new("test-badxml");
        let uri = write_bep_artifact(&harness, "bad.xml", b"not xml");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_ignores_non_lcov_and_reports_missing() {
        let harness = Harness::new("cov-ignore");
        let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), xml)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no coverage.dat"), "{err}");
    }

    #[test]
    fn coverage_unreadable_is_incomplete() {
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("coverage.dat"), missing_uri("c.dat"))],
            )]),
            ..Harness::new("cov-unreadable")
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_single_missing_dat_fails_when_bazel_passed() {
        let harness = Harness::new("cov-partial-missing");
        let raw = (0..4)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("coverage.dat"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok7-{index}.dat"),
                            MINIMAL_LCOV.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(
            err.contains("coverage.dat for //a:missing (run 1, shard 1, attempt 1)"),
            "{err}"
        );
    }

    #[test]
    fn coverage_missing_artifact_blocks_min_coverage() {
        let harness = Harness::new("cov-missing-threshold");
        let raw = (0..4)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("coverage.dat"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok8-{index}.dat"),
                            MINIMAL_LCOV.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(!err.contains("meets minimum"), "{err}");
    }

    #[test]
    fn coverage_invalid_artifact_blocks_min_coverage() {
        let harness = Harness::new("cov-invalid-threshold");
        let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
        let bad_uri = write_bep_artifact(&harness, "bad.dat", b"not lcov");
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
                test_result_line("//a:bad", &[(String::from("coverage.dat"), bad_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(
            err.contains("invalid coverage.dat for //a:bad (run 1, shard 1, attempt 1)"),
            "{err}"
        );
        assert!(!err.contains("meets minimum"), "{err}");
    }

    #[test]
    fn coverage_superseded_attempt_is_not_required() {
        let harness = Harness::new("cov-retry-ok");
        let final_uri = write_bep_artifact(&harness, "final.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    &[(String::from("test.lcov"), missing_uri("attempt1.dat"))],
                ),
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    &[(String::from("test.lcov"), final_uri)],
                ),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("incomplete_results"), "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    #[test]
    fn coverage_final_attempt_missing_is_required() {
        let harness = Harness::new("cov-retry-missing");
        let stale_uri = write_bep_artifact(&harness, "stale.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    &[(String::from("test.lcov"), stale_uri)],
                ),
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    &[(String::from("test.lcov"), missing_uri("final.dat"))],
                ),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(
            err.contains("test.lcov for //a:t (run 1, shard 1, attempt 2)"),
            "{err}"
        );
    }

    #[test]
    fn coverage_uninstrumented_test_does_not_demand_coverage() {
        let harness = Harness::new("cov-uninstrumented");
        let lcov_uri = write_bep_artifact(&harness, "inst.dat", MINIMAL_LCOV.as_bytes());
        let xml_uri = write_bep_artifact(&harness, "plain.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:instrumented", &[(String::from("test.lcov"), lcov_uri)]),
                test_result_line("//a:plain", &[(String::from("test.xml"), xml_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("incomplete_results"), "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    #[test]
    fn coverage_partial_report_is_written_and_marked_incomplete() {
        let harness = Harness::new("cov-partial-report");
        let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
                test_result_line(
                    "//a:missing",
                    &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
                ),
            ]),
            ..harness
        };
        let (code, out, err) =
            harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Wrote lcov report to out.lcov."), "{out}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(
            err.contains("coverage.dat for //a:missing"),
            "the partial report names the missing target artifact: {err}"
        );
        let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
        assert_eq!(document, MINIMAL_LCOV.as_bytes());
    }

    #[test]
    fn coverage_incomplete_json_agrees_with_exit_status() {
        let harness = Harness::new("cov-incomplete-json");
        let ok_uri = write_bep_artifact(&harness, "ok.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("coverage.dat"), ok_uri)]),
                test_result_line(
                    "//a:missing",
                    &[(String::from("coverage.dat"), missing_uri("missing.dat"))],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("incomplete_results"), "{out}");
        assert!(out.contains("\"results_complete\":false"), "{out}");
        assert!(!out.contains("coverage_below_minimum"), "{out}");
    }

    #[test]
    fn coverage_two_missing_dats_fail_when_bazel_passed() {
        let harness = Harness::new("cov-partial-two");
        let raw = (0..6)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("coverage.dat"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok3-{index}.dat"),
                            MINIMAL_LCOV.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        for index in 0..2 {
            lines.push(test_result_line(
                &format!("//a:missing{index}"),
                &[(
                    String::from("coverage.dat"),
                    missing_uri(&format!("missing3-{index}.dat")),
                )],
            ));
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(err.contains("2 missing or invalid results"), "{err}");
    }

    #[test]
    fn coverage_one_invalid_and_one_missing_dat_fail_when_bazel_passed() {
        let harness = Harness::new("cov-mixed-two");
        let raw = (0..6)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.lcov"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok4-{index}.dat"),
                            MINIMAL_LCOV.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:bad",
            &[(
                String::from("test.lcov"),
                write_bep_artifact(&harness, "bad3.dat", b"not lcov"),
            )],
        ));
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("coverage.dat"), missing_uri("missing5.dat"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(err.contains("2 missing or invalid results"), "{err}");
    }

    #[test]
    fn coverage_large_missing_fraction_fails_when_bazel_passed() {
        let harness = Harness::new("cov-partial-majority");
        let raw = (0..2)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("coverage.dat"),
                        write_bep_artifact(
                            &harness,
                            &format!("ok6-{index}.dat"),
                            MINIMAL_LCOV.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        for index in 0..5 {
            lines.push(test_result_line(
                &format!("//a:missing{index}"),
                &[(
                    String::from("coverage.dat"),
                    missing_uri(&format!("covmajority{index}.dat")),
                )],
            ));
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("5 missing or invalid results"), "{err}");
    }

    #[test]
    fn coverage_empty_dat_does_not_count_as_missing() {
        let harness = Harness::new("cov-empty-ok");
        let empty = write_bep_artifact(&harness, "empty2.dat", b"");
        let ok = write_bep_artifact(&harness, "ok5.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:empty", &[(String::from("coverage.dat"), empty)]),
                test_result_line("//a:ok", &[(String::from("coverage.dat"), ok)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_single_missing_dat_fails_when_bazel_failed() {
        let harness = Harness::new("cov-partial-bazelfail");
        let uri = write_bep_artifact(&harness, "ok2.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("coverage.dat"), uri)]),
                test_result_line(
                    "//a:missing",
                    &[(String::from("coverage.dat"), missing_uri("missing2.dat"))],
                ),
            ]),
            bazel_code: 1,
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_report_failed_when_incomplete() {
        let harness = Harness::new("cov-repfail");
        let uri = write_bep_artifact(&harness, "bad2.dat", b"not lcov");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("report_failed"), "{err}");
    }

    #[test]
    fn test_report_write_failure_is_operational() {
        let harness = Harness::new("test-writefail");
        let uri = write_bep_artifact(&harness, "w.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&[
            "test",
            "--output=text",
            "--report=junit=missing-dir/out.xml",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("report_failed"), "{err}");
    }

    #[test]
    fn test_report_json_emits_report_event() {
        let harness = Harness::new("test-jsonrep");
        let uri = write_bep_artifact(&harness, "j.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("report"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
    }

    #[test]
    fn test_rejects_diff_output_at_parse() {
        let err = crate::args::parse(
            &["test", "--output=diff", "--report=junit=out.xml"]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .expect_err("diff rejected");
        assert_eq!(
            err,
            crate::args::ArgsError::UnsupportedOption {
                command: "test",
                option: "--output=diff".to_owned(),
            }
        );
    }

    #[test]
    fn test_incomplete_json_reports_incomplete_event() {
        let harness = Harness {
            raw_bep: Some(vec![String::from(
                "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
            )]),
            ..Harness::new("test-incjson")
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("incomplete_results"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
    }

    #[test]
    fn coverage_lcov_without_trailing_newline_gets_newline() {
        let harness = Harness::new("cov-nonl");
        let raw = b"SF:src/a.py\nDA:1,1\nend_of_record";
        let uri = write_bep_artifact(&harness, "nonl.dat", raw);
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 0, "{out}");
        let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
        assert!(document.ends_with(b"\n"), "{document:?}");
    }

    #[test]
    fn coverage_render_failure_json_reports_error_event() {
        let harness = Harness::new("cov-render-json");
        let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), xml)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--report=lcov=out.lcov"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("report_failed"), "{out}");
    }

    #[test]
    fn test_report_write_failure_json_reports_error_event() {
        let harness = Harness::new("test-writefail-json");
        let uri = write_bep_artifact(&harness, "w2.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&[
            "test",
            "--output=json",
            "--report=junit=missing-dir/out.xml",
        ]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("report_failed"), "{out}");
    }

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

    #[test]
    fn test_bytestream_outputs_resolve_through_info_testlogs() {
        let harness = Harness::new("test-info-roots");
        let testlogs = harness.temp.join("testlogs");
        harness.query.script_info(&format!(
            "bazel-testlogs: {}\nexecution_root: {}\nignored: {}\n",
            testlogs.display(),
            harness.temp.join("execroot").display(),
            harness.temp.display(),
        ));
        let root = testlogs.join("a").join("t");
        std::fs::create_dir_all(&root).expect("testlogs dir");
        std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(
                    String::from("test.xml"),
                    "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                )],
            )]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("tests=\"1\""), "{text}");
        let calls = harness.query.info_calls.borrow();
        assert_eq!(calls.len(), 1, "one bazel info run per report run");
        let argv = &calls[0];
        assert!(argv.contains(&"info".to_owned()), "{argv:?}");
        assert!(argv.contains(&"bazel-testlogs".to_owned()), "{argv:?}");
        assert!(argv.contains(&"execution_root".to_owned()), "{argv:?}");
        assert!(
            argv.contains(&"--config=dx_dev".to_owned()),
            "the test profile reaches info: {argv:?}"
        );
        assert!(
            harness.query.calls.borrow().is_empty(),
            "info must not consume query outputs"
        );
    }

    #[test]
    fn test_info_carries_startup_options_before_the_verb() {
        let harness = Harness::new("test-info-startup");
        let testlogs = harness.temp.join("testlogs");
        harness.query.script_info(&format!(
            "bazel-testlogs: {}\nexecution_root: {}\n",
            testlogs.display(),
            harness.temp.join("execroot").display(),
        ));
        let root = testlogs.join("a").join("t");
        std::fs::create_dir_all(&root).expect("testlogs dir");
        std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(
                    String::from("test.xml"),
                    "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                )],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&[
            "test",
            "--bazel-startup-option=--output_base=/tmp/a",
            "--output=text",
        ]);
        assert_eq!(code, 0, "{err}");
        let calls = harness.query.info_calls.borrow();
        assert_eq!(calls.len(), 1, "one bazel info run per report run");
        let argv = &calls[0];
        let verb = argv.iter().position(|arg| arg == "info").expect("info");
        assert_eq!(
            &argv[..verb],
            &[
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
            ]
        );
    }

    #[test]
    fn test_sharded_and_retried_bytestream_outputs_keep_identity() {
        let harness = Harness::new("test-info-identity");
        let testlogs = harness.temp.join("testlogs");
        harness
            .query
            .script_info(&format!("bazel-testlogs: {}\n", testlogs.display()));
        let unit = testlogs.join("a").join("t");
        let attempts = unit.join("shard_1_of_2").join("test_attempts");
        std::fs::create_dir_all(&attempts).expect("attempt dir");
        std::fs::create_dir_all(unit.join("shard_2_of_2")).expect("shard dir");
        std::fs::write(attempts.join("attempt_1.xml"), MINIMAL_TEST_XML).expect("attempt xml");
        std::fs::write(unit.join("shard_1_of_2").join("test.xml"), MINIMAL_TEST_XML)
            .expect("shard xml");
        std::fs::write(unit.join("shard_2_of_2").join("test.xml"), MINIMAL_TEST_XML)
            .expect("shard xml");
        let entry = || {
            vec![(
                String::from("test.xml"),
                "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
            )]
        };
        let harness = Harness {
            raw_bep: Some(vec![
                test_summary_line("//a:t", 2),
                test_result_identity_line("//a:t", 1, 1, 1, &entry()),
                test_result_identity_line("//a:t", 1, 1, 2, &entry()),
                test_result_identity_line("//a:t", 1, 2, 1, &entry()),
            ]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("tests=\"3\""), "{text}");
    }

    #[test]
    fn test_unreadable_bytestream_names_label_and_identity() {
        let harness = Harness::new("test-info-missing");
        harness.query.script_info(&format!(
            "bazel-testlogs: {}\n",
            harness.temp.join("testlogs").display()
        ));
        let harness = Harness {
            raw_bep: Some(vec![test_result_identity_line(
                "//a:t",
                2,
                3,
                1,
                &[(
                    String::from("test.xml"),
                    "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                )],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
        assert!(
            err.contains("unreadable test.xml for //a:t (run 2, shard 3, attempt 1)"),
            "{err}"
        );
        assert!(
            err.contains("a/t/shard_3_of_3_run_2_of_2/test.xml"),
            "the stable path keeps package and identity: {err}"
        );
    }

    #[test]
    fn test_workspace_fallback_survives_without_info() {
        let harness = Harness::new("test-no-info");
        let root = harness.workspace.join("bazel-testlogs").join("a").join("t");
        std::fs::create_dir_all(&root).expect("testlogs dir");
        std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(
                    String::from("test.xml"),
                    "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                )],
            )]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(harness.query.info_calls.borrow().len(), 1);
    }

    const MIXED_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/><testcase name="fails" classname="c" time="0.2"><failure message="m">t</failure></testcase><testcase name="errors" classname="c" time="0.3"><error message="e">t</error></testcase><testcase name="skips" classname="c" time="0"><skipped message="s"/></testcase></testsuite></testsuites>"#;

    const EMPTY_TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"></testsuite></testsuites>"#;

    #[test]
    fn test_outcomes_carry_bep_status_cache_and_timing() {
        let harness = Harness::new("test-outcome-meta");
        let ok_uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let cached_uri = write_bep_artifact(&harness, "cached.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_full_line(
                    "//a:ok",
                    1,
                    1,
                    1,
                    "PASSED",
                    Some("cfg-one"),
                    Some(false),
                    Some(12),
                    &[(String::from("test.xml"), ok_uri)],
                ),
                test_result_full_line(
                    "//a:cached",
                    2,
                    3,
                    1,
                    "FAILED",
                    None,
                    Some(true),
                    None,
                    &[(String::from("test.xml"), cached_uri)],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 2, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//a:cached"));
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["cached"], serde_json::json!(true));
        assert_eq!(outcomes[0]["run"], serde_json::json!(2));
        assert_eq!(outcomes[0]["shard"], serde_json::json!(3));
        assert!(outcomes[0].get("configuration").is_none(), "{out}");
        assert!(outcomes[0].get("duration_millis").is_none(), "{out}");
        assert_eq!(outcomes[1]["target"], serde_json::json!("//a:ok"));
        assert_eq!(outcomes[1]["outcome"], serde_json::json!("passed"));
        assert_eq!(outcomes[1]["status"], serde_json::json!("PASSED"));
        assert_eq!(outcomes[1]["configuration"], serde_json::json!("cfg-one"));
        assert_eq!(outcomes[1]["cached"], serde_json::json!(false));
        assert_eq!(outcomes[1]["run"], serde_json::json!(1));
        assert_eq!(outcomes[1]["shard"], serde_json::json!(1));
        assert_eq!(outcomes[1]["attempt"], serde_json::json!(1));
        assert_eq!(outcomes[1]["duration_millis"], serde_json::json!(12));
        assert_eq!(outcomes[1]["evidence_complete"], serde_json::json!(true));
        let kinds = event_kinds(&events);
        let last_outcome = kinds
            .iter()
            .rposition(|kind| *kind == "test_outcome")
            .expect("outcome");
        let finished = kinds
            .iter()
            .position(|kind| *kind == "command_finished")
            .expect("finished");
        assert!(
            last_outcome < finished,
            "outcomes precede command_finished: {kinds:?}"
        );
        let finished_event = event(&events, "command_finished");
        assert_eq!(finished_event["exit_code"], serde_json::json!(0));
        assert_eq!(finished_event["results_complete"], serde_json::json!(true));
    }

    #[test]
    fn test_outcome_counts_cases_by_severity() {
        let harness = Harness::new("test-outcome-cases");
        let uri = write_bep_artifact(&harness, "mixed.xml", MIXED_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("passed"));
        assert_eq!(outcomes[0]["cases_total"], serde_json::json!(4));
        assert_eq!(outcomes[0]["cases_failed"], serde_json::json!(1));
        assert_eq!(outcomes[0]["cases_error"], serde_json::json!(1));
        assert_eq!(outcomes[0]["cases_skipped"], serde_json::json!(1));
        assert_eq!(outcomes[0]["artifacts"]["collected"], serde_json::json!(1));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(true));
    }

    #[test]
    fn test_flaky_result_reports_passed_after_retry() {
        let harness = Harness::new("test-outcome-flaky");
        let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
        let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    "FAILED",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), first_uri)],
                ),
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    "FLAKY",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), retry_uri)],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 2, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["attempt"], serde_json::json!(1));
        assert_eq!(
            outcomes[1]["outcome"],
            serde_json::json!("passed_after_retry")
        );
        assert_eq!(outcomes[1]["attempt"], serde_json::json!(2));
    }

    #[test]
    fn test_flaky_summary_marks_retried_pass_distinctly() {
        let harness = Harness::new("test-summary-flaky");
        let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
        let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    "FAILED",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), first_uri)],
                ),
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    "PASSED",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), retry_uri)],
                ),
                test_flaky_summary_line("//a:t"),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 2, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["attempt"], serde_json::json!(1));
        assert_eq!(
            outcomes[1]["outcome"],
            serde_json::json!("passed_after_retry")
        );
        assert_eq!(outcomes[1]["attempt"], serde_json::json!(2));
    }

    #[test]
    fn test_flaky_success_is_visible_in_text_summary() {
        let harness = Harness::new("test-flaky-text");
        let first_uri = write_bep_artifact(&harness, "first.xml", MINIMAL_TEST_XML.as_bytes());
        let retry_uri = write_bep_artifact(&harness, "retry.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    "FAILED",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), first_uri)],
                ),
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    "PASSED",
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), retry_uri)],
                ),
                test_flaky_summary_line("//a:t"),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("dx: flaky success (passed after retry): //a:t (attempt 2)"),
            "{err}"
        );
    }

    #[test]
    fn test_clean_pass_prints_no_flaky_summary() {
        let harness = Harness::new("test-clean-text");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("flaky success"), "{err}");
    }

    #[test]
    fn test_timeout_and_failure_outcomes_stay_distinct() {
        let harness = Harness::new("test-outcome-statuses");
        let timeout_uri = write_bep_artifact(&harness, "timeout.xml", MINIMAL_TEST_XML.as_bytes());
        let failed_uri = write_bep_artifact(&harness, "failed.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_full_line(
                    "//a:slow",
                    1,
                    1,
                    1,
                    "TIMEOUT",
                    None,
                    None,
                    None,
                    &[(String::from("test.xml"), timeout_uri)],
                ),
                test_result_full_line(
                    "//a:bad",
                    1,
                    1,
                    1,
                    "FAILED",
                    None,
                    None,
                    None,
                    &[(String::from("test.xml"), failed_uri)],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 2, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[1]["outcome"], serde_json::json!("timeout"));
        assert_eq!(outcomes[1]["status"], serde_json::json!("TIMEOUT"));
    }

    #[test]
    fn test_missing_status_reports_unknown_outcome() {
        let harness = Harness::new("test-outcome-unknown");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let line = serde_json::json!({
            "id": {"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}},
            "testResult": {"testActionOutput": [{"name": "test.xml", "uri": uri}]},
        })
        .to_string();
        let harness = Harness {
            raw_bep: Some(vec![line]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("unknown"));
        assert!(outcomes[0].get("status").is_none(), "{out}");
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(true));
    }

    #[test]
    fn test_zero_case_xml_reports_zero_counts() {
        let harness = Harness::new("test-outcome-zero");
        let uri = write_bep_artifact(&harness, "empty.xml", EMPTY_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["cases_total"], serde_json::json!(0));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(true));
    }

    #[test]
    fn test_status_only_result_is_incomplete_evidence() {
        let harness = Harness {
            raw_bep: Some(vec![String::from(
                "{\"id\": {\"testResult\": {\"label\": \"//a:t\"}}, \"testResult\": {\"status\": \"PASSED\"}}",
            )]),
            ..Harness::new("test-outcome-nofile")
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("passed"));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
        assert_eq!(outcomes[0]["artifacts"]["collected"], serde_json::json!(0));
        let finished = event(&events, "command_finished");
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    #[test]
    fn test_strict_evidence_fails_tolerated_gaps() {
        let harness = Harness::new("test-strict-tolerated");
        let raw = (0..4)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("strict-ok-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:missing",
            &[(String::from("test.xml"), missing_uri("strict-missing.xml"))],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json", "--strict-evidence"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 5, "{out}");
        let missing = outcomes
            .iter()
            .find(|outcome| outcome["target"] == serde_json::json!("//a:missing"))
            .expect("missing target outcome");
        assert_eq!(missing["evidence_complete"], serde_json::json!(false));
        assert_eq!(missing["artifacts"]["missing"], serde_json::json!(1));
        let incomplete: Vec<_> = events_of_kind(&events, "error")
            .into_iter()
            .filter(|error| error["code"] == serde_json::json!("incomplete_results"))
            .collect();
        assert_eq!(incomplete.len(), 1, "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    #[test]
    fn test_strict_evidence_reports_text_failure() {
        let harness = Harness::new("test-strict-text");
        let raw = (0..4)
            .map(|index| {
                test_result_line(
                    &format!("//a:ok{index}"),
                    &[(
                        String::from("test.xml"),
                        write_bep_artifact(
                            &harness,
                            &format!("strict-text-ok-{index}.xml"),
                            MINIMAL_TEST_XML.as_bytes(),
                        ),
                    )],
                )
            })
            .collect::<Vec<_>>();
        let mut lines = raw;
        lines.push(test_result_line(
            "//a:missing",
            &[(
                String::from("test.xml"),
                missing_uri("strict-text-missing.xml"),
            )],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text", "--strict-evidence"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results (strict)"), "{err}");
        assert!(!err.contains("tolerated"), "{err}");
    }

    #[test]
    fn test_strict_evidence_passes_complete_run() {
        let harness = Harness::new("test-strict-complete");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&[
            "test",
            "--output=text",
            "--strict-evidence",
            "--report=junit=out.xml",
        ]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("incomplete_results"), "{err}");
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("<testsuite name=\"//a:t\""), "{text}");
    }

    #[test]
    fn test_failed_build_preserves_code_and_reports_outcomes() {
        let harness = Harness::new("test-outcome-bazelfail");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            bazel_code: 4,
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 4, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(true));
        let failed: Vec<_> = events_of_kind(&events, "error")
            .into_iter()
            .filter(|error| error["code"] == serde_json::json!("bazel_failed"))
            .collect();
        assert_eq!(failed.len(), 1, "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(4));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    #[test]
    fn test_signalled_run_emits_no_test_outcome() {
        let mut harness = Harness::new("test-outcome-signalled");
        harness.signalled = true;
        let (code, out, err) = harness.run(&["test", "--output=json"]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(!out.contains("test_outcome"), "{out}");
    }

    #[test]
    fn test_coverage_outcomes_mark_only_final_attempt_evidence() {
        let harness = Harness::new("cov-outcome-retry");
        let final_uri = write_bep_artifact(&harness, "final.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    &[(String::from("test.lcov"), missing_uri("cov-attempt1.dat"))],
                ),
                test_result_identity_line(
                    "//a:t",
                    1,
                    1,
                    2,
                    &[(String::from("test.lcov"), final_uri)],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["attempt"], serde_json::json!(2));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(true));
        assert_eq!(outcomes[0]["artifacts"]["collected"], serde_json::json!(1));
    }

    fn run_output_child(harness: &Harness, parent: &str) -> std::path::PathBuf {
        let dir = harness.workspace.join(parent);
        let mut children: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("run-output parent")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.is_dir())
            .collect();
        assert_eq!(children.len(), 1, "{children:?}");
        children.pop().expect("child")
    }

    fn read_manifest(child: &std::path::Path) -> serde_json::Value {
        let text = std::fs::read_to_string(child.join("manifest.json")).expect("manifest");
        serde_json::from_str(&text).expect("manifest json")
    }

    #[test]
    fn test_run_output_retains_logs_and_writes_manifest() {
        let harness = Harness::new("test-runout-ok");
        let xml = write_bep_artifact(&harness, "r.xml", MINIMAL_TEST_XML.as_bytes());
        let log_uri = write_bep_artifact(&harness, "r.log", b"hello log");
        let shot_uri = write_bep_artifact(&harness, "shot.png", b"fake-png");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[
                    (String::from("test.xml"), xml),
                    (String::from("test.log"), log_uri),
                    (String::from("screenshot.png"), shot_uri),
                ],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=text", "--run-output=out"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Wrote run outputs to"), "{out}");
        let child = run_output_child(&harness, "out");
        let manifest = read_manifest(&child);
        assert_eq!(manifest["version"], serde_json::json!(1));
        assert_eq!(manifest["command"], serde_json::json!("test"));
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 3, "{manifest}");
        for entry in artifacts {
            assert_eq!(entry["target"], serde_json::json!("//a:t"));
            assert_eq!(entry["available"], serde_json::json!(true));
            let rel = entry["path"].as_str().expect("path");
            assert!(!rel.contains('/'), "{rel}");
            assert!(!rel.contains('\\'), "{rel}");
            let bytes = std::fs::read(child.join(rel)).expect("retained");
            assert!(!bytes.is_empty(), "{rel}");
        }
        let names: Vec<&str> = artifacts
            .iter()
            .map(|entry| entry["name"].as_str().expect("name"))
            .collect();
        assert!(names.contains(&"test.xml"));
        assert!(names.contains(&"test.log"));
        assert!(names.contains(&"screenshot.png"));
    }

    #[test]
    fn test_run_output_json_stays_ndjson_with_report_event() {
        let harness = Harness::new("test-runout-json");
        let uri = write_bep_artifact(&harness, "j2.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json", "--run-output=out"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert!(!events.is_empty(), "{out}");
        let reports = events_of_kind(&events, "report");
        let manifest = reports
            .iter()
            .find(|report| report["format"] == serde_json::json!("run-output"))
            .expect("run-output report");
        let path = manifest["path"].as_str().expect("path");
        assert!(path.ends_with("manifest.json"), "{path}");
        assert!(std::path::Path::new(path).is_file(), "{path}");
    }

    #[test]
    fn test_run_output_marks_missing_and_unsafe_names() {
        let harness = Harness::new("test-runout-neg");
        let xml = write_bep_artifact(&harness, "ok3.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("test.xml"), xml)]),
                test_result_line(
                    "//a:missing",
                    &[(String::from("test.log"), missing_uri("gone.log"))],
                ),
                test_result_line(
                    "//a:evil",
                    &[(String::from("../evil"), missing_uri("evil"))],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=text", "--run-output=out"]);
        assert_eq!(code, 0, "{out}");
        let child = run_output_child(&harness, "out");
        let manifest = read_manifest(&child);
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 3, "{manifest}");
        let ok = artifacts
            .iter()
            .find(|entry| entry["target"] == serde_json::json!("//a:ok"))
            .expect("ok");
        assert_eq!(ok["available"], serde_json::json!(true));
        let missing = artifacts
            .iter()
            .find(|entry| entry["target"] == serde_json::json!("//a:missing"))
            .expect("missing");
        assert_eq!(missing["available"], serde_json::json!(false));
        assert!(missing.get("path").is_none(), "{missing}");
        let evil = artifacts
            .iter()
            .find(|entry| entry["target"] == serde_json::json!("//a:evil"))
            .expect("evil");
        assert_eq!(evil["available"], serde_json::json!(false));
        assert!(
            evil["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("unsafe"),
            "{evil}"
        );
        assert!(child.join("manifest.json").is_file());
        for entry in std::fs::read_dir(&child).expect("read child") {
            let path = entry.expect("entry").path();
            if path.is_file() && path.file_name().unwrap_or_default() != "manifest.json" {
                let name = path
                    .file_name()
                    .expect("name")
                    .to_string_lossy()
                    .into_owned();
                assert!(!name.contains('/'), "{name}");
                assert!(!name.contains(".."), "{name}");
            }
        }
    }

    #[test]
    fn test_run_output_is_unique_across_runs() {
        let harness = Harness::new("test-runout-twice");
        let xml = write_bep_artifact(&harness, "twice.xml", MINIMAL_TEST_XML.as_bytes());
        let shot = write_bep_artifact(&harness, "twice.png", b"fake-png");
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[
                    (String::from("test.xml"), xml),
                    (String::from("shot.png"), shot),
                ],
            )]),
            ..harness
        };
        let (first_code, _, _) = harness.run(&["test", "--output=text", "--run-output=out"]);
        assert_eq!(first_code, 0);
        let (second_code, _, _) = harness.run(&["test", "--output=text", "--run-output=out"]);
        assert_eq!(second_code, 0);
        let dir = harness.workspace.join("out");
        let children: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("parent")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.is_dir())
            .collect();
        assert_eq!(children.len(), 2, "{children:?}");
        for child in &children {
            let manifest = read_manifest(child);
            let artifacts = manifest["artifacts"].as_array().expect("artifacts");
            assert_eq!(artifacts.len(), 2, "{manifest}");
            assert!(artifacts
                .iter()
                .any(|entry| entry["name"] == serde_json::json!("shot.png")));
        }
        assert_ne!(children[0], children[1]);
    }

    #[test]
    fn coverage_run_output_retains_lcov() {
        let harness = Harness::new("cov-runout-ok");
        let uri = write_bep_artifact(&harness, "c.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.lcov"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=text", "--run-output=out"]);
        assert_eq!(code, 0, "{out}");
        let child = run_output_child(&harness, "out");
        let manifest = read_manifest(&child);
        assert_eq!(manifest["command"], serde_json::json!("coverage"));
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 1, "{manifest}");
        assert_eq!(artifacts[0]["available"], serde_json::json!(true));
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::super::test_support::*;

    const RECEIPT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c" time="0.1"/></testsuite></testsuites>"#;
    const RECEIPT_LCOV: &str = "SF:src/a.py\nDA:1,1\nend_of_record\n";

    fn read_receipt(child: &std::path::Path) -> serde_json::Value {
        let text = std::fs::read_to_string(child.join("receipt.json")).expect("receipt");
        serde_json::from_str(&text).expect("receipt json")
    }

    fn receipt_child(harness: &Harness, parent: &str) -> std::path::PathBuf {
        let dir = harness.workspace.join(parent);
        let mut children: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("run-output parent")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.is_dir())
            .collect();
        assert_eq!(children.len(), 1, "{children:?}");
        children.pop().expect("child")
    }

    #[test]
    fn test_run_output_writes_receipt_with_failed_targets() {
        let harness = Harness::new("test-receipt-ok");
        let xml = write_bep_artifact(&harness, "ok.xml", RECEIPT_XML.as_bytes());
        let bad = write_bep_artifact(&harness, "bad.xml", RECEIPT_XML.as_bytes());
        let harness = Harness {
            bazel_code: 1,
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("test.xml"), xml)]),
                test_result_full_line(
                    "//a:bad",
                    1,
                    1,
                    1,
                    "FAILED",
                    None,
                    None,
                    None,
                    &[(String::from("test.xml"), bad)],
                ),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&[
            "test",
            "//a:ok",
            "//a:bad",
            "--output=text",
            "--run-output=out",
        ]);
        assert_eq!(code, 1, "{out}");
        let child = receipt_child(&harness, "out");
        let receipt = read_receipt(&child);
        assert_eq!(receipt["version"], serde_json::json!(1));
        assert_eq!(receipt["command"], serde_json::json!("test"));
        assert_eq!(
            receipt["scopes"],
            serde_json::json!(["//a:ok", "//a:bad"]),
            "{receipt}"
        );
        assert_eq!(receipt["profile"], serde_json::json!("dx_dev"), "{receipt}");
        assert_eq!(
            receipt["failed_targets"],
            serde_json::json!(["//a:bad"]),
            "{receipt}"
        );
        assert_eq!(receipt["validation_performed"], serde_json::json!(true));
        assert_eq!(receipt["manifest"], serde_json::json!("manifest.json"));
        let outcomes = receipt["outcomes"].as_array().expect("outcomes");
        assert_eq!(outcomes.len(), 2, "{receipt}");
        assert_eq!(
            receipt["workspace"]["identity"],
            serde_json::json!("partial"),
            "{receipt}"
        );
    }

    #[test]
    fn test_receipt_redacts_secret_options() {
        let harness = Harness::new("test-receipt-redact");
        let xml = write_bep_artifact(&harness, "redact.xml", RECEIPT_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), xml)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&[
            "test",
            "--output=text",
            "--run-output=out",
            "--",
            "--jobs=2",
            "--token=abc",
        ]);
        assert_eq!(code, 0, "{out}");
        let child = receipt_child(&harness, "out");
        let receipt = read_receipt(&child);
        assert_eq!(
            receipt["bazel_options"],
            serde_json::json!(["--jobs=2"]),
            "{receipt}"
        );
        assert_eq!(
            receipt["withheld_options"],
            serde_json::json!(["--token"]),
            "{receipt}"
        );
        assert_eq!(
            receipt["failed_targets"],
            serde_json::json!([]),
            "{receipt}"
        );
    }

    #[test]
    fn test_coverage_receipt_records_no_profile() {
        let harness = Harness::new("test-receipt-coverage");
        let trace = write_bep_artifact(&harness, "trace.dat", RECEIPT_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("coverage.dat"), trace)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=text", "--run-output=out"]);
        assert_eq!(code, 0, "{out}");
        let child = receipt_child(&harness, "out");
        let receipt = read_receipt(&child);
        assert_eq!(
            receipt["command"],
            serde_json::json!("coverage"),
            "{receipt}"
        );
        assert!(receipt["profile"].is_null(), "{receipt}");
    }
}
