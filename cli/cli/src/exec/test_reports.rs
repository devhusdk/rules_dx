use super::build_failures::report_build_failures;
use super::common::*;
use super::coverage_candidate::{Candidate, Unavailable, candidate, tracked_orphans};
use super::failure::CODE_NO_TESTS;
use super::failure_reports::write_failure_reports;
use super::skipped_targets::{group_skipped, skipped_notice};
use dx_bep::{
    ArtifactReader, OutputLocations, TestEvents, TestResultFile, TestResultRecord,
    collect_test_events,
};
use dx_lcov::{CellAccount, SOURCES_OUTPUT_GROUP, SourceManifest, parse_source_manifest};
use dx_output::{
    ActionCounts, FinishedCounts, NoticeEvent, OutputMode, TestCase, TestCounts, TestOutcome,
    command_finished, notice_event, report_event, test_outcome_event, test_result_record,
};
use dx_plan::args::Invocation;
use dx_plan::plan::WorkflowVerb;
use dx_plan::reports::{
    Destination, JunitCase, JunitMessage, PlannedReport, ReportError, StandardFormat,
    junit_infrastructure_case, parse_test_xml, render_junit, validate_lcov,
};
use dx_plan::resolve::QueryRunner;
use std::collections::BTreeMap;
use std::io::{BufReader, Write};
use std::path::Path;

/// Returns one exec path spelled without the build directory, so a report reads the same on every host.
fn stable_exec_path(exec_path: &Path) -> String {
    let text = dx_path::posix(exec_path);
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

/// Bazel's exit code when every test built and ran but some failed.
const BAZEL_TESTS_FAILED: i32 = 3;

/// Whether a test outcome counts as a failed test.
fn is_failed(outcome: &str) -> bool {
    matches!(outcome, "failed" | "timeout")
}

fn unusable_detail(count: usize, first: &str) -> String {
    if count > 1 {
        format!("{count} missing or invalid results: {first}")
    } else {
        first.to_owned()
    }
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

/// Marks each outcome a later attempt of the same target, configuration, run and shard replaced.
fn mark_superseded_attempts(reported: &mut [TestOutcome]) {
    let mut latest: BTreeMap<(String, Option<String>, u32, u32), u32> = BTreeMap::new();
    for outcome in reported.iter() {
        if let (Some(run), Some(shard), Some(attempt)) =
            (outcome.run, outcome.shard, outcome.attempt)
        {
            let key = (
                outcome.target.clone(),
                outcome.configuration.clone(),
                run,
                shard,
            );
            let seen = latest.entry(key).or_insert(attempt);
            *seen = (*seen).max(attempt);
        }
    }
    for outcome in reported.iter_mut() {
        if let (Some(run), Some(shard), Some(attempt)) =
            (outcome.run, outcome.shard, outcome.attempt)
        {
            let key = (
                outcome.target.clone(),
                outcome.configuration.clone(),
                run,
                shard,
            );
            outcome.superseded = latest.get(&key).is_some_and(|last| attempt < *last);
        }
    }
}

pub(crate) struct TestReportsRequest<'a> {
    pub(crate) invocation: &'a Invocation,
    pub(crate) workspace: &'a Path,
    pub(crate) out: &'a mut dyn Write,
    pub(crate) err: &'a mut dyn Write,
    pub(crate) verb: WorkflowVerb,
    pub(crate) bep: &'a Path,
    pub(crate) planned_reports: &'a [PlannedReport],
    pub(crate) failure_reports: &'a [PlannedReport],
    pub(crate) results_reports: &'a [PlannedReport],
    pub(crate) run_id: &'a str,
    pub(crate) stdout_report: bool,
    pub(crate) bazel_code: i32,
    pub(crate) query_runner: &'a dyn QueryRunner,
    pub(crate) results_complete: &'a mut bool,
    pub(crate) actions: Option<ActionCounts>,
    /// True when the run resolved to exactly `//...`, so every tracked source is in scope.
    pub(crate) whole_repo: bool,
}

/// Reads the output roots `bazel info` reports for this run, or workspace-only locations.
#[allow(clippy::disallowed_methods)] // info takes no build settings
pub(super) fn output_locations(
    invocation: &Invocation,
    workspace: &Path,
    verb: WorkflowVerb,
    query_runner: &dyn QueryRunner,
) -> OutputLocations {
    let mut locations = OutputLocations::new(workspace);
    let mut argv = dx_process::startup_argv(&invocation.bazel_startup_options);
    argv.push("info".to_owned());
    if verb != WorkflowVerb::Coverage {
        argv.push(invocation.build_mode().config_flag());
    }
    argv.extend(invocation.raw_bazel_options().iter().cloned());
    argv.push("bazel-testlogs".to_owned());
    argv.push("execution_root".to_owned());
    argv.push("output_path".to_owned());
    if let Ok(result) = query_runner.run_info(&argv, workspace)
        && result.code == Some(0)
    {
        locations.apply_bazel_info(&result.stdout);
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

const CASE_EVENT_LIMIT: usize = 1000;

const CASE_MESSAGE_LIMIT: usize = 4096;

const LOG_TAIL_LINES: usize = 40;

const LOG_TAIL_BYTES: usize = 8192;

/// The last `LOG_TAIL_LINES` lines of a test log. When those exceed `LOG_TAIL_BYTES`, the oldest
/// whole lines are dropped; a single oversize line is cut at a char boundary instead.
fn log_tail(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(LOG_TAIL_LINES);
    let mut tail = lines[start..].join("\n");
    if tail.len() > LOG_TAIL_BYTES {
        let mut from = tail.len() - LOG_TAIL_BYTES;
        while !tail.is_char_boundary(from) {
            from += 1;
        }
        let at_line_start = tail.as_bytes()[from - 1] == b'\n';
        let window = &tail[from..];
        tail = match window.split_once('\n') {
            _ if at_line_start => window.to_owned(),
            Some((_, rest)) => rest.to_owned(),
            None => window.to_owned(),
        };
    }
    (!tail.trim().is_empty()).then_some(tail)
}

/// Adds one parsed test.xml's cases to the outcome's counts and bounded case list.
fn tally_cases(outcome: &mut TestOutcome, cases: &[JunitCase], cases_seen: &mut Vec<TestCase>) {
    outcome.cases_total = Some(outcome.cases_total.unwrap_or(0) + cases.len() as u64);
    for case in cases {
        if case.failure.is_some() {
            outcome.cases_failed = Some(outcome.cases_failed.unwrap_or(0) + 1);
        }
        if case.error.is_some() {
            outcome.cases_error = Some(outcome.cases_error.unwrap_or(0) + 1);
        }
        if case.skipped.is_some() {
            outcome.cases_skipped = Some(outcome.cases_skipped.unwrap_or(0) + 1);
        }
        if cases_seen.len() < CASE_EVENT_LIMIT || case.failure.is_some() || case.error.is_some() {
            cases_seen.push(test_case(case));
        } else {
            outcome.cases_omitted += 1;
        }
    }
}

/// Points a result that did not pass at its test.log and keeps the log's tail. The undeclared
/// outputs zip, where traces and screenshots land, is listed beside it.
fn attach_log(outcome: &mut TestOutcome, result: &TestResultRecord, reader: &dyn ArtifactReader) {
    if outcome.outcome == "passed" {
        return;
    }
    if let Some(log) = result.files.iter().find(|file| file.name == "test.log") {
        outcome.log_path = Some(log.exec_path.to_string_lossy().into_owned());
        outcome.log_tail = reader
            .read_artifact(&log.exec_path)
            .ok()
            .and_then(|bytes| log_tail(&bytes));
    }
    if let Some(zip) = result
        .files
        .iter()
        .find(|file| file.name.ends_with("outputs.zip"))
    {
        outcome.outputs_path = Some(zip.exec_path.to_string_lossy().into_owned());
    }
}

/// Counts every missing or invalid piece of required evidence and keeps the first one.
#[derive(Default)]
struct Evidence {
    count: usize,
    first: String,
}

impl Evidence {
    fn record(&mut self, problem: String) {
        self.count += 1;
        if self.first.is_empty() {
            self.first = problem;
        }
    }
}

/// The machine outcome of one native test case.
fn case_outcome(case: &JunitCase) -> &'static str {
    if case.error.is_some() {
        "error"
    } else if case.failure.is_some() {
        "failed"
    } else if case.skipped.is_some() {
        "skipped"
    } else {
        "passed"
    }
}

/// Cuts text to at most `limit` bytes on a character boundary.
fn bounded(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_owned(), false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

/// The reason a native report gave for one case, when it gave one.
fn case_message(case: &JunitCase) -> Option<&str> {
    let note: &JunitMessage = case
        .error
        .as_ref()
        .or(case.failure.as_ref())
        .or(case.skipped.as_ref())?;
    let text = note
        .message
        .as_deref()
        .filter(|message| !message.is_empty())
        .unwrap_or(&note.text);
    (!text.is_empty()).then_some(text)
}

/// The first workspace-relative `path:line` in a failure text, such as `foo_test.go:42:` or `(Foo.java:17)`.
fn text_location(text: &str) -> Option<(String, u64)> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ',' | '<' | '>'))
        .find_map(|token| {
            let mut parts = token.split(':');
            let path = parts.next()?;
            let line: u64 = parts.next()?.parse().ok()?;
            let relative = !path.starts_with('/') && !path.starts_with('\\');
            let named = path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.contains('.'));
            (relative && named && line > 0).then(|| (path.to_owned(), line))
        })
}

/// Where a case failed: the report's `file` and `line` attributes, else the first location in its failure text.
fn case_location(case: &JunitCase) -> (Option<String>, Option<u64>) {
    if case.file.is_some() {
        return (case.file.clone(), case.line);
    }
    let note = case.error.as_ref().or(case.failure.as_ref());
    let found = note.and_then(|note| {
        note.message
            .as_deref()
            .and_then(text_location)
            .or_else(|| text_location(&note.text))
    });
    match found {
        Some((file, line)) => (Some(file), Some(line)),
        None => (None, None),
    }
}

/// One case read from a result's native report.
fn test_case(case: &JunitCase) -> TestCase {
    let message = case_message(case).map(|text| bounded(text, CASE_MESSAGE_LIMIT).0);
    let (file, line) = case_location(case);
    TestCase {
        classname: case.classname.clone(),
        name: case.name.clone(),
        outcome: case_outcome(case).to_owned(),
        message,
        file,
        line,
    }
}

/// The machine outcome for a target Bazel summarized without any test result.
fn summary_outcome(status: Option<&str>) -> String {
    match status {
        Some("FAILED_TO_BUILD") => "failed_to_build".to_owned(),
        other => status_outcome(other),
    }
}

/// Accounts for undelivered events, summaries without results, unbuilt tests and skipped targets.
fn account_stream(events: &TestEvents, evidence: &mut Evidence, reported: &mut Vec<TestOutcome>) {
    for gap in &events.gaps {
        evidence.record(gap.describe());
        if gap.kind != "testResult" {
            continue;
        }
        let Some(label) = gap.label.clone() else {
            continue;
        };
        reported.push(TestOutcome {
            target: label,
            configuration: gap.configuration.clone(),
            outcome: gap.outcome().to_owned(),
            run: gap.run,
            shard: gap.shard,
            attempt: gap.attempt,
            artifacts_missing: 1,
            ..TestOutcome::default()
        });
    }
    for summary in &events.summaries {
        let has_result = events.results.iter().any(|result| {
            result.label == summary.label && result.configuration == summary.configuration
        });
        let has_gap = events.gaps.iter().any(|gap| {
            gap.kind == "testResult" && gap.label.as_deref() == Some(summary.label.as_str())
        });
        if has_result || has_gap {
            continue;
        }
        let status = summary.overall_status.as_deref();
        evidence.record(format!(
            "no test result was reported for {} (summary {})",
            summary.label,
            status.unwrap_or("unknown")
        ));
        reported.push(TestOutcome {
            target: summary.label.clone(),
            configuration: summary.configuration.clone(),
            outcome: summary_outcome(status),
            status: summary.overall_status.clone(),
            ..TestOutcome::default()
        });
    }
    for target in &events.unbuilt {
        let summarized = events
            .summaries
            .iter()
            .any(|summary| summary.label == target.label);
        let has_result = events
            .results
            .iter()
            .any(|result| result.label == target.label);
        if summarized || has_result {
            continue;
        }
        evidence.record(format!("{} failed to build", target.label));
        reported.push(TestOutcome {
            target: target.label.clone(),
            configuration: target.configuration.clone(),
            outcome: "failed_to_build".to_owned(),
            ..TestOutcome::default()
        });
    }
    for gap in &events.skipped {
        let Some(label) = gap.label.clone() else {
            continue;
        };
        reported.push(TestOutcome {
            target: label,
            configuration: gap.configuration.clone(),
            outcome: gap.outcome().to_owned(),
            evidence_complete: true,
            ..TestOutcome::default()
        });
    }
    for target in &events.filtered {
        reported.push(TestOutcome {
            target: target.label.clone(),
            configuration: target.configuration.clone(),
            outcome: "filtered".to_owned(),
            evidence_complete: true,
            ..TestOutcome::default()
        });
    }
    if !events.ended {
        evidence.record("build events ended before Bazel's last event".to_owned());
    }
}

const FILTERED_LABELS_SHOWN: usize = 3;

fn filtered_notice(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    filtered: &[dx_bep::UnbuiltTest],
) -> Result<(), i32> {
    if filtered.is_empty() {
        return Ok(());
    }
    let labels: Vec<&str> = filtered
        .iter()
        .take(FILTERED_LABELS_SHOWN)
        .map(|target| target.label.as_str())
        .collect();
    let more = filtered.len().saturating_sub(labels.len());
    let mut message = format!(
        "{} {} did not run: a test filter removed {}",
        filtered.len(),
        if filtered.len() == 1 { "test" } else { "tests" },
        labels.join(", ")
    );
    if more > 0 {
        message.push_str(&format!(" and {more} more"));
    }
    match output {
        OutputMode::Json => {
            emit_built(
                out,
                notice_event(&NoticeEvent::info(CODE_TESTS_FILTERED, message)),
            )?;
        }
        OutputMode::Text { .. } => {
            let _ = writeln!(err, "dx: tests_filtered: {message}");
        }
        _ => {}
    }
    Ok(())
}

/// The results report: every outcome of the run, passed ones included.
fn results_document(
    reported: &[TestOutcome],
    complete: bool,
    bazel_code: i32,
    run_id: &str,
) -> String {
    let results: Vec<serde_json::Value> = reported
        .iter()
        .filter_map(|outcome| test_result_record(outcome).ok())
        .collect();
    let document = serde_json::json!({
        "record_version": 1,
        "run_id": run_id,
        "exit_code": bazel_code,
        "complete": complete,
        "results": results,
    });
    serde_json::to_string_pretty(&document).unwrap_or_default()
}

/// `testSummary` overall statuses keyed by target and configuration.
type Summaries<'a> = BTreeMap<(&'a str, Option<&'a str>), &'a str>;

/// The outcome of one test result before any of its artifacts are read.
fn outcome_for(result: &TestResultRecord, summaries: &Summaries) -> TestOutcome {
    let summary = summaries
        .get(&(result.label.as_str(), result.configuration.as_deref()))
        .copied();
    TestOutcome {
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
    }
}

/// Reads one result artifact, or counts it missing and records why.
fn read_or_record(
    reader: &dyn ArtifactReader,
    file: &TestResultFile,
    where_: &str,
    outcome: &mut TestOutcome,
    evidence: &mut Evidence,
) -> Option<Vec<u8>> {
    match reader.read_artifact(&file.exec_path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            outcome.artifacts_missing += 1;
            evidence.record(format!(
                "unreadable {where_} at {}: {error}",
                stable_exec_path(&file.exec_path)
            ));
            None
        }
    }
}

/// Parses one result's test.xml and stamps each case with the result's run and configuration.
fn parse_cases(result: &TestResultRecord, bytes: &[u8]) -> Result<Vec<JunitCase>, ReportError> {
    let shard = result.shard.saturating_sub(1);
    let attempt = result.attempt.saturating_sub(1);
    let mut cases = parse_test_xml(bytes, shard, attempt)?;
    for case in &mut cases {
        case.run = result.run.saturating_sub(1);
        case.configuration = result.configuration.clone();
    }
    Ok(cases)
}

/// Why the results are incomplete: no artifacts at all, or any missing or invalid evidence.
fn evidence_gap(empty: bool, evidence: &Evidence, missing_text: &str) -> Option<String> {
    if empty {
        Some(if evidence.first.is_empty() {
            missing_text.to_owned()
        } else {
            evidence.first.clone()
        })
    } else if evidence.count > 0 {
        Some(unusable_detail(evidence.count, &evidence.first))
    } else {
        None
    }
}

/// The aspect that writes coverage source manifests, as the build events name it.
const SOURCES_ASPECT_SUFFIX: &str = "coverage_sources.bzl%dx_coverage_sources_aspect";

/// The coverage source manifests a run reported, and every problem that left one missing.
#[derive(Default)]
struct Sources {
    manifests: Vec<SourceManifest>,
    missing: Vec<String>,
    invalid: Vec<String>,
    /// `bazel <version>` from the build events' `started` event.
    collector: Option<String>,
    /// The checkout measured, when the workspace is a Git work tree.
    candidate: Option<Candidate>,
    /// Why the tracked-source check did not run, when it is not an accounting error.
    tracked_unavailable: Option<String>,
}

/// Reads the coverage source manifests from the run's build events.
fn collect_sources(bep: &Path, workspace: &Path) -> Sources {
    let mut sources = Sources {
        collector: dx_bep::stream_build_tool_version(bep)
            .ok()
            .flatten()
            .map(|version| format!("bazel {version}")),
        ..Sources::default()
    };
    let account = match collect_output_account(bep, SOURCES_OUTPUT_GROUP, workspace) {
        Ok(account) => account,
        Err((_, message)) => {
            sources.missing.push(format!(
                "coverage source manifests are unreadable: {message}"
            ));
            return sources;
        }
    };
    for target in &account.targets {
        if !target
            .aspect
            .as_deref()
            .is_some_and(|aspect| aspect.ends_with(SOURCES_ASPECT_SUFFIX))
        {
            continue;
        }
        if !target.success || target.incomplete {
            sources.missing.push(format!(
                "{} did not report its coverage source manifest",
                target.label
            ));
            continue;
        }
        for artifact in &target.artifacts {
            match parse_source_manifest(&String::from_utf8_lossy(&artifact.bytes)) {
                Ok(manifest) => sources.manifests.push(manifest),
                Err(error) => sources.invalid.push(format!(
                    "invalid coverage source manifest {}: {error}",
                    stable_exec_path(&artifact.exec_path)
                )),
            }
        }
    }
    sources
}

/// Everything one run's results produced: report inputs, outcomes and the evidence gap.
#[derive(Default)]
struct Collected {
    suites: Vec<(String, Vec<JunitCase>)>,
    lcov_documents: Vec<String>,
    sources: Sources,
    reported: Vec<TestOutcome>,
    gap: Option<String>,
    /// Sorted, deduplicated (label, language) pairs that ran under coverage without a collector.
    no_collector: Vec<(String, String)>,
}

/// Reads every result's test.xml into suites and outcomes for `dx test`.
fn collect_test_xml(
    events: &TestEvents,
    summaries: &Summaries,
    reader: &dyn ArtifactReader,
    mut evidence: Evidence,
    err: &mut dyn Write,
) -> Collected {
    let mut grouped: BTreeMap<String, Vec<JunitCase>> = BTreeMap::new();
    let mut reported: Vec<TestOutcome> = Vec::new();
    for result in &events.results {
        let mut outcome = outcome_for(result, summaries);
        let mut cases_seen: Vec<TestCase> = Vec::new();
        let mut reports = 0usize;
        for file in &result.files {
            if file.name != "test.xml" {
                continue;
            }
            reports += 1;
            let where_ = result_where(
                &result.label,
                &file.name,
                result.run,
                result.shard,
                result.attempt,
            );
            let Some(bytes) = read_or_record(reader, file, &where_, &mut outcome, &mut evidence)
            else {
                continue;
            };
            match parse_cases(result, &bytes) {
                Ok(cases) => {
                    if std::str::from_utf8(&bytes).is_err() {
                        let _ = writeln!(
                            err,
                            "dx: warning: {where_} has invalid UTF-8; replaced invalid bytes"
                        );
                    }
                    outcome.artifacts_collected += 1;
                    tally_cases(&mut outcome, &cases, &mut cases_seen);
                    grouped
                        .entry(result.label.clone())
                        .or_default()
                        .extend(cases);
                }
                Err(error) => {
                    outcome.artifacts_invalid += 1;
                    evidence.record(format!(
                        "invalid {where_} at {}: {error}",
                        stable_exec_path(&file.exec_path)
                    ));
                }
            }
        }
        if reports == 0 {
            outcome.artifacts_missing += 1;
            evidence.record(format!(
                "{} was not reported",
                result_where(
                    &result.label,
                    "test.xml",
                    result.run,
                    result.shard,
                    result.attempt
                )
            ));
        }
        outcome.evidence_complete = outcome.artifacts_collected > 0
            && outcome.artifacts_missing == 0
            && outcome.artifacts_invalid == 0;
        if outcome.evidence_complete
            && outcome.outcome == "passed"
            && outcome.cases_total == Some(0)
        {
            outcome.outcome = "no_cases".to_owned();
        }
        attach_log(&mut outcome, result, reader);
        outcome.cases = cases_seen;
        reported.push(outcome);
    }
    account_stream(events, &mut evidence, &mut reported);
    let exempt =
        events.results.is_empty() && (!events.skipped.is_empty() || !events.filtered.is_empty());
    let gap = evidence_gap(
        grouped.is_empty() && !exempt,
        &evidence,
        "no test.xml artifacts were reported",
    );
    Collected {
        suites: grouped.into_iter().collect(),
        reported,
        gap,
        ..Collected::default()
    }
}

/// Reads every result's final coverage output into LCOV documents and outcomes for `dx coverage`.
fn collect_lcov(
    events: &TestEvents,
    summaries: &Summaries,
    reader: &dyn ArtifactReader,
    mut evidence: Evidence,
    sources: Sources,
) -> Collected {
    let finals = final_coverage_attempts(&events.results);
    let mut lcov_documents: Vec<String> = Vec::new();
    let mut reported: Vec<TestOutcome> = Vec::new();
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
        let mut outcome = outcome_for(result, summaries);
        for file in required {
            let where_ = result_where(
                &result.label,
                &file.name,
                result.run,
                result.shard,
                result.attempt,
            );
            let Some(bytes) = read_or_record(reader, file, &where_, &mut outcome, &mut evidence)
            else {
                continue;
            };
            match validate_lcov(&bytes) {
                Ok(()) => {
                    outcome.artifacts_collected += 1;
                    lcov_documents.push(String::from_utf8_lossy(&bytes).into_owned());
                }
                Err(error) => {
                    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
                        outcome.artifacts_collected += 1;
                        continue;
                    }
                    outcome.artifacts_invalid += 1;
                    evidence.record(format!(
                        "invalid {where_} at {}: {error}",
                        stable_exec_path(&file.exec_path)
                    ));
                }
            }
        }
        outcome.evidence_complete = (outcome.artifacts_collected > 0
            && outcome.artifacts_missing == 0
            && outcome.artifacts_invalid == 0)
            || !result.files.iter().any(is_coverage_file);
        if outcome.outcome != "passed" {
            // Best effort: coverage completeness stays about the LCOV files only.
            let mut cases_seen: Vec<TestCase> = Vec::new();
            for file in result.files.iter().filter(|file| file.name == "test.xml") {
                let Ok(bytes) = reader.read_artifact(&file.exec_path) else {
                    continue;
                };
                if let Ok(cases) = parse_cases(result, &bytes) {
                    tally_cases(&mut outcome, &cases, &mut cases_seen);
                }
            }
            outcome.cases = cases_seen;
            attach_log(&mut outcome, result, reader);
        }
        reported.push(outcome);
    }
    account_stream(events, &mut evidence, &mut reported);
    for problem in &sources.missing {
        evidence.record(problem.clone());
    }
    let no_collector = no_collector_labels(events);
    // A run of only collector-less tests has no LCOV by design; the
    // no_coverage_collector verdict names the cause instead of a missing-artifact gap.
    let gap = evidence_gap(
        lcov_documents.is_empty() && no_collector.is_empty(),
        &evidence,
        "no coverage.dat artifacts were reported",
    );
    Collected {
        lcov_documents,
        sources,
        reported,
        gap,
        no_collector,
        ..Collected::default()
    }
}

/// Tests tagged as having no coverage collector that produced a result, as (label, language), once each.
fn no_collector_labels(events: &TestEvents) -> Vec<(String, String)> {
    let mut tests: Vec<(String, String)> = events
        .results
        .iter()
        .filter_map(|result| {
            let language = events.no_coverage_collector.get(&result.label)?;
            Some((result.label.clone(), language.clone()))
        })
        .collect();
    tests.sort();
    tests.dedup();
    tests
}

/// Reports tests that ran under coverage with no collector; true when none should fail the run.
/// Without `strict` they are skipped with one summary line per language.
fn announce_no_collector(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    tests: &[(String, String)],
    strict: bool,
) -> Result<bool, i32> {
    if !strict {
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for (_, language) in tests {
            *counts.entry(language.as_str()).or_default() += 1;
        }
        for (language, count) in counts {
            let noun = if count == 1 { "test" } else { "tests" };
            let message =
                format!("skipped {count} {language} {noun}: no coverage collector (#7298)");
            if output == OutputMode::Json {
                emit_built(
                    out,
                    notice_event(&NoticeEvent::info(CODE_NO_COVERAGE_COLLECTOR, message)),
                )?;
            } else {
                let _ = writeln!(err, "dx: {message}");
            }
        }
        return Ok(true);
    }
    for (label, _) in tests {
        let message = format!("no coverage collector for {label}");
        if output == OutputMode::Json {
            emit_built(
                out,
                dx_output::error_event(CODE_NO_COVERAGE_COLLECTOR, &message, None, None, None),
            )?;
        } else {
            let _ = writeln!(err, "dx: {CODE_NO_COVERAGE_COLLECTOR}: {message}");
        }
    }
    Ok(tests.is_empty())
}

/// Prints each outcome in the run's output mode and counts the ones that were not superseded.
fn emit_outcomes(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    reported: &[TestOutcome],
    filtered: &[dx_bep::UnbuiltTest],
) -> Result<TestCounts, i32> {
    let mut counts = TestCounts::default();
    if output == OutputMode::Json {
        for outcome in reported {
            let passed = outcome.evidence_complete
                && matches!(
                    outcome.outcome.as_str(),
                    "passed" | "passed_after_retry" | "no_cases"
                );
            if !outcome.superseded && passed {
                counts.passed += 1;
                if outcome.cached == Some(true) {
                    counts.cached += 1;
                }
                if outcome.outcome == "no_cases" {
                    counts.no_cases += 1;
                }
            } else if !outcome.superseded && is_failed(&outcome.outcome) {
                counts.failed += 1;
            } else if !outcome.superseded
                && matches!(outcome.outcome.as_str(), "skipped" | "filtered")
            {
                counts.skipped += 1;
            }
            if outcome.evidence_complete
                && (outcome.outcome == "passed"
                    || (outcome.outcome == "no_cases" && !outcome.superseded))
            {
                continue;
            }
            emit_built(out, test_outcome_event(outcome))?;
        }
        filtered_notice(output, out, err, filtered)?;
    } else if matches!(output, OutputMode::Text { .. }) {
        filtered_notice(output, out, err, filtered)?;
        for outcome in reported {
            if outcome.outcome == "passed_after_retry" {
                let attempt = outcome
                    .attempt
                    .map(|attempt| format!(" (attempt {attempt})"))
                    .unwrap_or_default();
                let _ = writeln!(
                    err,
                    "dx: flaky success (passed after retry): {}{attempt}",
                    outcome.target
                );
            }
            if !outcome.superseded && is_failed(&outcome.outcome) {
                write_failed_line(err, outcome);
            }
        }
    }
    Ok(counts)
}

/// Most failing case names one text-mode failure line lists.
const TEXT_CASE_LIMIT: usize = 3;

/// Prints `dx: failed: <target>: <cases> (log: <path>)` for one failed result.
fn write_failed_line(err: &mut dyn Write, outcome: &TestOutcome) {
    let failing: Vec<&str> = outcome
        .cases
        .iter()
        .filter(|case| case.outcome == "failed" || case.outcome == "error")
        .map(|case| case.name.as_str())
        .collect();
    let mut line = format!("dx: failed: {}", outcome.target);
    if !failing.is_empty() {
        line.push_str(": ");
        line.push_str(&failing[..failing.len().min(TEXT_CASE_LIMIT)].join(", "));
        if failing.len() > TEXT_CASE_LIMIT {
            line.push_str(&format!(" and {} more", failing.len() - TEXT_CASE_LIMIT));
        }
    }
    if let Some(log) = &outcome.log_path {
        line.push_str(&format!(" (log: {log})"));
    }
    let _ = writeln!(err, "{line}");
}

/// Writes reports to their destinations and announces each one in the run's output mode.
struct ReportSink<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    output: OutputMode,
    workspace: &'a Path,
    stdout_report: bool,
}

impl ReportSink<'_> {
    /// Writes one named report and announces it; false when the write failed and was reported.
    fn write(
        &mut self,
        name: &str,
        destination: &Destination,
        document: &str,
        complete: bool,
    ) -> Result<bool, i32> {
        if let Err(error) = write_report_document(self.out, self.workspace, destination, document) {
            self.failed(&format!("failed to write {name} report to {error}"), None)?;
            return Ok(false);
        }
        if self.output == OutputMode::Json {
            emit_built(
                self.out,
                report_event(name, destination.display(), complete),
            )?;
        } else if matches!(self.output, OutputMode::Text { .. }) && !self.stdout_report {
            let _ = writeln!(
                self.out,
                "Wrote {name} report to {}.",
                destination.display()
            );
        }
        Ok(true)
    }

    /// Reports one report that could not be produced, with the next step for its cause.
    fn failed(&mut self, detail: &str, next: Option<&str>) -> Result<(), i32> {
        report_failed_with_next(self.out, self.err, self.output, detail, next)
    }
}

/// The verb's report document, or why it could not be rendered.
fn render_report(
    verb: WorkflowVerb,
    suites: &[(String, Vec<JunitCase>)],
    lcov_documents: &[String],
    detail: &str,
    format: StandardFormat,
    sources_document: Option<&str>,
) -> Result<String, String> {
    if format == StandardFormat::Sources {
        return match sources_document {
            Some(document) if detail.is_empty() => Ok(document.to_owned()),
            _ => Err(if detail.is_empty() {
                "no coverage source account is available".to_owned()
            } else {
                detail.to_owned()
            }),
        };
    }
    match verb {
        WorkflowVerb::Test => render_junit(suites).map_err(|error| error.to_string()),
        WorkflowVerb::Coverage => {
            if lcov_documents.is_empty() {
                Err(detail.to_owned())
            } else {
                let mut combined = lcov_documents.join("\n");
                if !combined.ends_with('\n') {
                    combined.push('\n');
                }
                Ok(combined)
            }
        }
        _ => Err(detail.to_owned()), // LCOV_EXCL_LINE - reason: defensive arm, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    }
}

/// Reports why the results are incomplete, when evidence was missing or invalid.
fn announce_incomplete(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    detail: &str,
) -> Result<(), i32> {
    if detail.is_empty() {
        return Ok(());
    }
    if output == OutputMode::Json {
        emit_built(
            out,
            dx_output::error_event("incomplete_results", detail, None, None, None),
        )?;
    } else {
        let _ = writeln!(err, "dx: incomplete_results: {detail}");
    }
    Ok(())
}

/// Checks the cell's owned-source line coverage against `--min-coverage`; true when there is no minimum.
fn coverage_threshold(
    minimum: Option<u32>,
    account: Option<&CellAccount>,
    sources: &Sources,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let Some(minimum) = minimum else {
        return Ok(true);
    };
    let Some(account) = account else {
        return Ok(false);
    };
    let covered = account.covered_lines;
    let eligible = account.eligible_lines;
    let errors = sources.invalid.len() + account.errors.len();
    let threshold_ok = errors == 0 && account.meets(minimum);
    let lines = if eligible > 0 {
        let percent = 100.0 * covered as f64 / eligible as f64;
        format!("coverage {percent:.2}% ({covered}/{eligible} lines)")
    } else {
        format!("coverage ({covered}/{eligible} lines)")
    };
    let cell = &account.cell;
    let summary = if errors > 0 {
        let noun = if errors == 1 { "gap" } else { "gaps" };
        format!(
            "{lines} in package {cell} has {errors} coverage source {noun} (see {CODE_COVERAGE_SOURCES_INCOMPLETE}); minimum {minimum}% (coverage_below_minimum)"
        )
    } else if eligible == 0 {
        format!(
            "no executable lines in the owned sources of package {cell} (coverage_below_minimum)"
        )
    } else if threshold_ok {
        format!("{lines} meets minimum {minimum}% in package {cell}")
    } else {
        format!("{lines} below minimum {minimum}% in package {cell} (coverage_below_minimum)")
    };
    if output == OutputMode::Json {
        if !threshold_ok {
            emit_built(
                out,
                dx_output::error_event(CODE_COVERAGE_BELOW_MINIMUM, &summary, None, None, None),
            )?;
        }
    } else {
        let _ = writeln!(err, "dx: {summary}");
    }
    Ok(threshold_ok)
}

/// How many files the text coverage table lists.
const WORST_FILES: usize = 10;

/// Prints the total coverage line (when no minimum prints it) and the least covered files.
fn coverage_summary(
    account: &CellAccount,
    show_total: bool,
    output: OutputMode,
    err: &mut dyn Write,
) {
    let OutputMode::Text { quiet: false } = output else {
        return;
    };
    let covered = account.covered_lines;
    let eligible = account.eligible_lines;
    if show_total {
        if eligible > 0 {
            let percent = 100.0 * covered as f64 / eligible as f64;
            let _ = writeln!(
                err,
                "dx: coverage {percent:.1}% ({covered}/{eligible} lines)"
            );
        } else {
            let _ = writeln!(err, "dx: coverage (0/0 lines)");
        }
    }
    let mut worst: Vec<(&String, u64, u64)> = account
        .file_lines
        .iter()
        .filter(|(_, (covered, eligible))| covered < eligible)
        .map(|(path, (covered, eligible))| (path, *covered, *eligible))
        .collect();
    worst.sort_by(|a, b| {
        (a.1 * b.2)
            .cmp(&(b.1 * a.2))
            .then((b.2 - b.1).cmp(&(a.2 - a.1)))
            .then(a.0.cmp(b.0))
    });
    if worst.is_empty() {
        return;
    }
    let _ = writeln!(err, "dx: least covered files:");
    for (path, covered, eligible) in worst.into_iter().take(WORST_FILES) {
        let percent = 100.0 * covered as f64 / eligible as f64;
        let _ = writeln!(err, "  {percent:5.1}%  {covered}/{eligible}  {path}");
    }
}

/// Accounts every owned source of the run against the merged coverage.
fn account_sources(
    sources: &Sources,
    lcov_documents: &[String],
    workspace: &Path,
) -> dx_lcov::CellAccount {
    dx_lcov::account(&sources.manifests, lcov_documents, &|path| {
        std::fs::read(workspace.join(path))
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|error| dx_lcov::LcovError::Io {
                message: format!("{path}: {error}"),
            })
    })
}

/// The `sources` report: the cell's owned sources with dispositions and the production sources the report never saw.
fn sources_report(sources: &Sources, account: &dx_lcov::CellAccount) -> String {
    let owned: std::collections::BTreeMap<&str, &str> = account
        .files
        .iter()
        .map(|(path, disposition)| {
            let name = match disposition {
                dx_lcov::Disposition::Eligible => "eligible",
                dx_lcov::Disposition::Support => "support",
                dx_lcov::Disposition::Excluded => "excluded",
                dx_lcov::Disposition::Unsupported => "unsupported",
            };
            (path.as_str(), name)
        })
        .chain(account.absent.iter().map(|path| (path.as_str(), "absent")))
        .collect();
    let errors: Vec<&String> = sources.invalid.iter().chain(&account.errors).collect();
    let candidate = sources
        .candidate
        .as_ref()
        .map(|found| serde_json::json!({"sha": found.sha, "dirty": found.dirty}));
    let document = serde_json::json!({
        "record_version": 2,
        "cell": account.cell,
        "candidate": candidate,
        "collector": sources.collector,
        "owned": owned,
        "absent": account.absent,
        "orphans": account.orphans,
        "errors": errors,
    });
    serde_json::to_string_pretty(&document).unwrap_or_default() + "\n"
}

/// Prints the account's counts and every gap; true when none.
/// With `enforce` (`--min-coverage`) each gap is a JSON `error` event instead of a warning notice.
fn coverage_sources(
    sources: &Sources,
    account: &dx_lcov::CellAccount,
    output: OutputMode,
    enforce: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let summary = account.summary();
    let errors: Vec<&String> = sources.invalid.iter().chain(&account.errors).collect();
    let info: fn(&str, String) -> NoticeEvent = NoticeEvent::info;
    let warning: fn(&str, String) -> NoticeEvent = NoticeEvent::warning;
    let unavailable = sources
        .tracked_unavailable
        .iter()
        .map(|why| (warning, CODE_COVERAGE_SOURCES, why));
    let notices = std::iter::once((info, CODE_COVERAGE_SOURCES, &summary))
        .chain(unavailable)
        .chain(
            errors
                .iter()
                .map(|message| (warning, CODE_COVERAGE_SOURCES_INCOMPLETE, *message)),
        );
    for (notice, code, message) in notices {
        match output {
            OutputMode::Json if enforce && code == CODE_COVERAGE_SOURCES_INCOMPLETE => {
                emit_built(out, dx_output::error_event(code, message, None, None, None))?;
            }
            OutputMode::Json => {
                emit_built(out, notice_event(&notice(code, message.clone())))?;
            }
            OutputMode::Text { .. } if code == CODE_COVERAGE_SOURCES => {
                let _ = writeln!(err, "dx: {message}");
            }
            OutputMode::Text { .. } => {
                let _ = writeln!(err, "dx: {code}: {message}");
            }
            _ => {}
        }
    }
    Ok(errors.is_empty())
}

/// Emits the cell's `coverage` event in JSON mode.
fn coverage_event(
    sources: &Sources,
    account: &CellAccount,
    minimum: Option<u32>,
    output: OutputMode,
    out: &mut dyn Write,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    let errors = (sources.invalid.len() + account.errors.len()) as u64;
    let event = dx_output::CoverageEvent {
        cell: account.cell.clone(),
        covered_lines: account.covered_lines,
        eligible_lines: account.eligible_lines,
        raw_lines: account.raw_lines,
        excluded_lines: account.excluded_lines,
        eligible_files: account.eligible_files,
        support_files: account.support_files,
        excluded_files: account.excluded_files,
        unsupported_files: account.unsupported_files,
        minimum,
        passed: errors == 0 && account.meets(minimum.unwrap_or(0)),
        errors,
        dx_version: dx_adopt::DX_VERSION.to_owned(),
        sha: sources.candidate.as_ref().map(|found| found.sha.clone()),
        dirty: sources.candidate.as_ref().is_some_and(|found| found.dirty),
        collector: sources.collector.clone(),
    };
    emit_built(out, dx_output::coverage_event(&event))
}

/// Reads the run's test events; a stream Bazel cut short is a problem only when Bazel failed too.
fn read_events(
    invocation: &Invocation,
    workspace: &Path,
    verb: WorkflowVerb,
    bep: &Path,
    bazel_code: i32,
    query_runner: &dyn QueryRunner,
) -> Result<(TestEvents, Option<String>), (&'static str, String)> {
    let file = match std::fs::File::open(bep) {
        Ok(file) => file,
        Err(error) if bazel_code != 0 => {
            return Ok((
                TestEvents::default(),
                Some(format!("build events are unavailable: {error}")),
            ));
        }
        Err(error) => {
            return Err((
                CODE_UNREADABLE_BEP,
                format!("failed to read build events: {error}"),
            ));
        }
    };
    match collect_test_events(BufReader::new(file), || {
        Some(output_locations(invocation, workspace, verb, query_runner))
    }) {
        Ok(events) => match events.problem.clone() {
            Some(problem) if bazel_code == 0 => {
                Err((CODE_INVALID_BEP, format!("invalid build events: {problem}")))
            }
            Some(problem) => Ok((events, Some(format!("build events end early: {problem}")))),
            None => Ok((events, None)),
        },
        Err(error) if bazel_code != 0 => Ok((
            TestEvents::default(),
            Some(format!("invalid build events: {error}")),
        )),
        Err(error) => Err((CODE_INVALID_BEP, format!("invalid build events: {error}"))),
    }
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
        failure_reports,
        results_reports,
        run_id,
        stdout_report,
        bazel_code,
        query_runner,
        results_complete,
        actions,
        whole_repo,
    } = request;
    let failure_count = if bazel_code != 0 {
        match report_build_failures(bep, invocation.output, out, err) {
            Ok(reported) => reported.count,
            Err(exit) => return kept_child(err, bazel_code, exit),
        }
    } else {
        0
    };
    let failure_reports_ok = match write_failure_reports(
        invocation,
        workspace,
        bep,
        bazel_code,
        failure_reports,
        out,
        err,
    ) {
        Ok(landed) => landed,
        Err(exit) => return kept_child(err, bazel_code, exit),
    };
    let read = read_events(invocation, workspace, verb, bep, bazel_code, query_runner);
    let sources = if verb == WorkflowVerb::Coverage
        && read.is_ok()
        && (bazel_code == 0 || bazel_code == BAZEL_TESTS_FAILED)
    {
        collect_sources(bep, workspace)
    } else {
        Sources::default()
    };
    let _ = std::fs::remove_file(bep);
    let (events, stream_problem) = match read {
        Ok(read) => read,
        Err((code, message)) => return operational(invocation, out, err, code, &message),
    };
    let reader = FsArtifacts;
    let mut evidence = Evidence::default();
    if let Some(problem) = stream_problem {
        evidence.record(problem);
    }
    let summaries = summary_statuses(&events);
    let Collected {
        mut suites,
        lcov_documents,
        mut sources,
        mut reported,
        gap,
        no_collector,
    } = if verb == WorkflowVerb::Test {
        collect_test_xml(&events, &summaries, &reader, evidence, err)
    } else {
        collect_lcov(&events, &summaries, &reader, evidence, sources)
    };
    let complete = (bazel_code == 0 || bazel_code == BAZEL_TESTS_FAILED) && gap.is_none();
    let detail = gap.unwrap_or_default();
    let nothing_ran = failure_count > 0 && events.results.is_empty();
    if verb == WorkflowVerb::Test && !complete && !nothing_ran {
        suites.push(junit_infrastructure_case(&detail));
    }
    mark_superseded_attempts(&mut reported);
    let counts = match emit_outcomes(invocation.output, out, err, &reported, &events.filtered) {
        Ok(counts) => counts,
        Err(exit) => return kept_child(err, bazel_code, exit),
    };
    let groups = group_skipped(&events.skipped, invocation, workspace, query_runner);
    if let Err(exit) = skipped_notice(invocation.output, out, err, &groups) {
        return kept_child(err, bazel_code, exit);
    }
    let mut reports_ok = failure_reports_ok;
    let mut sink = ReportSink {
        out: &mut *out,
        err: &mut *err,
        output: invocation.output,
        workspace,
        stdout_report,
    };
    if !results_reports.is_empty() {
        let document = results_document(&reported, complete, bazel_code, run_id);
        for planned in results_reports {
            match sink.write("results", &planned.destination, &document, complete) {
                Ok(landed) => reports_ok &= landed,
                Err(exit) => return kept_child(err, bazel_code, exit),
            }
        }
    }
    let mut account = (verb == WorkflowVerb::Coverage && detail.is_empty())
        .then(|| account_sources(&sources, &lcov_documents, workspace));
    if let Some(account) = account.as_mut() {
        sources.candidate = candidate(workspace, query_runner);
        if whole_repo {
            match tracked_orphans(workspace, query_runner, &invocation.bazel_startup_options) {
                Ok(orphans) => account.record_orphans(orphans),
                Err(Unavailable::NotGit) => {
                    sources.tracked_unavailable =
                        Some("tracked sources unavailable: not a Git work tree".to_owned());
                }
                Err(Unavailable::Failed(why)) => account
                    .errors
                    .push(format!("tracked sources unavailable: {why}")),
            }
        }
    }
    let sources_document = account
        .as_ref()
        .map(|account| sources_report(&sources, account));
    for planned in planned_reports {
        let name = planned.format.name();
        let document = match render_report(
            verb,
            &suites,
            &lcov_documents,
            &detail,
            planned.format,
            sources_document.as_deref(),
        ) {
            Ok(document) => document,
            Err(reason) => {
                reports_ok = false;
                let message = format!("failed to render {name} report: {reason}");
                let next = (!detail.is_empty() && reason == detail)
                    .then_some(super::guidance::EVIDENCE_GAP_NEXT);
                if let Err(exit) = sink.failed(&message, next) {
                    return kept_child(err, bazel_code, exit);
                }
                continue;
            }
        };
        let complete_so_far = complete && reports_ok;
        match sink.write(name, &planned.destination, &document, complete_so_far) {
            Ok(landed) => reports_ok &= landed,
            Err(exit) => return kept_child(err, bazel_code, exit),
        }
    }
    if let Err(exit) = announce_incomplete(invocation.output, out, err, &detail) {
        return kept_child(err, bazel_code, exit);
    }
    let collector_ok = match announce_no_collector(
        invocation.output,
        out,
        err,
        &no_collector,
        dx_output::meets_threshold(dx_output::Severity::Warning, invocation.fail_on),
    ) {
        Ok(ok) => ok,
        Err(exit) => return kept_child(err, bazel_code, exit),
    };
    let threshold_ok = if verb == WorkflowVerb::Coverage {
        match coverage_threshold(
            invocation.min_coverage,
            account.as_ref(),
            &sources,
            invocation.output,
            out,
            err,
        ) {
            Ok(met) => met,
            Err(exit) => return kept_child(err, bazel_code, exit),
        }
    } else {
        true
    };
    if verb == WorkflowVerb::Coverage
        && let Some(account) = &account
    {
        coverage_summary(
            account,
            invocation.min_coverage.is_none(),
            invocation.output,
            err,
        );
    }
    let sources_ok = match &account {
        Some(account) => {
            let enforce = invocation.min_coverage.is_some();
            match coverage_sources(&sources, account, invocation.output, enforce, out, err) {
                Ok(accounted) => accounted || !enforce,
                Err(exit) => return kept_child(err, bazel_code, exit),
            }
        }
        None => true,
    };
    if let Some(account) = &account
        && let Err(exit) = coverage_event(
            &sources,
            account,
            invocation.min_coverage,
            invocation.output,
            out,
        )
    {
        return kept_child(err, bazel_code, exit);
    }
    let changed_ok = if verb == WorkflowVerb::Coverage {
        let gate = super::changed_coverage::ChangedGate {
            minimum: invocation.min_changed_line_coverage,
            account: account.as_ref(),
            lcov_documents: &lcov_documents,
            workspace,
            runner: query_runner,
            output: invocation.output,
        };
        match super::changed_coverage::changed_line_threshold(&gate, out, err) {
            Ok(met) => met,
            Err(exit) => return kept_child(err, bazel_code, exit),
        }
    } else {
        true
    };
    let code = if bazel_code != 0 {
        bazel_code
    } else if complete && reports_ok && threshold_ok && collector_ok && sources_ok && changed_ok {
        0
    } else {
        1
    };
    *results_complete = complete && reports_ok;
    let no_tests = bazel_code == BAZEL_NO_TESTS && reported.is_empty();
    if no_tests {
        let _ = writeln!(err, "dx: {CODE_NO_TESTS}: no test targets match the scope");
        if let Some(next) = super::guidance::next_action(CODE_NO_TESTS) {
            let _ = writeln!(err, "{next}");
        }
    }
    if invocation.output == OutputMode::Json {
        if bazel_code == BAZEL_TESTS_FAILED && counts.failed > 0 {
            let message = if counts.failed == 1 {
                "1 test failed; read the test_outcome events".to_owned()
            } else {
                format!(
                    "{} tests failed; read the test_outcome events",
                    counts.failed
                )
            };
            if let Err(exit) = emit_built(
                out,
                bazel_exit_event(CODE_TESTS_FAILED, &message, bazel_code, "execute"),
            ) {
                return kept_child(err, bazel_code, exit);
            }
        } else if no_tests {
            if let Err(exit) = emit_built(
                out,
                bazel_exit_event(
                    CODE_NO_TESTS,
                    "no test targets match the scope",
                    bazel_code,
                    "execute",
                ),
            ) {
                return kept_child(err, bazel_code, exit);
            }
        } else if bazel_code != 0
            && let Err(exit) = emit_built(
                out,
                bazel_exit_event(
                    bazel_failure_code(bazel_code),
                    &format!(
                        "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                        invocation.command.name(),
                    ),
                    bazel_code,
                    "execute",
                ),
            )
        {
            return kept_child(err, bazel_code, exit);
        }
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(*results_complete),
                run_id: (!results_reports.is_empty()).then(|| run_id.to_owned()),
                tests: (!reported.is_empty()).then_some(counts),
                actions,
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return kept_child(err, bazel_code, exit);
        }
    }
    code
}

#[cfg(test)]
#[path = "test_reports_tests.rs"]
mod tests;
