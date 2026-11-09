use super::*;

/// Returns one exec path spelled without the build directory, so a report reads the same on every host.
pub(super) fn stable_exec_path(exec_path: &Path) -> String {
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
pub(super) const BAZEL_TESTS_FAILED: i32 = 3;

/// Whether a test outcome counts as a failed test.
pub(super) fn is_failed(outcome: &str) -> bool {
    matches!(outcome, "failed" | "timeout")
}

pub(super) fn unusable_detail(count: usize, first: &str) -> String {
    if count > 1 {
        format!("{count} missing or invalid results: {first}")
    } else {
        first.to_owned()
    }
}

/// True for the names Bazel reports a test's coverage result under.
pub(super) fn is_coverage_output(name: &str) -> bool {
    name == "coverage.dat" || name == "test.lcov"
}

/// True for result files that carry coverage evidence.
pub(super) fn is_coverage_file(file: &TestResultFile) -> bool {
    is_coverage_output(&file.name)
}

/// The latest attempt Bazel reported for each result's named coverage output.
pub(super) fn final_coverage_attempts(
    results: &[TestResultRecord],
) -> BTreeMap<(&str, u32, u32, &str), u32> {
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
pub(super) fn mark_superseded_attempts(reported: &mut [TestOutcome]) {
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
pub(crate) fn output_locations(
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
pub(super) fn status_outcome(status: Option<&str>) -> String {
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
pub(super) fn result_outcome(status: Option<&str>, summary: Option<&str>) -> String {
    if status == Some("PASSED") && summary == Some("FLAKY") {
        return "passed_after_retry".to_owned();
    }
    status_outcome(status)
}

/// Indexes `testSummary` overall statuses by target and configuration.
pub(super) fn summary_statuses(
    events: &dx_bep::TestEvents,
) -> BTreeMap<(&str, Option<&str>), &str> {
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
pub(super) fn result_where(label: &str, name: &str, run: u32, shard: u32, attempt: u32) -> String {
    format!("{name} for {label} (run {run}, shard {shard}, attempt {attempt})")
}

pub(super) const CASE_EVENT_LIMIT: usize = 1000;

pub(super) const CASE_MESSAGE_LIMIT: usize = 4096;

pub(super) const LOG_TAIL_LINES: usize = 40;

pub(super) const LOG_TAIL_BYTES: usize = 8192;

/// The last `LOG_TAIL_LINES` lines of a test log. When those exceed `LOG_TAIL_BYTES`, the oldest
/// whole lines are dropped; a single oversize line is cut at a char boundary instead.
pub(super) fn log_tail(bytes: &[u8]) -> Option<String> {
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
pub(super) fn tally_cases(
    outcome: &mut TestOutcome,
    cases: &[JunitCase],
    cases_seen: &mut Vec<TestCase>,
) {
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
pub(super) fn attach_log(
    outcome: &mut TestOutcome,
    result: &TestResultRecord,
    reader: &dyn ArtifactReader,
) {
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
pub(super) struct Evidence {
    pub(super) count: usize,
    pub(super) first: String,
}

impl Evidence {
    pub(super) fn record(&mut self, problem: String) {
        self.count += 1;
        if self.first.is_empty() {
            self.first = problem;
        }
    }
}

/// The machine outcome of one native test case.
pub(super) fn case_outcome(case: &JunitCase) -> &'static str {
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
pub(super) fn bounded(text: &str, limit: usize) -> (String, bool) {
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
pub(super) fn case_message(case: &JunitCase) -> Option<&str> {
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
pub(super) fn text_location(text: &str) -> Option<(String, u64)> {
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
pub(super) fn case_location(case: &JunitCase) -> (Option<String>, Option<u64>) {
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
pub(super) fn test_case(case: &JunitCase) -> TestCase {
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
pub(super) fn summary_outcome(status: Option<&str>) -> String {
    match status {
        Some("FAILED_TO_BUILD") => "failed_to_build".to_owned(),
        other => status_outcome(other),
    }
}

/// Accounts for undelivered events, summaries without results, unbuilt tests and skipped targets.
pub(super) fn account_stream(
    events: &TestEvents,
    evidence: &mut Evidence,
    reported: &mut Vec<TestOutcome>,
) {
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

pub(super) const FILTERED_LABELS_SHOWN: usize = 3;

pub(super) fn filtered_notice(
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
pub(super) fn results_document(
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
pub(super) type Summaries<'a> = BTreeMap<(&'a str, Option<&'a str>), &'a str>;

/// The outcome of one test result before any of its artifacts are read.
pub(super) fn outcome_for(result: &TestResultRecord, summaries: &Summaries) -> TestOutcome {
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
pub(super) fn read_or_record(
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
pub(super) fn parse_cases(
    result: &TestResultRecord,
    bytes: &[u8],
) -> Result<Vec<JunitCase>, ReportError> {
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
pub(super) fn evidence_gap(empty: bool, evidence: &Evidence, missing_text: &str) -> Option<String> {
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
