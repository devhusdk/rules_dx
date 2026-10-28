use super::build_failures::report_build_failures;
use super::common::*;
use super::coverage_candidate::{candidate, tracked_orphans, Candidate, Unavailable};
use super::failure_reports::write_failure_reports;
use dx_bep::{
    collect_test_events, ArtifactReader, OutputLocations, TestEvents, TestResultFile,
    TestResultRecord,
};
use dx_lcov::{parse_source_manifest, CellAccount, SourceManifest, SOURCES_OUTPUT_GROUP};
use dx_output::{
    command_finished, notice_event, report_event, test_outcome_event, test_result_record,
    ActionCounts, FinishedCounts, NoticeEvent, OutputMode, TestCase, TestCounts, TestOutcome,
};
use dx_plan::args::Invocation;
use dx_plan::plan::WorkflowVerb;
use dx_plan::reports::{
    junit_infrastructure_case, parse_test_xml, render_junit, validate_lcov, Destination, JunitCase,
    JunitMessage, PlannedReport, ReportError, StandardFormat,
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
    argv.extend(invocation.raw_bazel_options().iter().cloned());
    argv.push("bazel-testlogs".to_owned());
    argv.push("execution_root".to_owned());
    argv.push("output_path".to_owned());
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

/// Points a result that did not pass at its test.log and keeps the log's tail.
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

/// One case read from a result's native report.
fn test_case(case: &JunitCase) -> TestCase {
    let message = case_message(case).map(|text| bounded(text, CASE_MESSAGE_LIMIT).0);
    TestCase {
        classname: case.classname.clone(),
        name: case.name.clone(),
        outcome: case_outcome(case).to_owned(),
        message,
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
    /// Sorted, deduplicated labels that ran under coverage without a collector.
    no_collector: Vec<String>,
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

/// Labels tagged as having no coverage collector that produced a test result, once each.
fn no_collector_labels(events: &TestEvents) -> Vec<String> {
    let mut labels: Vec<String> = events
        .results
        .iter()
        .filter(|result| events.no_coverage_collector.contains(&result.label))
        .map(|result| result.label.clone())
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

/// Reports each test that ran under coverage with no collector; true when there were none.
fn announce_no_collector(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    labels: &[String],
) -> Result<bool, i32> {
    for label in labels {
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
    Ok(labels.is_empty())
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
        }
    }
    Ok(counts)
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
            "coverage_below_minimum: {lines} in cell {cell} has {errors} coverage source {noun} (see {CODE_COVERAGE_SOURCES_INCOMPLETE}); minimum {minimum}%"
        )
    } else if eligible == 0 {
        format!("coverage_below_minimum: no executable lines in the owned sources of cell {cell}")
    } else if threshold_ok {
        format!("{lines} meets minimum {minimum}% in cell {cell}")
    } else {
        format!("coverage_below_minimum: {lines} below minimum {minimum}% in cell {cell}")
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

/// Accounts every owned source of the run against the merged coverage.
fn account_sources(
    sources: &Sources,
    lcov_documents: &[String],
    workspace: &Path,
) -> dx_lcov::CellAccount {
    dx_lcov::account(&sources.manifests, lcov_documents, &|path| {
        std::fs::read_to_string(workspace.join(path)).map_err(|error| dx_lcov::LcovError::Io {
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
    let collector_ok = match announce_no_collector(invocation.output, out, err, &no_collector) {
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
    if let Some(account) = &account {
        if let Err(exit) = coverage_event(
            &sources,
            account,
            invocation.min_coverage,
            invocation.output,
            out,
        ) {
            return kept_child(err, bazel_code, exit);
        }
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
        } else if bazel_code != 0 {
            if let Err(exit) = emit_built(out, bazel_exit_event(
                CODE_BAZEL_FAILED,
                &format!(
                    "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                    invocation.command.name(),
                ),
                bazel_code,
                "execute",
            )) {
return kept_child(err, bazel_code, exit);
}
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
mod tests {
    #[test]
    fn log_tail_keeps_last_forty_lines() {
        let text: String = (1..=50).map(|n| format!("line {n}\n")).collect();
        let want: Vec<String> = (11..=50).map(|n| format!("line {n}")).collect();
        assert_eq!(super::log_tail(text.as_bytes()), Some(want.join("\n")));
    }

    #[test]
    fn log_tail_returns_short_logs_whole() {
        assert_eq!(super::log_tail(b"a\nb\nc\n"), Some("a\nb\nc".to_owned()));
    }

    #[test]
    fn log_tail_drops_partial_first_line_when_capped() {
        let lines: Vec<String> = (0..40)
            .map(|n| format!("{n:02}{}", "x".repeat(298)))
            .collect();
        let tail = super::log_tail(lines.join("\n").as_bytes()).unwrap();
        assert!(tail.len() <= super::LOG_TAIL_BYTES);
        let got: Vec<&str> = tail.split('\n').collect();
        assert!(got.iter().all(|l| l.len() == 300));
        assert_eq!(
            got,
            lines[40 - got.len()..]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        assert!(!tail.starts_with('\n'));
    }

    #[test]
    fn log_tail_cuts_one_oversize_multibyte_line_at_char_boundary() {
        let tail = super::log_tail("日".repeat(3500).as_bytes()).unwrap();
        assert!(!tail.is_empty() && tail.len() <= super::LOG_TAIL_BYTES);
    }

    #[test]
    fn log_tail_is_none_for_blank_input() {
        assert_eq!(super::log_tail(b""), None);
        assert_eq!(super::log_tail(b"  \n\t\n\n"), None);
    }

    #[test]
    fn log_tail_tolerates_invalid_utf8() {
        assert!(super::log_tail(b"ok\n\xff\xfe bad\n").is_some());
    }

    use super::super::test_support::*;
    use super::stable_exec_path;
    use std::path::Path;

    #[test]
    fn coverage_sources_returns_the_write_code_when_stdout_fails() {
        let account = dx_lcov::CellAccount::default();
        let got = super::coverage_sources(
            &super::Sources::default(),
            &account,
            dx_output::OutputMode::Json,
            false,
            &mut BrokenPipe,
            &mut Vec::new(),
        );
        assert_eq!(got, Err(dx_process::broken_pipe_code()));
    }

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
        let harness = owning_a(harness);
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
        let harness = owning_a(harness);
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    const HALF_LCOV: &str = "SF:src/a.py\nDA:1,1\nDA:2,0\nend_of_record\n";

    #[test]
    fn coverage_min_coverage_passes_at_threshold() {
        let harness = coverage_harness("cov-threshold-ok", MINIMAL_LCOV.as_bytes());
        let harness = owning_a(harness);
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("meets minimum 100%"), "{err}");
    }

    #[test]
    fn coverage_min_coverage_fails_below_threshold() {
        let harness = coverage_harness("cov-threshold-low", HALF_LCOV.as_bytes());
        let harness = with_sources(harness, PY_SOURCES);
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("coverage_below_minimum"), "{err}");
        assert!(err.contains("50.00% (1/2 lines)"), "{err}");
    }

    #[test]
    fn coverage_min_coverage_failure_reports_json_event() {
        let harness = coverage_harness("cov-threshold-json", HALF_LCOV.as_bytes());
        let harness = with_sources(harness, PY_SOURCES);
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=80"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("coverage_below_minimum"), "{out}");
    }

    #[test]
    fn coverage_min_coverage_fails_without_executable_lines() {
        let harness = coverage_harness("cov-threshold-empty", b"SF:src/a.py\nend_of_record\n");
        let harness = with_sources(harness, PY_SOURCES);
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
        let harness = with_sources(harness, "v1 cell linux_x86_64\nprod source src/lib.rs\n");
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=80"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("coverage_below_minimum"), "{err}");
    }

    const SOURCES_ASPECT: &str = "//quality:coverage_sources.bzl%dx_coverage_sources_aspect";

    const PY_SOURCES: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";

    /// Adds one `//a:lib` source manifest to a harness's build events.
    fn with_sources(harness: Harness, manifest: &str) -> Harness {
        let uri = write_bep_artifact(&harness, "sources.txt", manifest.as_bytes());
        let mut lines = vec![
            sources_set("s1", &uri),
            sources_completed("//a:lib", true, "s1"),
        ];
        lines.extend(harness.raw_bep.clone().unwrap_or_default());
        Harness {
            raw_bep: Some(lines),
            ..harness
        }
    }

    fn sources_completed(label: &str, success: bool, set: &str) -> String {
        let mut completed = serde_json::json!({"success": success});
        if success {
            completed["outputGroup"] =
                serde_json::json!([{"name": "dx_coverage_sources", "fileSets": [{"id": set}]}]);
        }
        serde_json::json!({
            "id": {"targetCompleted": {"label": label, "aspect": SOURCES_ASPECT}},
            "completed": completed,
        })
        .to_string()
    }

    fn sources_set(set: &str, uri: &str) -> String {
        serde_json::json!({
            "id": {"namedSet": {"id": set}},
            "namedSetOfFiles": {"files": [{"name": "lib.dx_coverage_sources.txt", "uri": uri}]},
        })
        .to_string()
    }

    /// Prepends a manifest owning `src/a.py`, so `--min-coverage` can account the run's sources.
    fn owning_a(mut harness: Harness) -> Harness {
        let uri = write_bep_artifact(
            &harness,
            "sources.txt",
            b"v1 cell linux_x86_64\nprod source src/a.py\n",
        );
        let lines = [
            sources_set("s1", &uri),
            sources_completed("//a:lib", true, "s1"),
        ];
        if let Some(bep) = harness.raw_bep.as_mut() {
            bep.splice(0..0, lines);
        }
        harness
    }

    fn sources_harness(name: &str, manifest: &str, lcov: &str) -> Harness {
        let harness = Harness::new(name);
        let lcov_uri = write_bep_artifact(&harness, "coverage.dat", lcov.as_bytes());
        let manifest_uri = write_bep_artifact(&harness, "sources.txt", manifest.as_bytes());
        Harness {
            raw_bep: Some(vec![
                sources_set("s1", &manifest_uri),
                sources_completed("//a:lib", true, "s1"),
                test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
            ]),
            ..harness
        }
    }

    #[test]
    fn coverage_absent_production_source_is_a_notice() {
        let harness = sources_harness(
            "cov-sources-absent",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
            MINIMAL_LCOV,
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains(
                "dx: coverage_sources_incomplete: production source absent from coverage report: src/b.py"
            ),
            "{err}"
        );
    }

    #[test]
    fn coverage_sources_report_lists_owned_and_absent() {
        let harness = sources_harness(
            "cov-sources-report",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\ntest source src/a_test.py\n",
            MINIMAL_LCOV,
        );
        let path = harness.workspace.join("sources.json");
        let (code, _, err) = harness.run(&[
            "coverage",
            "--output=text",
            &format!("--report=sources={}", path.display()),
        ]);
        assert_eq!(code, 0, "{err}");
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("sources report")).unwrap();
        assert_eq!(document["record_version"], serde_json::json!(2));
        assert_eq!(document["orphans"], serde_json::json!([]));
        assert_eq!(document["absent"], serde_json::json!(["src/b.py"]));
        assert_eq!(document["owned"]["src/a.py"], serde_json::json!("eligible"));
        assert_eq!(
            document["owned"]["src/a_test.py"],
            serde_json::json!("support")
        );
    }

    const ORPHAN_MANIFEST: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";
    const ORPHAN_MESSAGE: &str = "tracked source not owned by any Bazel target: src/orphan.py";

    /// Scripts a clean Git checkout whose tracked files include a used and an unused source.
    fn script_checkout(harness: &Harness) {
        std::fs::create_dir_all(harness.workspace.join("src")).expect("mkdir");
        std::fs::write(harness.workspace.join("src/BUILD.bazel"), "").expect("build");
        harness.query.script_checkout_git(0, "true\n");
        harness.query.script_checkout_git(0, "abc123\n");
        harness.query.script_checkout_git(0, "");
        harness.query.script_checkout_git(0, "true\n");
        harness
            .query
            .script_checkout_git(0, "src/a.py\0src/orphan.py\0src/BUILD.bazel\0");
        harness.query.script_graph(&[dx_testing::cli::graph_rule(
            "//src:lib",
            &["//src:a.py"],
            &[],
        )]);
    }

    #[test]
    fn coverage_tracked_orphan_fails_min_coverage() {
        let harness = sources_harness("cov-orphan-min", ORPHAN_MANIFEST, MINIMAL_LCOV);
        script_checkout(&harness);
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(ORPHAN_MESSAGE), "{err}");
    }

    #[test]
    fn coverage_tracked_orphan_is_a_notice_without_minimum() {
        let harness = sources_harness("cov-orphan-notice", ORPHAN_MANIFEST, MINIMAL_LCOV);
        script_checkout(&harness);
        let path = harness.workspace.join("sources.json");
        let (code, _, err) = harness.run(&[
            "coverage",
            "--output=text",
            &format!("--report=sources={}", path.display()),
        ]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains(ORPHAN_MESSAGE), "{err}");
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("sources report")).unwrap();
        assert_eq!(document["orphans"], serde_json::json!(["src/orphan.py"]));
        assert_eq!(
            document["candidate"],
            serde_json::json!({"sha": "abc123", "dirty": false})
        );
    }

    #[test]
    fn coverage_tracked_check_skips_scoped_runs() {
        let harness = sources_harness("cov-orphan-scoped", ORPHAN_MANIFEST, MINIMAL_LCOV);
        script_checkout(&harness);
        let (code, _, err) = harness.run(&[
            "coverage",
            "--output=text",
            "--min-coverage=100",
            "//src:lib",
        ]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains(ORPHAN_MESSAGE), "{err}");
    }

    #[test]
    fn coverage_tracked_check_skips_non_git_workspace() {
        let harness = sources_harness("cov-orphan-nogit", ORPHAN_MANIFEST, MINIMAL_LCOV);
        harness.query.script_checkout_git(128, "");
        harness.query.script_checkout_git(128, "");
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("dx: tracked sources unavailable: not a Git work tree"),
            "{err}"
        );
    }

    #[test]
    fn coverage_tracked_query_failure_fails_min_coverage() {
        let harness = sources_harness("cov-orphan-query", ORPHAN_MANIFEST, MINIMAL_LCOV);
        script_checkout(&harness);
        harness.query.outputs.borrow_mut().clear();
        harness.query.script_failure(1, "ERROR: boom\n");
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("tracked sources unavailable:"), "{err}");
    }

    #[test]
    fn coverage_event_carries_sha_dirty_and_collector() {
        let mut harness = sources_harness("cov-event-candidate", ORPHAN_MANIFEST, MINIMAL_LCOV);
        script_checkout(&harness);
        harness.raw_bep.as_mut().expect("bep").insert(
            0,
            r#"{"id":{"started":{}},"started":{"buildToolVersion":"9.0.0"}}"#.to_owned(),
        );
        let (code, out, err) = harness.run(&["coverage", "--output=json"]);
        assert_eq!(code, 0, "{err}");
        let event = out
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "coverage")
            .expect("coverage event");
        assert_eq!(event["sha"], "abc123");
        assert_eq!(event["dirty"], false);
        assert_eq!(event["collector"], "bazel 9.0.0");
    }

    #[test]
    fn coverage_zero_hit_line_fails_min_coverage() {
        let harness = sources_harness("cov-sources-zero-hit", PY_SOURCES, HALF_LCOV);
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "dx: coverage_below_minimum: coverage 50.00% (1/2 lines) below minimum 100% in cell linux_x86_64"
            ),
            "{err}"
        );
    }

    #[test]
    fn coverage_report_without_manifests_fails_min_coverage() {
        let harness = coverage_harness("cov-sources-none", MINIMAL_LCOV.as_bytes());
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("coverage_below_minimum"), "{err}");
        assert!(
            err.contains("no coverage source manifests were reported"),
            "{err}"
        );
    }

    #[test]
    fn coverage_event_reports_the_cell_account() {
        let harness = sources_harness(
            "cov-sources-event",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
            MINIMAL_LCOV,
        );
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
        assert_eq!(code, 1, "{out}");
        let events: Vec<serde_json::Value> = out
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let coverage: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == "coverage")
            .collect();
        assert_eq!(coverage.len(), 1, "{out}");
        let event = coverage[0];
        assert_eq!(event["cell"], "linux_x86_64", "{event}");
        assert_eq!(event["covered_lines"], 1, "{event}");
        assert_eq!(event["eligible_lines"], 1, "{event}");
        assert_eq!(event["minimum"], 100, "{event}");
        assert_eq!(event["passed"], false, "{event}");
        assert_eq!(event["errors"], 1, "{event}");
        assert_eq!(event["dx_version"], dx_adopt::DX_VERSION, "{event}");
        assert!(
            events
                .iter()
                .any(|event| event["code"] == "coverage_below_minimum"),
            "{out}"
        );
    }

    #[test]
    fn coverage_event_passes_a_clean_cell_without_minimum() {
        let harness = sources_harness("cov-sources-event-clean", PY_SOURCES, MINIMAL_LCOV);
        let (code, out, _) = harness.run(&["coverage", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let event = out
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "coverage")
            .unwrap_or_else(|| panic!("no coverage event: {out}"));
        assert_eq!(event["passed"], true, "{event}");
        assert!(event.get("minimum").is_none(), "{event}");
    }

    #[test]
    fn coverage_sources_counts_line_and_silent_test_sources() {
        let harness = sources_harness(
            "cov-sources-counts",
            "v1 cell linux_x86_64\nprod source src/a.py\ntest source src/a_test.py\n",
            MINIMAL_LCOV,
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains(
                "dx: coverage sources linux_x86_64: 1 raw, 0 excluded, 1 eligible, 1 covered lines; 1 eligible, 1 support, 0 excluded, 0 unsupported files"
            ),
            "{err}"
        );
        assert!(!err.contains("coverage_sources_incomplete"), "{err}");
    }

    #[test]
    fn coverage_sources_notice_is_a_json_event() {
        let harness = sources_harness(
            "cov-sources-json",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
            MINIMAL_LCOV,
        );
        let (code, out, _) = harness.run(&["coverage", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let notice = out
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["code"] == "coverage_sources_incomplete")
            .unwrap_or_else(|| panic!("no notice: {out}"));
        assert_eq!(notice["event"], "notice", "{notice}");
        assert_eq!(notice["level"], "warning", "{notice}");
        assert_eq!(
            notice["message"],
            "production source absent from coverage report: src/b.py"
        );
        let counts = out
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["code"] == "coverage_sources")
            .unwrap_or_else(|| panic!("no counts notice: {out}"));
        assert_eq!(counts["level"], "info", "{counts}");
        assert!(
            counts["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("coverage sources linux_x86_64: ")),
            "{counts}"
        );
    }

    #[test]
    fn coverage_invalid_source_manifest_is_a_notice() {
        let harness = sources_harness("cov-sources-invalid", "v1 cell unknown\n", MINIMAL_LCOV);
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("dx: coverage_sources_incomplete: invalid coverage source manifest"),
            "{err}"
        );
        assert!(err.contains("unknown cell"), "{err}");
    }

    #[test]
    fn coverage_absent_production_source_fails_min_coverage() {
        let harness = sources_harness(
            "cov-sources-absent-gate",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
            MINIMAL_LCOV,
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(!err.contains("meets minimum"), "{err}");
        assert!(
            err.contains(
                "dx: coverage_below_minimum: coverage 100.00% (1/1 lines) in cell linux_x86_64 has 1 coverage source gap"
            ),
            "{err}"
        );
        assert!(
            err.contains(
                "dx: coverage_sources_incomplete: production source absent from coverage report: src/b.py"
            ),
            "{err}"
        );
    }

    #[test]
    fn coverage_unowned_report_path_fails_min_coverage() {
        let harness = sources_harness(
            "cov-sources-unowned-gate",
            "v1 cell linux_x86_64\nprod source src/other.py\n",
            "SF:src/a.py\nDA:1,1\nend_of_record\nSF:src/other.py\nDA:1,1\nend_of_record\n",
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "dx: coverage_sources_incomplete: coverage report path not owned by any target: src/a.py"
            ),
            "{err}"
        );
    }

    #[test]
    fn coverage_invalid_source_manifest_fails_min_coverage() {
        let harness = sources_harness(
            "cov-sources-invalid-gate",
            "v1 cell unknown\n",
            MINIMAL_LCOV,
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("dx: coverage_sources_incomplete: invalid coverage source manifest"),
            "{err}"
        );
    }

    #[test]
    fn coverage_sources_error_is_a_json_error_under_min_coverage() {
        let harness = sources_harness(
            "cov-sources-json-gate",
            "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
            MINIMAL_LCOV,
        );
        let (code, out, _) = harness.run(&["coverage", "--output=json", "--min-coverage=100"]);
        assert_eq!(code, 1, "{out}");
        let error = out
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["code"] == "coverage_sources_incomplete")
            .unwrap_or_else(|| panic!("no error: {out}"));
        assert_eq!(error["event"], "error", "{error}");
        assert_eq!(
            error["message"],
            "production source absent from coverage report: src/b.py"
        );
    }

    #[test]
    fn coverage_test_only_sources_pass_min_coverage() {
        let harness = sources_harness(
            "cov-sources-test-only-gate",
            "v1 cell linux_x86_64\nprod source src/a.py\ntest source src/a_test.py\n",
            MINIMAL_LCOV,
        );
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("coverage_sources_incomplete"), "{err}");
    }

    #[test]
    fn coverage_missing_source_manifest_blocks_min_coverage() {
        let harness = Harness::new("cov-sources-missing");
        let lcov_uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                sources_completed("//a:lib", false, "s1"),
                test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--min-coverage=100"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "dx: incomplete_results: //a:lib did not report its coverage source manifest"
            ),
            "{err}"
        );
        assert!(!err.contains("meets minimum"), "{err}");
    }

    #[test]
    fn coverage_unreadable_source_manifest_is_incomplete() {
        let harness = Harness::new("cov-sources-unreadable");
        let lcov_uri = write_bep_artifact(&harness, "coverage.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                sources_set("s1", &missing_uri("sources.txt")),
                sources_completed("//a:lib", true, "s1"),
                test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("coverage source manifests are unreadable"),
            "{err}"
        );
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
    fn test_missing_bep_keeps_bazel_code() {
        let mut harness = Harness::new("test-nobep-failed");
        harness.skip_bep = true;
        harness.bazel_code = 2;
        let (code, out, _) = harness.run(&[
            "test",
            "--output=json",
            "--report=results=r.json",
            "--report=junit=j.xml",
        ]);
        assert_eq!(code, 2, "{out}");
        let events = json_events(&out);
        assert!(
            events_of_kind(&events, "notice")
                .iter()
                .any(|event| event["code"] == "build_events_incomplete"),
            "{out}"
        );
        let errors: Vec<_> = events_of_kind(&events, "error")
            .into_iter()
            .map(|event| event["code"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert!(errors.contains(&"incomplete_results".to_owned()), "{out}");
        assert!(errors.contains(&"bazel_failed".to_owned()), "{out}");
        assert!(!out.contains("unreadable_bep"), "{out}");
        assert_eq!(
            event(&events, "command_finished")["exit_code"],
            serde_json::json!(2)
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("r.json")).expect("results report"),
        )
        .expect("results json");
        assert_eq!(document["exit_code"], serde_json::json!(2));
        assert_eq!(document["complete"], serde_json::json!(false));
        assert_eq!(document["results"], serde_json::json!([]));
        let junit = std::fs::read_to_string(harness.workspace.join("j.xml")).expect("junit");
        assert!(junit.contains("dx.infrastructure"), "{junit}");
        assert!(junit.contains("build events are unavailable"), "{junit}");
    }

    #[test]
    fn test_cut_off_bep_keeps_bazel_code_and_results() {
        let harness = Harness::new("test-cutbep-failed");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            bazel_code: 37,
            unterminated_bep: true,
            raw_bep: Some(vec![
                test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
                String::from(r#"{"id":{"testRes"#),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&[
            "test",
            "--output=json",
            "--report=results=r.json",
            "--report=junit=j.xml",
        ]);
        assert_eq!(code, 37, "{out}");
        assert!(!out.contains("invalid_bep"), "{out}");
        let events = json_events(&out);
        let incomplete = events_of_kind(&events, "error")
            .into_iter()
            .find(|event| event["code"] == "incomplete_results")
            .expect("incomplete_results");
        assert!(
            incomplete["message"]
                .as_str()
                .is_some_and(|message| message.contains("build events end early: line 2")),
            "{out}"
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("r.json")).expect("results report"),
        )
        .expect("results json");
        assert_eq!(document["exit_code"], serde_json::json!(37));
        assert_eq!(document["complete"], serde_json::json!(false));
        let results = document["results"].as_array().expect("results");
        assert_eq!(results.len(), 1, "{document}");
        assert_eq!(results[0]["target"], serde_json::json!("//a:t"));
        let junit = std::fs::read_to_string(harness.workspace.join("j.xml")).expect("junit");
        assert!(junit.contains("<testsuite name=\"//a:t\""), "{junit}");
    }

    #[test]
    fn test_cut_off_bep_with_bazel_success_is_operational() {
        let harness = Harness::new("test-cutbep-passed");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            unterminated_bep: true,
            raw_bep: Some(vec![
                test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
                String::from(r#"{"id":{"testRes"#),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("invalid_bep"), "{err}");
    }

    #[test]
    fn coverage_missing_bep_keeps_bazel_code() {
        let mut harness = coverage_harness("cov-nobep-failed", MINIMAL_LCOV.as_bytes());
        harness.skip_bep = true;
        harness.bazel_code = 2;
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 2, "{err}");
        assert!(!err.contains("unreadable_bep"), "{err}");
        assert!(err.contains("build events are unavailable"), "{err}");
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
    fn test_small_missing_fraction_fails_when_bazel_passed() {
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
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("incomplete_results: 2 missing or invalid results"),
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
    fn test_one_missing_and_one_invalid_xml_fails_when_bazel_passed() {
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
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("incomplete_results: 2 missing or invalid results"),
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
    fn coverage_report_without_lcov_names_the_gap_not_the_destination() {
        let harness = Harness::new("cov-gap-report");
        let xml = write_bep_artifact(&harness, "x.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), xml)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("report_failed"), "{err}");
        assert!(err.contains("no coverage.dat"), "{err}");
        assert!(err.contains("fix the test or coverage problem"), "{err}");
        assert!(!err.contains("writable"), "{err}");
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
        let harness = owning_a(harness);
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
        let harness = owning_a(harness);
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
        let err = dx_plan::args::parse(
            &["test", "--output=diff", "--report=junit=out.xml"]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .expect_err("diff rejected");
        assert_eq!(
            err,
            dx_plan::args::ArgsError::UnsupportedOption {
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
    fn test_file_outputs_skip_bazel_info() {
        let harness = Harness::new("test-file-no-info");
        let uri = write_bep_artifact(&harness, "test.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("tests=\"1\""), "{text}");
        assert!(harness.query.info_calls.borrow().is_empty());
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
        assert!(argv.contains(&"output_path".to_owned()), "{argv:?}");
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
    fn test_bytestream_outputs_resolve_through_configuration_mnemonic() {
        let harness = Harness::new("test-info-mnemonic");
        let out = harness.temp.join("out");
        harness.query.script_info(&format!(
            "bazel-testlogs: {}\nexecution_root: {}\noutput_path: {}\n",
            out.join("k8-fastbuild").join("testlogs").display(),
            harness.temp.join("execroot").display(),
            out.display(),
        ));
        let root = out
            .join("k8-fastbuild-ST-x")
            .join("testlogs")
            .join("a")
            .join("t");
        std::fs::create_dir_all(&root).expect("testlogs dir");
        std::fs::write(root.join("test.xml"), MINIMAL_TEST_XML).expect("test.xml");
        let configuration = serde_json::json!({
            "id": {"configuration": {"id": "cfgST"}},
            "configuration": {"mnemonic": "k8-fastbuild-ST-x"},
        })
        .to_string();
        let harness = Harness {
            raw_bep: Some(vec![
                configuration,
                test_result_full_line(
                    "//a:t",
                    1,
                    1,
                    1,
                    "PASSED",
                    Some("cfgST"),
                    None,
                    None,
                    &[(
                        String::from("test.xml"),
                        "bytestream://remote.buildbuddy.io/blobs/abc/10".to_owned(),
                    )],
                ),
            ]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("incomplete_results"), "{out}");
        let document = std::fs::read(harness.workspace.join("out.xml")).expect("junit");
        let text = String::from_utf8(document).expect("utf8");
        assert!(text.contains("tests=\"1\""), "{text}");
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
    fn two_runs_of_one_label_render_distinct_case_names() {
        let harness = Harness::new("test-junit-runs");
        let first = write_bep_artifact(&harness, "run1.xml", MINIMAL_TEST_XML.as_bytes());
        let second = write_bep_artifact(&harness, "run2.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_identity_line("//a:t", 1, 1, 1, &[(String::from("test.xml"), first)]),
                test_result_identity_line("//a:t", 2, 1, 1, &[(String::from("test.xml"), second)]),
            ]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        let text = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
        assert!(
            text.contains(r#"<testcase name="passes [run=0]""#),
            "{text}"
        );
        assert!(
            text.contains(r#"<testcase name="passes [run=1]""#),
            "{text}"
        );
    }

    #[test]
    fn invalid_utf8_test_xml_still_counts() {
        let harness = Harness::new("test-junit-lossy");
        let xml = b"<testsuites><testsuite name=\"s\"><testcase name=\"raw\"><system-out>\xff</system-out></testcase></testsuite></testsuites>";
        let uri = write_bep_artifact(&harness, "test.xml", xml);
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            err.contains(
                "dx: warning: test.xml for //a:t (run 1, shard 1, attempt 1) has invalid UTF-8"
            ),
            "{err}"
        );
        let text = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
        assert!(text.contains(r#"<testcase name="raw""#), "{text}");
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
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//a:cached"));
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["cached"], serde_json::json!(true));
        assert_eq!(outcomes[0]["run"], serde_json::json!(2));
        assert_eq!(outcomes[0]["shard"], serde_json::json!(3));
        assert!(outcomes[0].get("configuration").is_none(), "{out}");
        assert!(outcomes[0].get("duration_millis").is_none(), "{out}");
        let finished_event = event(&events, "command_finished");
        assert_eq!(
            finished_event["tests"],
            serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        assert_eq!(finished_event["exit_code"], serde_json::json!(0));
        assert_eq!(finished_event["results_complete"], serde_json::json!(true));
    }

    #[test]
    fn the_passed_count_matches_the_results_report() {
        let harness = Harness::new("test-outcome-all-passed");
        let mut lines = Vec::new();
        for index in 0..3 {
            let uri = write_bep_artifact(
                &harness,
                &format!("cases-{index}.xml"),
                MINIMAL_TEST_XML.as_bytes(),
            );
            lines.push(test_result_line(
                &format!("//rust:compile_test{index}"),
                &[(String::from("test.xml"), uri)],
            ));
        }
        for index in 0..4 {
            let uri = write_bep_artifact(
                &harness,
                &format!("empty-{index}.xml"),
                EMPTY_TEST_XML.as_bytes(),
            );
            lines.push(test_result_line(
                &format!("//go:generation_test{index}"),
                &[(String::from("test.xml"), uri)],
            ));
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert!(outcomes.is_empty(), "{out}");
        let results: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let entries = results["results"].as_array().expect("results");
        assert_eq!(entries.len(), 7, "{results}");
        assert!(
            entries.iter().all(|entry| entry["status"] == "PASSED"),
            "{results}"
        );
        let no_cases = entries
            .iter()
            .filter(|entry| entry["outcome"] == "no_cases")
            .count();
        assert_eq!(no_cases, 4, "{results}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 7, "failed": 0, "cached": 0, "no_cases": 4, "skipped": 0})
        );
    }

    #[test]
    fn test_outcome_counts_cases_by_severity() {
        let harness = Harness::new("test-outcome-cases");
        let uri = write_bep_artifact(&harness, "mixed.xml", MIXED_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["cases_total"], serde_json::json!(4));
        assert_eq!(outcomes[0]["cases_failed"], serde_json::json!(1));
        assert_eq!(outcomes[0]["cases_error"], serde_json::json!(1));
        assert_eq!(outcomes[0]["cases_skipped"], serde_json::json!(1));
        assert!(outcomes[0].get("artifacts").is_none(), "{out}");
        assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
        let failing = outcomes[0]["failing_cases"].as_array().expect("failing");
        let names: Vec<_> = failing.iter().map(|case| case["name"].clone()).collect();
        assert_eq!(
            names,
            vec![serde_json::json!("fails"), serde_json::json!("errors")]
        );
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
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 2, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed"));
        assert_eq!(outcomes[0]["attempt"], serde_json::json!(1));
        assert_eq!(outcomes[0]["final"], serde_json::json!(false), "{out}");
        assert_eq!(
            outcomes[1]["outcome"],
            serde_json::json!("passed_after_retry")
        );
        assert_eq!(outcomes[1]["attempt"], serde_json::json!(2));
        assert!(outcomes[1].get("final").is_none(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        let read_results = || -> serde_json::Value {
            serde_json::from_slice(
                &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
            )
            .expect("results json")
        };
        let results = read_results();
        let entries = results["results"].as_array().expect("results");
        assert_eq!(entries.len(), 2, "{results}");
        assert_eq!(entries[0]["attempt"], serde_json::json!(1));
        assert_eq!(entries[0]["final"], serde_json::json!(false), "{results}");
        assert_eq!(entries[1]["attempt"], serde_json::json!(2));
        assert!(entries[1].get("final").is_none(), "{results}");
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=results=res.json"]);
        assert_eq!(code, 0, "{out}{err}");
        let results = read_results();
        let entries = results["results"].as_array().expect("results");
        assert_eq!(entries.len(), 2, "{results}");
        assert_eq!(entries[0]["attempt"], serde_json::json!(1));
        assert_eq!(entries[0]["final"], serde_json::json!(false), "{results}");
        assert!(entries[1].get("final").is_none(), "{results}");
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
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
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
    fn flaky_summary_without_result_prints_no_attempt() {
        let harness = Harness {
            raw_bep: Some(vec![test_flaky_summary_line("//a:t")]),
            ..Harness::new("test-flaky-summary-only")
        };
        let (_, _, err) = harness.run(&["test", "--output=text"]);
        assert!(
            err.contains("dx: flaky success (passed after retry): //a:t\n"),
            "{err}"
        );
        assert!(!err.contains("(attempt"), "{err}");
    }

    #[test]
    fn later_planned_report_event_is_incomplete_after_an_earlier_write_failure() {
        let harness = Harness::new("test-writefail-later");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
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
            "--report=results=missing-dir/r.json",
            "--report=junit=j.xml",
        ]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let reports = events_of_kind(&events, "report");
        assert_eq!(reports.len(), 1, "{out}");
        assert_eq!(reports[0]["format"], serde_json::json!("junit"), "{out}");
        assert_eq!(
            reports[0]["results_complete"],
            serde_json::json!(false),
            "{out}"
        );
    }

    fn evidence(problems: &[&str]) -> super::Evidence {
        let mut evidence = super::Evidence::default();
        for problem in problems {
            evidence.record((*problem).to_owned());
        }
        evidence
    }

    #[test]
    fn evidence_gap_names_what_made_results_incomplete() {
        assert_eq!(
            super::evidence_gap(true, &evidence(&[]), "none reported"),
            Some("none reported".to_owned())
        );
        assert_eq!(
            super::evidence_gap(true, &evidence(&["a", "b"]), "none reported"),
            Some("a".to_owned())
        );
        assert_eq!(
            super::evidence_gap(false, &evidence(&["a", "b"]), "none reported"),
            Some("2 missing or invalid results: a".to_owned())
        );
        assert_eq!(
            super::evidence_gap(false, &evidence(&[]), "none reported"),
            None
        );
    }

    #[test]
    fn parse_cases_stamps_run_and_configuration() {
        let result = dx_bep::TestResultRecord {
            label: "//a:t".to_owned(),
            configuration: Some("cfg".to_owned()),
            run: 2,
            shard: 3,
            attempt: 1,
            status: Some("PASSED".to_owned()),
            cached_locally: None,
            duration_millis: None,
            strategy: None,
            files: Vec::new(),
        };
        let cases = super::parse_cases(&result, MINIMAL_TEST_XML.as_bytes()).expect("cases");
        assert!(!cases.is_empty());
        for case in &cases {
            assert_eq!(case.run, 1);
            assert_eq!(case.configuration.as_deref(), Some("cfg"));
        }
        assert!(super::parse_cases(&result, b"no elements").is_err());
    }

    #[test]
    fn coverage_threshold_passes_without_minimum_and_fails_on_incomplete_evidence() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let sources = super::Sources::default();
        let text = dx_output::OutputMode::Text { quiet: false };
        assert_eq!(
            super::coverage_threshold(None, None, &sources, text, &mut out, &mut err),
            Ok(true)
        );
        assert_eq!(
            super::coverage_threshold(Some(80), None, &sources, text, &mut out, &mut err),
            Ok(false)
        );
        assert!(out.is_empty() && err.is_empty());
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
        assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
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
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
        let results: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let entry = &results["results"][0];
        assert_eq!(entry["outcome"], serde_json::json!("no_cases"), "{results}");
        assert_eq!(entry["cases_total"], serde_json::json!(0), "{results}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 1, "skipped": 0})
        );
    }

    #[test]
    fn test_cached_no_cases_result_counts_cached_and_no_cases() {
        let harness = Harness::new("test-outcome-cached-zero");
        let uri = write_bep_artifact(
            &harness,
            "empty.xml",
            "<testsuites></testsuites>".as_bytes(),
        );
        let harness = Harness {
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "PASSED",
                None,
                Some(true),
                None,
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 1, "no_cases": 1, "skipped": 0})
        );
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
        assert_eq!(outcomes[0]["artifacts"]["missing"], serde_json::json!(1));
        let finished = event(&events, "command_finished");
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    #[test]
    fn test_single_missing_xml_fails_when_bazel_passed() {
        let harness = Harness::new("test-partial-missing");
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
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 4, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
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
    fn test_missing_xml_reports_text_failure() {
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
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn test_complete_run_passes() {
        let harness = Harness::new("test-strict-complete");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
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
        assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        let failed: Vec<_> = events_of_kind(&events, "error")
            .into_iter()
            .filter(|error| error["code"] == serde_json::json!("bazel_failed"))
            .collect();
        assert_eq!(failed.len(), 1, "{out}");
        assert_eq!(failed[0]["cause"], serde_json::json!("bazel"), "{out}");
        assert_eq!(failed[0]["status"], serde_json::json!(4), "{out}");
        assert_eq!(failed[0]["phase"], serde_json::json!("execute"), "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(4));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    fn failing_run(name: &str, statuses: &[&str]) -> (i32, String, String) {
        let harness = Harness::new(name);
        let lines = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let uri = write_bep_artifact(
                    &harness,
                    &format!("{index}.xml"),
                    MINIMAL_TEST_XML.as_bytes(),
                );
                test_result_full_line(
                    &format!("//a:t{index}"),
                    1,
                    1,
                    1,
                    status,
                    None,
                    None,
                    None,
                    &[(String::from("test.xml"), uri)],
                )
            })
            .collect();
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(lines),
            ..harness
        };
        harness.run(&["test", "--output=json"])
    }

    fn errors_with_code<'a>(
        events: &'a [serde_json::Value],
        code: &str,
    ) -> Vec<&'a serde_json::Value> {
        events_of_kind(events, "error")
            .into_iter()
            .filter(|error| error["code"] == serde_json::json!(code))
            .collect()
    }

    #[test]
    fn failing_tests_emit_tests_failed_with_counts() {
        let (code, out, err) = failing_run("test-tests-failed", &["PASSED", "FAILED"]);
        assert_eq!(code, 3, "{out}{err}");
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["code"],
            serde_json::json!("tests_failed"),
            "{out}"
        );
        assert_eq!(errors[0]["cause"], serde_json::json!("test"), "{out}");
        assert_eq!(errors[0]["status"], serde_json::json!(3), "{out}");
        assert_eq!(errors[0]["phase"], serde_json::json!("execute"), "{out}");
        assert_eq!(
            errors[0]["message"],
            serde_json::json!("1 test failed; read the test_outcome events"),
            "{out}"
        );
        assert!(
            errors_with_code(&events, "bazel_failed").is_empty(),
            "{out}"
        );
        assert!(!out.contains("dx status"), "{out}");
        assert!(err.is_empty(), "{err}");
        let finished = event(&events, "command_finished");
        assert_eq!(
            finished["tests"],
            serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        assert_eq!(finished["exit_code"], serde_json::json!(3));
    }

    fn attempts_run(
        name: &str,
        attempts: &[(&str, u32, &str)],
        flaky: &[&str],
    ) -> (i32, String, String) {
        let harness = Harness::new(name);
        let mut lines: Vec<String> = attempts
            .iter()
            .enumerate()
            .map(|(index, (label, attempt, status))| {
                let uri = write_bep_artifact(
                    &harness,
                    &format!("{index}.xml"),
                    MINIMAL_TEST_XML.as_bytes(),
                );
                test_result_full_line(
                    label,
                    1,
                    1,
                    *attempt,
                    status,
                    None,
                    Some(false),
                    None,
                    &[(String::from("test.xml"), uri)],
                )
            })
            .collect();
        lines.extend(flaky.iter().map(|label| test_flaky_summary_line(label)));
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(lines),
            ..harness
        };
        harness.run(&["test", "--output=json"])
    }

    #[test]
    fn flaky_target_with_real_failure_counts_one_failure() {
        let (code, out, err) = attempts_run(
            "test-flaky-and-failed",
            &[
                ("//a:t", 1, "FAILED"),
                ("//a:t", 2, "PASSED"),
                ("//b:t", 1, "FAILED"),
            ],
            &["//a:t"],
        );
        assert_eq!(code, 3, "{out}{err}");
        let events = json_events(&out);
        let errors = errors_with_code(&events, "tests_failed");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["message"],
            serde_json::json!("1 test failed; read the test_outcome events"),
            "{out}"
        );
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 3, "{out}");
        let finals: Vec<(String, u64, Option<serde_json::Value>)> = outcomes
            .iter()
            .map(|outcome| {
                (
                    outcome["target"].as_str().expect("target").to_owned(),
                    outcome["attempt"].as_u64().expect("attempt"),
                    outcome.get("final").cloned(),
                )
            })
            .collect();
        assert!(
            finals.contains(&("//a:t".to_owned(), 1, Some(serde_json::json!(false)))),
            "{out}"
        );
        assert!(finals.contains(&("//a:t".to_owned(), 2, None)), "{out}");
        assert!(finals.contains(&("//b:t".to_owned(), 1, None)), "{out}");
        assert_eq!(final_failures(&outcomes), 1, "{out}");
        assert!(err.is_empty(), "{err}");
    }

    fn final_failures(outcomes: &[&serde_json::Value]) -> usize {
        outcomes
            .iter()
            .filter(|outcome| outcome["outcome"] == "failed" && outcome.get("final").is_none())
            .count()
    }

    #[test]
    fn retried_failure_counts_once() {
        let (code, out, err) = attempts_run(
            "test-retried-failure",
            &[
                ("//a:t", 1, "FAILED"),
                ("//a:t", 2, "FAILED"),
                ("//a:t", 3, "FAILED"),
            ],
            &[],
        );
        assert_eq!(code, 3, "{out}{err}");
        let events = json_events(&out);
        let errors = errors_with_code(&events, "tests_failed");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["message"],
            serde_json::json!("1 test failed; read the test_outcome events"),
            "{out}"
        );
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 0, "failed": 1, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 3, "{out}");
        for outcome in &outcomes {
            let expected = if outcome["attempt"] == 3 {
                None
            } else {
                Some(&serde_json::json!(false))
            };
            assert_eq!(outcome.get("final"), expected, "{out}");
        }
        assert_eq!(final_failures(&outcomes), 1, "{out}");
    }

    #[test]
    fn every_failed_test_still_reports_counts() {
        let (code, out, _) = failing_run("test-all-failed", &["FAILED", "TIMEOUT"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let failed = errors_with_code(&events, "tests_failed");
        assert_eq!(failed.len(), 1, "{out}");
        assert_eq!(
            failed[0]["message"],
            serde_json::json!("2 tests failed; read the test_outcome events"),
            "{out}"
        );
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 0, "failed": 2, "cached": 0, "no_cases": 0, "skipped": 0})
        );
    }

    #[test]
    fn exit_three_without_failed_results_keeps_bazel_failed() {
        let (code, out, _) = failing_run("test-exit-three-passed", &["PASSED"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        assert!(
            errors_with_code(&events, "tests_failed").is_empty(),
            "{out}"
        );
        let failed = errors_with_code(&events, "bazel_failed");
        assert_eq!(failed.len(), 1, "{out}");
        assert_eq!(failed[0]["cause"], serde_json::json!("bazel"), "{out}");
        assert_eq!(failed[0]["status"], serde_json::json!(3), "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
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
        let (code, out, _) =
            harness.run(&["coverage", "--output=json", "--report=results=res.json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert!(events_of_kind(&events, "test_outcome").is_empty(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        let results: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let entries = results["results"].as_array().expect("results");
        assert_eq!(entries.len(), 1, "{results}");
        assert_eq!(entries[0]["attempt"], serde_json::json!(2));
        assert!(entries[0].get("artifacts").is_none(), "{results}");
        assert!(entries[0].get("final").is_none(), "{results}");
    }

    const MIXED_CASES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="s"><testcase name="passes" classname="c"/><testcase name="fails" classname="c"><failure message="expected 1">trace</failure></testcase><testcase name="errs" classname="c"><error>panic text</error></testcase><testcase name="ignored" classname="c"><skipped message="not on linux"/></testcase></testsuite></testsuites>"#;

    fn completed_line(label: &str, children: &[serde_json::Value]) -> String {
        serde_json::json!({
            "id": {"targetCompleted": {"label": label}},
            "completed": {"success": true},
            "children": children,
        })
        .to_string()
    }

    fn summary_line(label: &str, status: &str) -> String {
        serde_json::json!({
            "id": {"testSummary": {"label": label}},
            "testSummary": {"overallStatus": status},
        })
        .to_string()
    }

    #[test]
    fn a_result_without_xml_beside_one_with_xml_is_incomplete() {
        let harness = Harness::new("test-mixed-xml");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                test_result_line("//a:ok", &[(String::from("test.xml"), uri)]),
                test_result_line("//b:bare", &[]),
            ]),
            ..harness
        };
        let (code, out, err) = harness.run(&["test", "--output=text", "--report=junit=out.xml"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(
                "dx: incomplete_results: test.xml for //b:bare (run 1, shard 1, attempt 1) was not reported"
            ),
            "{err}"
        );
        let document = std::fs::read_to_string(harness.workspace.join("out.xml")).expect("junit");
        assert!(
            document.contains("<testsuite name=\"//a:ok\""),
            "{document}"
        );
        assert!(
            document.contains("test.xml for //b:bare (run 1, shard 1, attempt 1) was not reported"),
            "{document}"
        );
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=junit=out.xml"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//b:bare"));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
        assert_eq!(outcomes[0]["artifacts"]["missing"], serde_json::json!(1));
        assert_eq!(
            event(&events, "report")["results_complete"],
            serde_json::json!(false)
        );
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
        let incomplete: Vec<_> = events_of_kind(&events, "error")
            .into_iter()
            .filter(|error| error["code"] == "incomplete_results")
            .collect();
        assert_eq!(incomplete.len(), 1, "{out}");
    }

    #[test]
    fn readable_reports_emit_each_case_with_its_identity() {
        let harness = Harness::new("test-cases");
        let uri = write_bep_artifact(&harness, "cases.xml", MIXED_CASES_XML.as_bytes());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                2,
                1,
                "FAILED",
                Some("cfg1"),
                Some(true),
                Some(40),
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert_eq!(outcome["target"], serde_json::json!("//a:t"));
        assert_eq!(outcome["configuration"], serde_json::json!("cfg1"));
        assert_eq!(outcome["shard"], serde_json::json!(2));
        assert_eq!(outcome["cached"], serde_json::json!(true));
        assert_eq!(outcome["cases_total"], serde_json::json!(4));
        assert_eq!(outcome["cases_failed"], serde_json::json!(1));
        assert_eq!(outcome["cases_error"], serde_json::json!(1));
        assert_eq!(outcome["cases_skipped"], serde_json::json!(1));
        assert_eq!(
            outcome["failing_cases"],
            serde_json::json!([
                {"classname": "c", "name": "fails", "outcome": "failed", "message": "expected 1"},
                {"classname": "c", "name": "errs", "outcome": "error", "message": "panic text"},
            ])
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let entries = document["results"].as_array().expect("results");
        assert_eq!(entries.len(), 1, "{document}");
        let named: Vec<(&str, &str)> = entries[0]["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .map(|case| {
                (
                    case["name"].as_str().expect("name"),
                    case["outcome"].as_str().expect("outcome"),
                )
            })
            .collect();
        assert_eq!(
            named,
            vec![
                ("passes", "passed"),
                ("fails", "failed"),
                ("errs", "error"),
                ("ignored", "skipped"),
            ]
        );
        let reports = events_of_kind(&events, "report");
        assert_eq!(reports[0]["format"], serde_json::json!("results"));
        assert_eq!(reports[0]["path"], serde_json::json!("res.json"));
    }

    fn failing_result_files(harness: &Harness, with_xml: bool) -> Vec<(String, String)> {
        let mut files = vec![(
            String::from("test.lcov"),
            write_bep_artifact(harness, "coverage.dat", MINIMAL_LCOV.as_bytes()),
        )];
        if with_xml {
            files.push((
                String::from("test.xml"),
                write_bep_artifact(harness, "cases.xml", MIXED_CASES_XML.as_bytes()),
            ));
        }
        files.push((
            String::from("test.log"),
            write_bep_artifact(harness, "test.log", b"first\nboom happened\n"),
        ));
        files
    }

    #[test]
    fn test_failing_outcome_carries_log_path_and_tail() {
        let harness = Harness::new("test-failing-log");
        let files = failing_result_files(&harness, true);
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
            )]),
            ..harness
        };
        let (_, out, _) = harness.run(&["test", "--output=json"]);
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert!(outcome["log_path"].as_str().is_some(), "{out}");
        assert!(
            outcome["log_tail"]
                .as_str()
                .expect("tail")
                .contains("boom happened"),
            "{out}"
        );
    }

    #[test]
    fn coverage_failing_outcome_carries_failing_cases_and_log() {
        let harness = Harness::new("cov-failing-cases");
        let files = failing_result_files(&harness, true);
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
            )]),
            ..harness
        };
        let (_, out, _) = harness.run(&["coverage", "--output=json"]);
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        let failing = outcome["failing_cases"].as_array().expect("failing");
        let names: Vec<&str> = failing
            .iter()
            .map(|case| case["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, vec!["fails", "errs"], "{out}");
        assert!(outcome["log_path"].as_str().is_some(), "{out}");
        assert!(
            outcome["log_tail"]
                .as_str()
                .expect("tail")
                .contains("boom happened"),
            "{out}"
        );
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(true),
            "{out}"
        );
    }

    #[test]
    fn coverage_failing_outcome_without_test_xml_stays_complete() {
        let harness = Harness::new("cov-failing-no-xml");
        let files = failing_result_files(&harness, false);
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t", 1, 1, 1, "FAILED", None, None, None, &files,
            )]),
            ..harness
        };
        let (_, out, _) = harness.run(&["coverage", "--output=json"]);
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert!(outcome.get("failing_cases").is_none(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(true),
            "{out}"
        );
    }

    #[test]
    fn passed_count_is_per_run_and_shard() {
        let harness = Harness::new("test-count-unit");
        let mut lines = Vec::new();
        let xml = vec![(
            String::from("test.xml"),
            write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes()),
        )];
        for run in 1..=2 {
            for shard in 1..=2 {
                let attempt_two = run == 1 && shard == 1;
                if attempt_two {
                    lines.push(test_result_full_line(
                        "//a:t", run, shard, 1, "FAILED", None, None, None, &xml,
                    ));
                    lines.push(test_result_full_line(
                        "//a:t", run, shard, 2, "PASSED", None, None, None, &xml,
                    ));
                } else {
                    lines.push(test_result_identity_line("//a:t", run, shard, 1, &xml));
                }
            }
        }
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (_, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        let events = json_events(&out);
        let tests = &event(&events, "command_finished")["tests"];
        assert_eq!(tests["passed"], serde_json::json!(4), "{out}");
        assert_eq!(tests["failed"], serde_json::json!(0), "{out}");
        let results: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let passed = results["results"]
            .as_array()
            .expect("results")
            .iter()
            .filter(|entry| entry["status"] == "PASSED" && entry["final"] != false)
            .count();
        assert_eq!(passed, 4, "{results}");
    }

    #[test]
    fn failing_case_past_the_case_limit_is_still_reported() {
        let harness = Harness::new("test-late-failure");
        let mut xml = String::from("<testsuites><testsuite name=\"s\">");
        for index in 0..1500 {
            xml.push_str(&format!("<testcase name=\"case{index}\"/>"));
        }
        xml.push_str("<testcase name=\"late\"><failure message=\"late boom\"/></testcase>");
        xml.push_str("</testsuite></testsuites>");
        let uri = write_bep_artifact(&harness, "late.xml", xml.as_bytes());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (_, out, _) = harness.run(&["test", "--output=json"]);
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        let failing = outcome["failing_cases"].as_array().expect("failing");
        assert_eq!(failing.len(), 1, "{out}");
        assert_eq!(failing[0]["name"], serde_json::json!("late"));
    }

    #[test]
    fn default_results_report_failure_fails_the_run() {
        let harness = Harness::new("test-results-default-blocked");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        std::fs::create_dir_all(harness.workspace.join(".dx/results.json")).expect("block");
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert!(
            events.iter().any(|event| event["code"] == "report_failed"),
            "{out}"
        );
        let finished = events.last().expect("command_finished");
        assert_eq!(finished["event"], "command_finished", "{out}");
        assert_eq!(finished["results_complete"], false, "{out}");
    }

    #[test]
    fn default_results_report_failure_keeps_the_bazel_code() {
        let harness = Harness::new("test-results-default-blocked-bazel");
        let uri = write_bep_artifact(&harness, "bad.xml", MIXED_TEST_XML.as_bytes());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        std::fs::create_dir_all(harness.workspace.join(".dx/results.json")).expect("block");
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let finished = events.last().expect("command_finished");
        assert_eq!(finished["results_complete"], false, "{out}");
    }

    #[test]
    fn results_document_run_id_matches_command_finished() {
        let harness = Harness::new("test-results-run-id");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
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
        let finished = events.last().expect("command_finished");
        let run_id = finished["run_id"].as_str().expect("run_id in event");
        let file = std::fs::read_to_string(harness.workspace.join(".dx/results.json"))
            .expect("results file");
        let document: serde_json::Value = serde_json::from_str(&file).expect("json");
        assert_eq!(document["run_id"], run_id);
        assert_eq!(document["record_version"], 1);
    }

    #[test]
    fn explicit_results_report_carries_the_run_id() {
        let harness = Harness::new("test-results-run-id-explicit");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
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
            "--report",
            "results=elsewhere.json",
        ]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let finished = events.last().expect("command_finished");
        let run_id = finished["run_id"].as_str().expect("run_id in event");
        let file = std::fs::read_to_string(harness.workspace.join("elsewhere.json"))
            .expect("results file");
        let document: serde_json::Value = serde_json::from_str(&file).expect("json");
        assert_eq!(document["run_id"], run_id);
        assert!(!harness.workspace.join(".dx/results.json").exists());
    }

    #[test]
    fn case_detail_is_bounded_and_says_so() {
        let harness = Harness::new("test-case-bounds");
        let long = "é".repeat(3000);
        let mut xml = String::from("<testsuites><testsuite name=\"s\">");
        xml.push_str(&format!(
            "<testcase name=\"long\"><failure message=\"{long}\"/></testcase>"
        ));
        for index in 0..1000 {
            xml.push_str(&format!("<testcase name=\"case{index}\"/>"));
        }
        xml.push_str("</testsuite></testsuites>");
        let uri = write_bep_artifact(&harness, "many.xml", xml.as_bytes());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (_, out, _) = harness.run(&["test", "--output=json", "--report=results=res.json"]);
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert_eq!(outcome["cases_total"], serde_json::json!(1001));
        let failing = outcome["failing_cases"].as_array().expect("failing");
        assert_eq!(failing.len(), 1);
        let message = failing[0]["message"].as_str().expect("message");
        assert_eq!(message.len(), dx_output::EVENT_MESSAGE_LIMIT);
        assert!(message.chars().all(|c| c == 'é'));
        assert!(outcome.get("failing_cases_omitted").is_none(), "{out}");
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("res.json")).expect("results report"),
        )
        .expect("results json");
        let cases = document["results"][0]["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), 1000);
        assert_eq!(
            document["results"][0]["cases_omitted"],
            serde_json::json!(1)
        );
        assert_eq!(cases[0]["message"].as_str().expect("message").len(), 4096);
    }

    #[test]
    fn an_announced_result_that_never_arrives_is_incomplete() {
        let harness = Harness::new("test-gap");
        let uri = write_bep_artifact(&harness, "shard1.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                completed_line(
                    "//a:t",
                    &[
                        serde_json::json!({"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}}),
                        serde_json::json!({"testResult": {"label": "//a:t", "run": 1, "shard": 2, "attempt": 1}}),
                    ],
                ),
                test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("not_reported"));
        assert_eq!(outcomes[0]["shard"], serde_json::json!(2));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
        assert!(outcomes[0].get("status").is_none(), "{out}");
        assert!(outcomes[0].get("cases_total").is_none(), "{out}");
        let error = events_of_kind(&events, "error")
            .into_iter()
            .find(|error| error["code"] == "incomplete_results")
            .expect("incomplete");
        assert_eq!(
            error["message"],
            serde_json::json!(
                "testResult for //a:t (run 1, shard 2, attempt 1) was announced but never reported"
            )
        );
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn a_cancelled_result_keeps_the_bazel_code_and_its_cause() {
        let harness = Harness {
            bazel_code: 8,
            raw_bep: Some(vec![serde_json::json!({
                "id": {"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}},
                "aborted": {"reason": "USER_INTERRUPTED"},
            })
            .to_string()]),
            ..Harness::new("test-cancelled")
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 8, "{out}");
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert_eq!(outcome["outcome"], serde_json::json!("cancelled"));
        assert_eq!(outcome["evidence_complete"], serde_json::json!(false));
        assert_eq!(
            event(&events, "command_finished")["exit_code"],
            serde_json::json!(8)
        );
    }

    #[test]
    fn a_stream_without_bazels_last_event_is_incomplete() {
        let harness = Harness::new("test-unterminated");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            unterminated_bep: true,
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("dx: incomplete_results: build events ended before Bazel's last event"),
            "{err}"
        );
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert!(
            events_of_kind(&events, "test_outcome").is_empty(),
            "the collected result itself stays complete and is counted: {out}"
        );
        assert_eq!(
            event(&events, "command_finished")["tests"],
            serde_json::json!({"passed": 1, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn a_summary_without_results_is_its_own_outcome_without_cases() {
        let harness = Harness {
            bazel_code: 1,
            raw_bep: Some(vec![summary_line("//a:broken", "FAILED_TO_BUILD")]),
            ..Harness::new("test-summary-only")
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcome = event(&events, "test_outcome");
        assert_eq!(outcome["target"], serde_json::json!("//a:broken"));
        assert_eq!(outcome["outcome"], serde_json::json!("failed_to_build"));
        assert_eq!(outcome["status"], serde_json::json!("FAILED_TO_BUILD"));
        assert_eq!(outcome["evidence_complete"], serde_json::json!(false));
        for field in ["cases_total", "run", "shard", "attempt", "cached"] {
            assert!(outcome.get(field).is_none(), "{field}: {out}");
        }

        let harness = Harness {
            raw_bep: Some(vec![summary_line("//a:t", "PASSED")]),
            ..Harness::new("test-summary-only-pass")
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 1, "a summary is not a collected result: {err}");
        assert!(
            err.contains("no test result was reported for //a:t (summary PASSED)"),
            "{err}"
        );
    }

    #[test]
    fn a_test_that_fails_to_build_is_its_own_outcome() {
        let configured = |label: &str| {
            serde_json::json!({
                "id": {"targetConfigured": {"label": label}},
                "configured": {"targetKind": "sh_test rule", "testSize": "MEDIUM"},
            })
            .to_string()
        };
        let completed = |label: &str, success: bool| {
            serde_json::json!({
                "id": {"targetCompleted": {"label": label, "configuration": {"id": "cfg1"}}},
                "completed": {"success": success},
            })
            .to_string()
        };
        let harness = Harness::new("test-unbuilt");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            bazel_code: 1,
            raw_bep: Some(vec![
                configured("//a:broken"),
                configured("//a:t"),
                serde_json::json!({
                    "id": {"targetCompleted": {"label": "//a:broken", "configuration": {"id": "cfg1"}}},
                    "completed": {"outputGroup": [{"name": "default", "fileSets": []}]},
                })
                .to_string(),
                completed("//a:t", true),
                test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//a:broken"));
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("failed_to_build"));
        assert_eq!(outcomes[0]["configuration"], serde_json::json!("cfg1"));
        assert_eq!(outcomes[0]["evidence_complete"], serde_json::json!(false));
        for field in ["status", "cases_total", "run", "shard", "attempt", "cached"] {
            assert!(outcomes[0].get(field).is_none(), "{field}: {out}");
        }
        let error = events_of_kind(&events, "error")
            .into_iter()
            .find(|error| error["code"] == "incomplete_results")
            .expect("incomplete");
        assert_eq!(error["code"], serde_json::json!("incomplete_results"));
        assert_eq!(
            error["message"],
            serde_json::json!("//a:broken failed to build")
        );
        let finished = event(&events, "command_finished");
        assert_eq!(finished["results_complete"], serde_json::json!(false));

        let harness = Harness {
            bazel_code: 1,
            raw_bep: Some(vec![
                configured("//a:broken"),
                completed("//a:broken", false),
                summary_line("//a:broken", "FAILED_TO_BUILD"),
            ]),
            ..Harness::new("test-unbuilt-summarized")
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["status"], serde_json::json!("FAILED_TO_BUILD"));
    }

    #[test]
    fn skipped_targets_are_reported_without_blocking_collected_results() {
        let harness = Harness::new("test-skipped");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let configured = serde_json::json!({
            "id": {"targetConfigured": {"label": "//a:windows_only"}},
            "configured": {"targetKind": "sh_test rule", "testSize": "SMALL"},
        })
        .to_string();
        let skipped = serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:windows_only", "configuration": {"id": "cfg1"}}},
            "aborted": {"reason": "SKIPPED"},
        })
        .to_string();
        let harness = Harness {
            raw_bep: Some(vec![
                configured.clone(),
                skipped.clone(),
                test_result_line("//a:t", &[(String::from("test.xml"), uri)]),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//a:windows_only"));
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("skipped"));
        assert_eq!(outcomes[0]["configuration"], serde_json::json!("cfg1"));
        assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(true)
        );

        let only_skipped = Harness {
            raw_bep: Some(vec![configured, skipped]),
            ..Harness::new("test-only-skipped")
        };
        let (code, out, _) = only_skipped.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "nothing was selected to run: {out}");
        let events = json_events(&out);
        assert_eq!(
            event(&events, "test_outcome")["outcome"],
            serde_json::json!("skipped")
        );
    }

    fn filtered_target_lines() -> Vec<String> {
        vec![
            serde_json::json!({
                "id": {"targetConfigured": {"label": "//a:slow"}},
                "configured": {"targetKind": "sh_test rule", "testSize": "LARGE"},
            })
            .to_string(),
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//a:slow", "configuration": {"id": "cfg1"}}},
                "completed": {"success": true},
            })
            .to_string(),
        ]
    }

    #[test]
    fn filtered_targets_are_reported_and_counted() {
        let harness = Harness::new("test-filtered");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let mut lines = filtered_target_lines();
        lines.push(test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert_eq!(outcomes.len(), 1, "{out}");
        assert_eq!(outcomes[0]["target"], serde_json::json!("//a:slow"));
        assert_eq!(outcomes[0]["outcome"], serde_json::json!("filtered"));
        assert!(outcomes[0].get("evidence_complete").is_none(), "{out}");
        let notices: Vec<_> = events_of_kind(&events, "notice")
            .into_iter()
            .filter(|notice| notice["code"] == serde_json::json!("tests_filtered"))
            .collect();
        assert_eq!(notices.len(), 1, "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["tests"]["skipped"], serde_json::json!(1), "{out}");
        assert_eq!(finished["tests"]["failed"], serde_json::json!(0), "{out}");
    }

    #[test]
    fn filtered_targets_print_a_stderr_line_in_text_mode() {
        let harness = Harness::new("test-filtered-text");
        let uri = write_bep_artifact(&harness, "ok.xml", MINIMAL_TEST_XML.as_bytes());
        let mut lines = filtered_target_lines();
        lines.push(test_result_line(
            "//a:t",
            &[(String::from("test.xml"), uri)],
        ));
        let harness = Harness {
            raw_bep: Some(lines),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert_eq!(err.matches("dx: tests_filtered: ").count(), 1, "{err}");
        assert!(err.contains("//a:slow"), "{err}");
    }

    #[test]
    fn coverage_reports_filtered_targets() {
        let mut harness = coverage_harness("cov-filtered", MINIMAL_LCOV.as_bytes());
        let mut lines = filtered_target_lines();
        lines.extend(harness.raw_bep.take().expect("bep"));
        harness.raw_bep = Some(lines);
        let (_, out, _) = harness.run(&["coverage", "--output=json"]);
        let events = json_events(&out);
        let outcomes = events_of_kind(&events, "test_outcome");
        assert!(
            outcomes
                .iter()
                .any(|outcome| outcome["outcome"] == serde_json::json!("filtered")),
            "{out}"
        );
    }

    #[test]
    fn coverage_without_bazels_last_event_is_incomplete() {
        let mut harness = coverage_harness("cov-unterminated", MINIMAL_LCOV.as_bytes());
        harness.unterminated_bep = true;
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("dx: incomplete_results: build events ended before Bazel's last event"),
            "{err}"
        );
    }

    fn no_collector_configured(label: &str) -> String {
        serde_json::json!({
            "id": {"targetConfigured": {"label": label}},
            "configured": {
                "targetKind": "_zig_forward_test rule",
                "testSize": "SMALL",
                "tag": ["manual", "dx-no-coverage-collector"],
            },
        })
        .to_string()
    }

    #[test]
    fn coverage_fails_when_only_collectorless_tests_ran() {
        let harness = Harness::new("cov-no-collector-only");
        let empty = write_bep_artifact(&harness, "zig.dat", b"");
        let harness = Harness {
            raw_bep: Some(vec![
                no_collector_configured("//z:t"),
                test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("dx: no_coverage_collector: no coverage collector for //z:t"),
            "{err}"
        );
        assert!(!err.contains("incomplete_results"), "{err}");
    }

    #[test]
    fn coverage_fails_when_collectorless_tests_mix_with_lcov() {
        let harness = Harness::new("cov-no-collector-mixed");
        let empty = write_bep_artifact(&harness, "zig2.dat", b"");
        let ok = write_bep_artifact(&harness, "rust.dat", MINIMAL_LCOV.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                no_collector_configured("//z:t"),
                test_result_line("//z:t", &[(String::from("coverage.dat"), empty.clone())]),
                test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
                test_result_line("//r:t", &[(String::from("coverage.dat"), ok)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["coverage", "--output=text", "--report=lcov=out.lcov"]);
        assert_eq!(code, 1, "{err}");
        assert_eq!(err.matches("no_coverage_collector").count(), 1, "{err}");
        let document = std::fs::read(harness.workspace.join("out.lcov")).expect("lcov");
        assert_eq!(document, MINIMAL_LCOV.as_bytes());
    }

    #[test]
    fn coverage_ignores_collectorless_tests_without_results() {
        let mut harness = coverage_harness("cov-no-collector-filtered", MINIMAL_LCOV.as_bytes());
        let mut lines = vec![no_collector_configured("//z:skipped")];
        lines.extend(harness.raw_bep.take().expect("bep"));
        harness.raw_bep = Some(lines);
        let (code, _, err) = harness.run(&["coverage", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("no_coverage_collector"), "{err}");
    }

    #[test]
    fn coverage_no_collector_is_a_json_error_event() {
        let harness = Harness::new("cov-no-collector-json");
        let empty = write_bep_artifact(&harness, "zig3.dat", b"");
        let harness = Harness {
            raw_bep: Some(vec![
                no_collector_configured("//z:t"),
                test_result_line("//z:t", &[(String::from("coverage.dat"), empty)]),
            ]),
            ..harness
        };
        let (code, out, _) = harness.run(&["coverage", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert!(
            events_of_kind(&events, "error")
                .iter()
                .any(|event| event["code"] == serde_json::json!("no_coverage_collector")),
            "{out}"
        );
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(true),
            "{out}"
        );
    }

    #[test]
    fn test_ignores_the_no_collector_tag() {
        let harness = Harness::new("test-no-collector");
        let uri = write_bep_artifact(&harness, "zig.xml", MINIMAL_TEST_XML.as_bytes());
        let harness = Harness {
            raw_bep: Some(vec![
                no_collector_configured("//z:t"),
                test_result_line("//z:t", &[(String::from("test.xml"), uri)]),
            ]),
            ..harness
        };
        let (code, _, err) = harness.run(&["test", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("no_coverage_collector"), "{err}");
    }

    struct BrokenPipe;

    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken pipe",
            ))
        }
    }

    #[test]
    fn announce_incomplete_returns_the_stdout_code_when_stdout_breaks() {
        let mut err = Vec::new();
        assert_eq!(
            super::announce_incomplete(
                dx_output::OutputMode::Json,
                &mut BrokenPipe,
                &mut err,
                "no test.xml"
            ),
            Err(dx_process::broken_pipe_code())
        );
    }

    #[test]
    fn coverage_threshold_returns_the_stdout_code_when_stdout_breaks() {
        let mut err = Vec::new();
        let account = dx_lcov::CellAccount::default();
        assert_eq!(
            super::coverage_threshold(
                Some(80),
                Some(&account),
                &super::Sources::default(),
                dx_output::OutputMode::Json,
                &mut BrokenPipe,
                &mut err,
            ),
            Err(dx_process::broken_pipe_code())
        );
    }

    #[test]
    fn an_outcome_that_fails_to_build_ends_the_stream_once() {
        let outcome = dx_output::TestOutcome {
            outcome: "failed".to_owned(),
            ..dx_output::TestOutcome::default()
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let ended = super::emit_outcomes(
            dx_output::OutputMode::Json,
            &mut out,
            &mut err,
            &[outcome],
            &[],
        );
        assert_eq!(ended.err(), Some(dx_process::operational_code()));
        let events: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["error", "command_finished"], "{events:?}");
        assert_eq!(events[0]["code"], "invalid_result");
    }
}
