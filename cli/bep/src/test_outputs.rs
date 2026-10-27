use std::cell::LazyCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;

use serde_json::Value;

use super::{
    file_uri_to_path, is_bytestream_uri, malformed, missing_test_output, testlog_path,
    testlog_path_under, BepError, EventGap, EventLedger, EventLines, OutputLocations,
    TestOutputIdentity,
};

/// One named artifact of one test result, resolved to its local path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResultFile {
    pub name: String,
    pub exec_path: PathBuf,
}

/// One `testResult` event: Bazel's authoritative status for one run, shard, and attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResultRecord {
    pub label: String,
    pub configuration: Option<String>,
    pub run: u32,
    pub shard: u32,
    pub attempt: u32,
    pub status: Option<String>,
    pub cached_locally: Option<bool>,
    pub duration_millis: Option<u64>,
    pub strategy: Option<String>,
    pub files: Vec<TestResultFile>,
}

/// One `testSummary` event: Bazel's rolled-up status for one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSummaryRecord {
    pub label: String,
    pub configuration: Option<String>,
    pub overall_status: Option<String>,
    pub total_num_cached: Option<u64>,
    pub run_count: Option<u64>,
    pub attempt_count: Option<u64>,
}

/// One test target whose build Bazel reported as failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnbuiltTest {
    pub label: String,
    pub configuration: Option<String>,
}

/// The test result and summary events of one build event stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestEvents {
    pub results: Vec<TestResultRecord>,
    pub summaries: Vec<TestSummaryRecord>,
    pub ended: bool,
    pub gaps: Vec<EventGap>,
    pub skipped: Vec<EventGap>,
    pub unbuilt: Vec<UnbuiltTest>,
    /// Tests that built and completed but have no result or summary.
    pub filtered: Vec<UnbuiltTest>,
    /// The cut-off last line that ended the stream early.
    pub problem: Option<String>,
    /// Test labels whose configured target carries
    /// [`NO_COVERAGE_COLLECTOR_TAG`]: their language writes empty LCOV.
    pub no_coverage_collector: BTreeSet<String>,
}

/// The tag dx test macros put on a test whose language has no coverage
/// collector; it mirrors `DX_NO_COVERAGE_COLLECTOR_TAG` in
/// `libs/starlark/wrapper.bzl`.
pub const NO_COVERAGE_COLLECTOR_TAG: &str = "dx-no-coverage-collector";

struct TestResultId<'a> {
    label: &'a str,
    run: u32,
    shard: u32,
    attempt: u32,
}

impl<'a> TestResultId<'a> {
    fn parse(id: &'a serde_json::Map<String, Value>, line: u64) -> Result<Self, BepError> {
        let label = id
            .get("label")
            .and_then(Value::as_str)
            .ok_or_else(|| malformed(line, "id.testResult.label", "test result without label"))?;
        Ok(TestResultId {
            label,
            run: test_index(id, "run", "id.testResult.run", line)?,
            shard: test_index(id, "shard", "id.testResult.shard", line)?,
            attempt: test_index(id, "attempt", "id.testResult.attempt", line)?,
        })
    }
}

struct TestActionOutput<'a> {
    name: &'a str,
    uri: &'a str,
    path_prefix: Vec<String>,
}

impl<'a> TestActionOutput<'a> {
    fn parse(file: &'a Value, index: usize, line: u64) -> Result<Self, BepError> {
        let base = format!("testResult.testActionOutput[{index}]");
        let entry = file
            .as_object()
            .ok_or_else(|| malformed(line, &base, "test action output must be an object"))?;
        let name = entry.get("name").and_then(Value::as_str).ok_or_else(|| {
            malformed(
                line,
                &format!("{base}.name"),
                "test action output without name",
            )
        })?;
        let uri = entry.get("uri").and_then(Value::as_str).ok_or_else(|| {
            malformed(
                line,
                &format!("{base}.uri"),
                "test action output without uri",
            )
        })?;
        Ok(TestActionOutput {
            name,
            uri,
            path_prefix: path_prefix(entry, line, &base)?,
        })
    }
}

fn path_prefix(
    file: &serde_json::Map<String, Value>,
    line: u64,
    base: &str,
) -> Result<Vec<String>, BepError> {
    let Some(value) = file.get("pathPrefix") else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| malformed(line, &format!("{base}.pathPrefix"), "must be an array"))?;
    values
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                malformed(
                    line,
                    &format!("{base}.pathPrefix"),
                    "entries must be strings",
                )
            })
        })
        .collect()
}

struct PendingFile {
    name: String,
    uri: String,
    path_prefix: Vec<String>,
}

struct PendingResult {
    label: String,
    configuration: Option<String>,
    run: u32,
    shard: u32,
    attempt: u32,
    status: Option<String>,
    cached_locally: Option<bool>,
    duration_millis: Option<u64>,
    strategy: Option<String>,
    files: Vec<PendingFile>,
}

struct PendingSummary {
    label: String,
    configuration: Option<String>,
    overall_status: Option<String>,
    total_num_cached: Option<u64>,
    run_count: Option<u64>,
    attempt_count: Option<u64>,
}

/// One string field of a JSON object, when it is present and a string.
fn opt_string(object: &serde_json::Map<String, Value>, field: &str) -> Option<String> {
    object.get(field).and_then(Value::as_str).map(str::to_owned)
}

/// One boolean field of a JSON object, when it is present and a boolean.
fn opt_bool(object: &serde_json::Map<String, Value>, field: &str) -> Option<bool> {
    object.get(field).and_then(Value::as_bool)
}

/// One count field of a JSON object, spelled as a number or a digits string.
///
/// Bazel renders 64-bit integers as JSON strings, so both spellings count.
/// Anything else is unknown, not zero.
fn opt_count(object: &serde_json::Map<String, Value>, field: &str) -> Option<u64> {
    match object.get(field) {
        Some(Value::Number(raw)) => raw.as_u64(),
        Some(Value::String(raw)) => raw.parse::<u64>().ok(),
        _ => None,
    }
}

/// The configuration id Bazel ran one result or summary under, when reported.
fn opt_configuration(id: &serde_json::Map<String, Value>) -> Option<String> {
    id.get("configuration")
        .and_then(Value::as_object)
        .and_then(|configuration| configuration.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Reads the test events of a stream, keeping what came before a cut-off last line.
pub fn collect_test_events(
    reader: impl BufRead,
    locations: impl FnOnce() -> Option<OutputLocations>,
) -> Result<TestEvents, BepError> {
    let (mut events, tail) = read_test_events(reader, locations)?;
    events.problem = tail.map(|(line, reason)| format!("line {line}: {reason}"));
    Ok(events)
}

/// The test events of a stream and the line and reason of a cut-off last line.
fn read_test_events(
    reader: impl BufRead,
    locations: impl FnOnce() -> Option<OutputLocations>,
) -> Result<(TestEvents, Option<(u64, String)>), BepError> {
    let locations = LazyCell::new(locations);
    let mut pending: Vec<PendingResult> = Vec::new();
    let mut summaries: Vec<PendingSummary> = Vec::new();
    let mut shard_counts: HashMap<String, u32> = HashMap::new();
    let mut ledger = EventLedger::default();
    let mut test_labels: HashSet<String> = HashSet::new();
    let mut no_coverage_collector: BTreeSet<String> = BTreeSet::new();
    let mut failed_completions: Vec<UnbuiltTest> = Vec::new();
    let mut passed_completions: Vec<UnbuiltTest> = Vec::new();
    let mut mnemonics: HashMap<String, String> = HashMap::new();
    let mut lines = EventLines::new(reader);
    for next in lines.by_ref() {
        let (line_no, event) = next?;
        let object = event.as_object().ok_or_else(|| BepError::MalformedEvent {
            line: line_no,
            reason: "event must be a JSON object".to_owned(),
        })?;
        let id = object.get("id").and_then(Value::as_object).ok_or_else(|| {
            BepError::MalformedEvent {
                line: line_no,
                reason: "event without id".to_owned(),
            }
        })?;
        ledger.observe(object);
        if let Some((config, mnemonic)) = configuration_mnemonic(id, object) {
            mnemonics.insert(config.to_owned(), mnemonic.to_owned());
        }
        if object.contains_key("aborted") {
            continue;
        }
        if let Some(label) = test_label(id, object) {
            test_labels.insert(label.to_owned());
            if has_tag(object, NO_COVERAGE_COLLECTOR_TAG) {
                no_coverage_collector.insert(label.to_owned());
            }
        }
        if let Some((target, success)) = target_completion(id, object) {
            if success {
                passed_completions.push(target);
            } else {
                failed_completions.push(target);
            }
        }
        if let Some(summary_id) = id.get("testSummary").and_then(Value::as_object) {
            record_shard_count(summary_id, object, line_no, &mut shard_counts)?;
            summaries.push(parse_summary(summary_id, object));
            continue;
        }
        let result_id = id.get("testResult").and_then(Value::as_object);
        let Some(result_id) = result_id else {
            continue;
        };
        let label = TestResultId::parse(result_id, line_no)?;
        let result = object.get("testResult").and_then(Value::as_object);
        let reported = result.and_then(|result| result.get("testActionOutput"));
        let mut pending_files = Vec::new();
        if let Some(reported) = reported {
            let files = reported.as_array().ok_or_else(|| {
                malformed(
                    line_no,
                    "testResult.testActionOutput",
                    "testActionOutput must be an array",
                )
            })?;
            pending_files.reserve(files.len());
            for (index, file) in files.iter().enumerate() {
                let entry = TestActionOutput::parse(file, index, line_no)?;
                pending_files.push(PendingFile {
                    name: entry.name.to_owned(),
                    uri: entry.uri.to_owned(),
                    path_prefix: entry.path_prefix,
                });
            }
        }
        let status = result.and_then(|body| opt_string(body, "status"));
        let cached_locally = result.and_then(|body| opt_bool(body, "cachedLocally"));
        let duration_millis = result.and_then(|body| opt_count(body, "testAttemptDurationMillis"));
        let strategy = result
            .and_then(|body| body.get("executionInfo"))
            .and_then(Value::as_object)
            .and_then(|info| opt_string(info, "strategy"));
        pending.push(PendingResult {
            label: label.label.to_owned(),
            configuration: opt_configuration(result_id),
            run: label.run,
            shard: label.shard,
            attempt: label.attempt,
            status,
            cached_locally,
            duration_millis,
            strategy,
            files: pending_files,
        });
    }
    let run_totals = run_totals(&pending);
    let attempt_totals = attempt_totals(&pending);
    let mut results = Vec::new();
    for result in &pending {
        let run_total = run_totals.get(result.label.as_str()).copied().unwrap_or(1);
        let shard_total = shard_counts
            .get(&result.label)
            .copied()
            .unwrap_or(0)
            .max(shard_max(&pending, &result.label))
            .max(1);
        let attempt_total = attempt_totals
            .get(&(result.label.as_str(), result.run, result.shard))
            .copied()
            .unwrap_or(1);
        let identity = TestOutputIdentity {
            run: result.run,
            shard: result.shard,
            attempt: result.attempt,
            run_total,
            shard_total,
            attempt_total,
        };
        let mut files = Vec::with_capacity(result.files.len());
        for file in &result.files {
            let exec_path = if is_bytestream_uri(&file.uri) {
                let mnemonic = result
                    .configuration
                    .as_deref()
                    .and_then(|id| mnemonics.get(id))
                    .map(String::as_str);
                resolve_bytestream(
                    LazyCell::force(&locations).as_ref(),
                    mnemonic,
                    &result.label,
                    file,
                    identity,
                )?
            } else {
                file_uri_to_path(&file.uri)?
            };
            files.push(TestResultFile {
                name: file.name.clone(),
                exec_path,
            });
        }
        results.push(TestResultRecord {
            label: result.label.clone(),
            configuration: result.configuration.clone(),
            run: result.run,
            shard: result.shard,
            attempt: result.attempt,
            status: result.status.clone(),
            cached_locally: result.cached_locally,
            duration_millis: result.duration_millis,
            strategy: result.strategy.clone(),
            files,
        });
    }
    results.sort_by(|a, b| {
        a.label
            .as_bytes()
            .cmp(b.label.as_bytes())
            .then(a.run.cmp(&b.run))
            .then(a.shard.cmp(&b.shard))
            .then(a.attempt.cmp(&b.attempt))
    });
    let mut summary_records = Vec::with_capacity(summaries.len());
    for summary in &summaries {
        summary_records.push(TestSummaryRecord {
            label: summary.label.clone(),
            configuration: summary.configuration.clone(),
            overall_status: summary.overall_status.clone(),
            total_num_cached: summary.total_num_cached,
            run_count: summary.run_count,
            attempt_count: summary.attempt_count,
        });
    }
    summary_records.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    let skipped = ledger
        .gaps(&["targetCompleted"])
        .into_iter()
        .filter(|gap| {
            gap.is_skipped()
                && gap.aspect.is_none()
                && gap
                    .label
                    .as_ref()
                    .is_some_and(|label| test_labels.contains(label))
        })
        .collect();
    let mut unbuilt: Vec<UnbuiltTest> = failed_completions
        .into_iter()
        .filter(|target| test_labels.contains(&target.label))
        .collect();
    unbuilt.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    let gaps = ledger.gaps(&["testResult", "testSummary"]);
    let mut filtered: Vec<UnbuiltTest> = passed_completions
        .into_iter()
        .filter(|target| {
            test_labels.contains(&target.label)
                && !results.iter().any(|result| result.label == target.label)
                && !summary_records
                    .iter()
                    .any(|summary| summary.label == target.label)
                && !gaps
                    .iter()
                    .any(|gap| gap.label.as_deref() == Some(target.label.as_str()))
                && !unbuilt.iter().any(|other| other.label == target.label)
        })
        .collect();
    filtered.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    filtered.dedup();
    let events = TestEvents {
        results,
        summaries: summary_records,
        ended: ledger.ended(),
        gaps,
        skipped,
        unbuilt,
        filtered,
        problem: None,
        no_coverage_collector,
    };
    Ok((events, lines.tail))
}

/// A top-level target completion and whether Bazel reported success.
fn target_completion(
    id: &serde_json::Map<String, Value>,
    object: &serde_json::Map<String, Value>,
) -> Option<(UnbuiltTest, bool)> {
    let completed = id.get("targetCompleted").and_then(Value::as_object)?;
    if completed.contains_key("aspect") {
        return None;
    }
    let success = object
        .get("completed")
        .and_then(Value::as_object)
        .and_then(|body| opt_bool(body, "success"))
        .unwrap_or(false);
    let target = UnbuiltTest {
        label: completed.get("label").and_then(Value::as_str)?.to_owned(),
        configuration: opt_configuration(completed),
    };
    Some((target, success))
}

/// The label of a configured target Bazel will test, from its test size.
fn test_label<'a>(
    id: &'a serde_json::Map<String, Value>,
    object: &serde_json::Map<String, Value>,
) -> Option<&'a str> {
    let configured = id.get("targetConfigured").and_then(Value::as_object)?;
    if configured.contains_key("aspect") {
        return None;
    }
    object
        .get("configured")
        .and_then(Value::as_object)
        .filter(|body| body.contains_key("testSize"))?;
    configured.get("label").and_then(Value::as_str)
}

/// Whether a `targetConfigured` body lists `tag` in its `tag` array.
fn has_tag(object: &serde_json::Map<String, Value>, tag: &str) -> bool {
    object
        .get("configured")
        .and_then(|body| body.get("tag"))
        .and_then(Value::as_array)
        .is_some_and(|tags| tags.iter().any(|value| value.as_str() == Some(tag)))
}

/// One `testSummary` body beside its id, with unknown fields left unknown.
fn parse_summary(
    summary_id: &serde_json::Map<String, Value>,
    object: &serde_json::Map<String, Value>,
) -> PendingSummary {
    let label = summary_id
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let body = object.get("testSummary").and_then(Value::as_object);
    PendingSummary {
        label,
        configuration: opt_configuration(summary_id),
        overall_status: body.and_then(|body| opt_string(body, "overallStatus")),
        total_num_cached: body.and_then(|body| opt_count(body, "totalNumCached")),
        run_count: body.and_then(|body| opt_count(body, "runCount")),
        attempt_count: body.and_then(|body| opt_count(body, "attemptCount")),
    }
}

/// The configuration id and output directory mnemonic of a configuration event.
fn configuration_mnemonic<'a>(
    id: &'a serde_json::Map<String, Value>,
    object: &'a serde_json::Map<String, Value>,
) -> Option<(&'a str, &'a str)> {
    let config = id.get("configuration")?.get("id")?.as_str()?;
    let mnemonic = object.get("configuration")?.get("mnemonic")?.as_str()?;
    (!mnemonic.is_empty()).then_some((config, mnemonic))
}

fn resolve_bytestream(
    locations: Option<&OutputLocations>,
    mnemonic: Option<&str>,
    label: &str,
    file: &PendingFile,
    identity: TestOutputIdentity,
) -> Result<PathBuf, BepError> {
    let Some(locations) = locations else {
        return Err(missing_test_output(
            label,
            &file.name,
            identity,
            "no output roots for bytestream test outputs",
        ));
    };
    if let Some(root) = locations.prefix_testlogs_root(&file.path_prefix) {
        return testlog_path_under(root, label, &file.name, identity);
    }
    testlog_path(locations, mnemonic, label, &file.name, identity)
}

fn record_shard_count(
    summary_id: &serde_json::Map<String, Value>,
    object: &serde_json::Map<String, Value>,
    line: u64,
    shard_counts: &mut HashMap<String, u32>,
) -> Result<(), BepError> {
    let label = summary_id
        .get("label")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(line, "id.testSummary.label", "test summary without label"))?;
    let summary = object.get("testSummary").and_then(Value::as_object);
    let Some(count) = summary.and_then(|summary| summary.get("shardCount")) else {
        return Ok(());
    };
    let raw = count.as_u64().ok_or_else(|| {
        malformed(
            line,
            "testSummary.shardCount",
            "shard count must be a non-negative integer",
        )
    })?;
    let count = u32::try_from(raw).map_err(|_| {
        malformed(
            line,
            "testSummary.shardCount",
            "shard count must be a non-negative integer",
        )
    })?;
    if count > 0 {
        let seen = shard_counts.entry(label.to_owned()).or_insert(0);
        *seen = (*seen).max(count);
    }
    Ok(())
}

fn run_totals(pending: &[PendingResult]) -> HashMap<&str, u32> {
    let mut totals: HashMap<&str, u32> = HashMap::new();
    for result in pending {
        if result.files.is_empty() {
            continue;
        }
        let seen = totals.entry(result.label.as_str()).or_insert(0);
        *seen = (*seen).max(result.run);
    }
    totals
}

fn attempt_totals(pending: &[PendingResult]) -> HashMap<(&str, u32, u32), u32> {
    let mut totals: HashMap<(&str, u32, u32), u32> = HashMap::new();
    for result in pending {
        if result.files.is_empty() {
            continue;
        }
        let key = (result.label.as_str(), result.run, result.shard);
        let seen = totals.entry(key).or_insert(0);
        *seen = (*seen).max(result.attempt);
    }
    totals
}

fn shard_max(pending: &[PendingResult], label: &str) -> u32 {
    pending
        .iter()
        .filter(|result| result.label == label && !result.files.is_empty())
        .map(|result| result.shard)
        .max()
        .unwrap_or(0)
}

fn test_index(
    result_id: &serde_json::Map<String, Value>,
    field: &str,
    path: &str,
    line: u64,
) -> Result<u32, BepError> {
    match result_id.get(field) {
        None => Ok(1),
        Some(value) => {
            let raw = value.as_u64().ok_or_else(|| {
                malformed(
                    line,
                    path,
                    &format!("test result {field} must be a positive integer"),
                )
            })?;
            u32::try_from(raw)
                .ok()
                .filter(|index| *index >= 1)
                .ok_or_else(|| {
                    malformed(
                        line,
                        path,
                        &format!("test result {field} must be a positive integer"),
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[derive(Debug, PartialEq, Eq)]
    struct Output {
        label: String,
        name: String,
        exec_path: PathBuf,
        run: u32,
        shard: u32,
        attempt: u32,
    }

    fn outputs(
        reader: impl BufRead,
        locations: impl FnOnce() -> Option<OutputLocations>,
    ) -> Result<Vec<Output>, BepError> {
        let events = collect_test_events(reader, locations)?;
        assert_eq!(events.problem, None);
        Ok(events
            .results
            .iter()
            .flat_map(|result| {
                result.files.iter().map(|file| Output {
                    label: result.label.clone(),
                    name: file.name.clone(),
                    exec_path: file.exec_path.clone(),
                    run: result.run,
                    shard: result.shard,
                    attempt: result.attempt,
                })
            })
            .collect())
    }

    fn test_result(label: &str, outputs: &[(&str, &str)]) -> String {
        let entries = outputs
            .iter()
            .map(|(name, uri)| format!(r#"{{"name":{name:?},"uri":{uri:?}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"id":{{"testResult":{{"label":{label:?},"run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{entries}]}}}}"#
        )
    }

    /// A hand-written file URI naming one path under a root this host can resolve.
    fn out_uri(rel: &str) -> String {
        if cfg!(windows) {
            format!("file:///C:/out/{rel}")
        } else {
            format!("file:///out/{rel}")
        }
    }

    /// The host path one fixture root plus its path parts name.
    fn want_path(root: &str, parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(root);
        for part in parts {
            path.push(part);
        }
        path
    }

    /// The host path one slash-separated relative name under the fixture root names.
    fn out_path(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { r"C:\out" } else { "/out" };
        want_path(root, &rel.split('/').collect::<Vec<_>>())
    }

    #[test]
    fn test_outputs_sort_results_and_keep_file_order() {
        let stream = [
            test_result(
                "//z:t",
                &[
                    ("test.xml", out_uri("z/test.xml").as_str()),
                    ("test.log", out_uri("z/test.log").as_str()),
                ],
            ),
            r#"{"id":{"targetCompleted":{"label":"//z:t"}},"completed":{"success":true}}"#
                .to_owned(),
            test_result("//a:t", &[("test.xml", out_uri("a/test.xml").as_str())]),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || None).expect("collect");
        assert_eq!(
            got,
            vec![
                Output {
                    label: "//a:t".to_owned(),
                    name: "test.xml".to_owned(),
                    exec_path: out_path("a/test.xml"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
                Output {
                    label: "//z:t".to_owned(),
                    name: "test.xml".to_owned(),
                    exec_path: out_path("z/test.xml"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
                Output {
                    label: "//z:t".to_owned(),
                    name: "test.log".to_owned(),
                    exec_path: out_path("z/test.log"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
            ]
        );
    }

    fn test_result_with_identity(
        label: &str,
        run: u32,
        shard: u32,
        attempt: u32,
        outputs: &[(&str, &str)],
    ) -> String {
        let entries = outputs
            .iter()
            .map(|(name, uri)| format!(r#"{{"name":{name:?},"uri":{uri:?}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"id":{{"testResult":{{"label":{label:?},"run":{run},"shard":{shard},"attempt":{attempt}}}}},"testResult":{{"status":"PASSED","testActionOutput":[{entries}]}}}}"#
        )
    }

    #[test]
    fn test_outputs_capture_run_shard_attempt() {
        let stream = [
            test_result_with_identity("//a:t", 2, 3, 4, &[("test.xml", out_uri("a.xml").as_str())]),
            test_result_with_identity("//a:t", 1, 1, 2, &[("test.xml", out_uri("b.xml").as_str())]),
            test_result_with_identity("//a:t", 1, 1, 1, &[("test.xml", out_uri("c.xml").as_str())]),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || None).expect("collect");
        assert_eq!(
            got.iter()
                .map(|output| (output.run, output.shard, output.attempt))
                .collect::<Vec<_>>(),
            vec![(1, 1, 1), (1, 1, 2), (2, 3, 4)],
            "records sort by run, shard, then attempt"
        );
        assert_eq!(got[0].exec_path, out_path("c.xml"));
        assert_eq!(got[2].exec_path, out_path("a.xml"));
    }

    #[test]
    fn test_outputs_default_missing_identity_to_one() {
        let stream = format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
            out_uri("a.xml")
        );
        let got = outputs(Cursor::new(stream), || None).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].run, got[0].shard, got[0].attempt), (1, 1, 1));
    }

    #[test]
    fn test_outputs_reject_non_positive_identity() {
        for field in ["run", "shard", "attempt"] {
            let stream = format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t","{field}":0}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            );
            assert!(
                outputs(Cursor::new(stream), || None).is_err(),
                "{field}=0 must fail"
            );
            let stream = format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t","{field}":"one"}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            );
            assert!(
                outputs(Cursor::new(stream), || None).is_err(),
                "{field} string must fail"
            );
        }
    }

    #[test]
    fn test_outputs_without_entries_or_results_are_ignored() {
        let stream = [
            r#"{"id":{"progress":{}}}"#.to_owned(),
            test_result("//a:t", &[]),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || None).expect("collect");
        assert!(got.is_empty());
    }

    #[test]
    fn test_output_shape_failures_fail_collection() {
        let events = collect_test_events(Cursor::new("not json"), || None).expect("cut-off tail");
        assert!(events.results.is_empty());
        assert!(events
            .problem
            .is_some_and(|problem| problem.starts_with("line 1:")));
        for stream in [
            r#"{"no-id":true}"#.to_owned(),
            r#"{"id":{"testResult":{}},"testResult":{"testActionOutput":[]}}"#.to_owned(),
            test_result("//a:t", &[("test.xml", "bytestream://remote/1")]),
            "[]".to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":{}}}"#
                .to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":["x"]}}"#
                .to_owned(),
            format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"testActionOutput":[{{"uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            )
                .to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":[{"name":"test.xml"}]}}"#
                .to_owned(),
        ] {
            assert!(
                outputs(Cursor::new(stream), || None).is_err(),
                "must fail closed"
            );
        }
    }

    #[test]
    fn test_output_malformed_reasons_carry_json_paths() {
        let cases = [
            (
                r#"{"id":{"testResult":{}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.label",
                "test result without label",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","run":0}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.run",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","shard":"one"}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.shard",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","attempt":4294967296}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.attempt",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":{}}}"#,
                "testResult.testActionOutput",
                "must be an array",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":["x"]}}"#,
                "testResult.testActionOutput[0]",
                "must be an object",
            ),
            (
                &format!(
                    r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"testActionOutput":[{{"uri":"{}"}}]}}}}"#,
                    out_uri("a.xml")
                ),
                "testResult.testActionOutput[0].name",
                "without name",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":[{"name":"test.xml"}]}}"#,
                "testResult.testActionOutput[0].uri",
                "without uri",
            ),
        ];
        for (stream, path, wording) in cases {
            let err = outputs(Cursor::new(stream), || None).expect_err("must fail closed");
            let BepError::MalformedEvent { line, reason } = err else {
                panic!("want MalformedEvent, got {err:?}");
            };
            assert_eq!(line, 1);
            assert!(
                reason.contains(path),
                "reason {reason:?} must name JSON path {path}"
            );
            assert!(
                reason.contains(wording),
                "reason {reason:?} must keep wording {wording:?}"
            );
        }
    }

    #[test]
    fn test_outputs_unreadable_stream_fails() {
        struct FailRead;
        impl std::io::Read for FailRead {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "boom"))
            }
        }
        assert!(outputs(std::io::BufReader::new(FailRead), || None).is_err());
    }

    fn test_summary(label: &str, shard_count: Option<u32>) -> String {
        let count = shard_count
            .map(|count| format!(r#","shardCount":{count}"#))
            .unwrap_or_default();
        format!(
            r#"{{"id":{{"testSummary":{{"label":{label:?}}}}},"testSummary":{{"status":"PASSED"{count}}}}}"#
        )
    }

    #[test]
    fn bytestream_test_outputs_resolve_through_the_workspace_fallback() {
        use std::path::Path;
        let ws = Path::new("/ws");
        let stream = [test_result(
            "//cli/bep:dx_bep_test",
            &[
                ("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10"),
                (
                    "test.lcov",
                    "bytestream://remote.buildbuddy.io/blobs/def/20",
                ),
            ],
        )]
        .join("\n");
        let locations = OutputLocations::new(ws);
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("workspace root");
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[1].exec_path,
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "coverage.dat"]
            )
        );
        assert_eq!(got[1].name, "test.lcov");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "test.xml"]
            )
        );
    }

    #[test]
    fn bytestream_test_outputs_resolve_from_reported_roots() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\n",
        );
        let stream = [
            test_summary("//a:t", Some(3)),
            test_result_with_identity(
                "//a:t",
                2,
                3,
                2,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
            test_result_with_identity(
                "//a:t",
                2,
                3,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/def/20")],
            ),
            test_result_with_identity(
                "@@dep+//pkg:t",
                1,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/ghi/30")],
            ),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("reported roots");
        assert_eq!(got.len(), 3);
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &[
                    "a",
                    "t",
                    "shard_3_of_3_run_2_of_2",
                    "test_attempts",
                    "attempt_1.xml"
                ]
            ),
            "a superseded attempt reads its own file"
        );
        assert_eq!(
            got[1].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &["a", "t", "shard_3_of_3_run_2_of_2", "test.xml"]
            ),
            "the final attempt reads the canonical file"
        );
        assert_eq!(
            got[2].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &["external", "dep+", "pkg", "t", "test.xml"]
            )
        );
        assert_ne!(got[0].exec_path, got[1].exec_path);
    }

    #[test]
    fn bytestream_path_prefix_is_the_testlogs_root_under_the_execution_root() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\n",
        );
        let stream = [format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t","run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"bytestream://remote.buildbuddy.io/blobs/abc/10","pathPrefix":["bazel-out","k8-fastbuild","testlogs"]}}]}}}}"#
        )]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("path prefix");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/execroot",
                &[
                    "bazel-out",
                    "k8-fastbuild",
                    "testlogs",
                    "a",
                    "t",
                    "test.xml"
                ]
            ),
            "path prefixes are execution-root relative"
        );
        let mut workspace_only = OutputLocations::new(Path::new("/ws"));
        workspace_only.apply_bazel_info(b"bazel-testlogs: /out/k8-fastbuild/testlogs\n");
        let stream = [format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t","run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"bytestream://remote.buildbuddy.io/blobs/abc/10","pathPrefix":["bazel-out","k8-fastbuild","testlogs","a","t"]}}]}}}}"#
        )]
        .join("\n");
        let got =
            outputs(Cursor::new(stream), || Some(workspace_only.clone())).expect("no execroot");
        assert_eq!(
            got[0].exec_path,
            want_path("/out/k8-fastbuild/testlogs", &["a", "t", "test.xml"]),
            "without an execution root the testlogs root still resolves"
        );
    }

    fn configuration_event(id: &str, mnemonic: &str) -> String {
        serde_json::json!({
            "id": {"configuration": {"id": id}},
            "configuration": {"mnemonic": mnemonic, "platformName": "k8"},
        })
        .to_string()
    }

    fn configured_result(label: &str, config: &str, run: u32, output: Value) -> String {
        serde_json::json!({
            "id": {"testResult": {
                "label": label,
                "run": run,
                "shard": 1,
                "attempt": 1,
                "configuration": {"id": config},
            }},
            "testResult": {"status": "PASSED", "testActionOutput": [output]},
        })
        .to_string()
    }

    fn bytestream_xml() -> Value {
        serde_json::json!({
            "name": "test.xml",
            "uri": "bytestream://remote.buildbuddy.io/blobs/abc/25",
        })
    }

    fn configuration_locations() -> OutputLocations {
        let mut locations = OutputLocations::new(std::path::Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\noutput_path: /out\n",
        );
        locations
    }

    #[test]
    fn bytestream_test_outputs_resolve_under_the_configuration_mnemonic() {
        let locations = configuration_locations();
        let stream = [
            configuration_event("cfgST", "k8-fastbuild-ST-2c5af747e1f8"),
            configured_result("//a:t", "cfgST", 1, bytestream_xml()),
            configured_result("//b:t", "cfgLate", 1, bytestream_xml()),
            configured_result("//c:t", "cfgUnknown", 1, bytestream_xml()),
            configured_result("//d:t", "cfgTop", 1, bytestream_xml()),
            configuration_event("cfgLate", "k8-opt-ST-0123456789ab"),
            configuration_event("cfgTop", "k8-fastbuild"),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("configuration");
        let paths: Vec<_> = got.iter().map(|file| file.exec_path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                want_path(
                    "/out",
                    &[
                        "k8-fastbuild-ST-2c5af747e1f8",
                        "testlogs",
                        "a",
                        "t",
                        "test.xml"
                    ]
                ),
                want_path(
                    "/out",
                    &["k8-opt-ST-0123456789ab", "testlogs", "b", "t", "test.xml"]
                ),
                want_path("/out/k8-fastbuild/testlogs", &["c", "t", "test.xml"]),
                want_path("/out", &["k8-fastbuild", "testlogs", "d", "t", "test.xml"]),
            ],
            "each result reads its own configuration's testlogs, and an unknown one falls back"
        );
    }

    #[test]
    fn bytestream_configuration_mnemonic_keeps_the_run_layout() {
        let locations = configuration_locations();
        let mut stream = vec![configuration_event("cfgST", "k8-fastbuild-ST-x")];
        for run in 1..=5 {
            stream.push(configured_result("//a:t", "cfgST", run, bytestream_xml()));
        }
        let got =
            outputs(Cursor::new(stream.join("\n")), || Some(locations.clone())).expect("runs");
        assert_eq!(got.len(), 5);
        for (index, file) in got.iter().enumerate() {
            let run_dir = format!("run_{}_of_5", index + 1);
            assert_eq!(
                file.exec_path,
                want_path(
                    "/out",
                    &[
                        "k8-fastbuild-ST-x",
                        "testlogs",
                        "a",
                        "t",
                        &run_dir,
                        "test.xml"
                    ]
                )
            );
        }
    }

    #[test]
    fn bytestream_path_prefix_wins_over_the_configuration_mnemonic() {
        let locations = configuration_locations();
        let output = serde_json::json!({
            "name": "test.xml",
            "uri": "bytestream://remote.buildbuddy.io/blobs/abc/25",
            "pathPrefix": ["bazel-out", "k8-fastbuild-ST-y", "testlogs"],
        });
        let stream = [
            configuration_event("cfgST", "k8-fastbuild-ST-x"),
            configured_result("//a:t", "cfgST", 1, output),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("prefix");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/execroot",
                &[
                    "bazel-out",
                    "k8-fastbuild-ST-y",
                    "testlogs",
                    "a",
                    "t",
                    "test.xml"
                ]
            )
        );
    }

    fn prefixed_xml(prefix: &[&str]) -> Value {
        serde_json::json!({
            "name": "test.xml",
            "uri": "bytestream://remote.buildbuddy.io/blobs/abc/25",
            "pathPrefix": prefix,
        })
    }

    #[test]
    fn bytestream_path_prefix_keeps_the_shard_layout() {
        let locations = configuration_locations();
        let prefix = ["bazel-out", "k8-fastbuild-ST-y", "testlogs"];
        let stream = [
            test_summary("//a:t", Some(2)),
            configured_result("//a:t", "cfgST", 1, prefixed_xml(&prefix)),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("shards");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/execroot",
                &[
                    "bazel-out",
                    "k8-fastbuild-ST-y",
                    "testlogs",
                    "a",
                    "t",
                    "shard_1_of_2",
                    "test.xml"
                ]
            )
        );
    }

    #[test]
    fn bytestream_path_prefix_keeps_the_external_repository_layout() {
        let locations = configuration_locations();
        let prefix = ["bazel-out", "k8-fastbuild", "testlogs"];
        let stream = configured_result("@@repo//p:t", "cfgTop", 1, prefixed_xml(&prefix));
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("external");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/execroot",
                &[
                    "bazel-out",
                    "k8-fastbuild",
                    "testlogs",
                    "external",
                    "repo",
                    "p",
                    "t",
                    "test.xml"
                ]
            )
        );
    }

    #[test]
    fn bytestream_path_prefix_with_a_parent_segment_falls_back_to_the_configuration() {
        let locations = configuration_locations();
        let prefix = ["bazel-out", "..", "testlogs"];
        let stream = [
            configuration_event("cfgST", "k8-fastbuild-ST-x"),
            configured_result("//a:t", "cfgST", 1, prefixed_xml(&prefix)),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("fallback");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out",
                &["k8-fastbuild-ST-x", "testlogs", "a", "t", "test.xml"]
            )
        );
    }

    #[test]
    fn test_outputs_skip_the_roots_lookup_for_file_uris() {
        let calls = std::cell::Cell::new(0u32);
        let stream = [test_result(
            "//a:t",
            &[("test.xml", out_uri("testlogs/a/t/test.xml").as_str())],
        )]
        .join("\n");
        let got = outputs(Cursor::new(stream), || {
            calls.set(calls.get() + 1);
            None
        })
        .expect("file outputs");
        assert_eq!(got.len(), 1);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn bytestream_test_outputs_look_up_roots_once() {
        use std::path::Path;
        let calls = std::cell::Cell::new(0u32);
        let uri = "bytestream://remote.buildbuddy.io/blobs/abc/10";
        let stream = [
            test_result_with_identity("//a:t", 1, 1, 1, &[("test.xml", uri)]),
            test_result_with_identity("//a:t", 1, 2, 1, &[("test.xml", uri)]),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || {
            calls.set(calls.get() + 1);
            Some(OutputLocations::new(Path::new("/ws")))
        })
        .expect("bytestream outputs");
        assert_eq!(got.len(), 2);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn bytestream_test_outputs_without_roots_name_label_and_identity() {
        let stream = [test_result_with_identity(
            "//a:t",
            2,
            3,
            1,
            &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
        )]
        .join("\n");
        let err = outputs(Cursor::new(stream), || None).expect_err("no roots must fail");
        let message = err.to_string();
        assert!(message.contains("//a:t"), "{message}");
        assert!(message.contains("run 2"), "{message}");
        assert!(message.contains("shard 3"), "{message}");
        assert!(message.contains("attempt 1"), "{message}");
        assert!(message.contains("test.xml"), "{message}");
    }

    #[test]
    fn shard_counts_shape_paths_even_when_one_shard_reports() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(b"bazel-testlogs: /out/testlogs\n");
        let stream = [
            test_summary("//a:t", Some(3)),
            test_result_with_identity(
                "//a:t",
                1,
                2,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("summary");
        assert_eq!(
            got[0].exec_path,
            want_path("/out/testlogs", &["a", "t", "shard_2_of_3", "test.xml"]),
            "testSummary.shardCount supplies the shard total"
        );
        let stream = [
            test_result_with_identity(
                "//a:t",
                1,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
            test_result_with_identity(
                "//a:t",
                3,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/def/20")],
            ),
        ]
        .join("\n");
        let got = outputs(Cursor::new(stream), || Some(locations.clone())).expect("runs");
        assert_eq!(
            got[1].exec_path,
            want_path("/out/testlogs", &["a", "t", "run_3_of_3", "test.xml"]),
            "the highest reported run is the run total"
        );
    }

    #[test]
    fn test_events_capture_result_metadata() {
        let stream = format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t","run":1,"shard":1,"attempt":2,"configuration":{{"id":"cfg1"}}}}}},"testResult":{{"status":"FAILED","cachedLocally":true,"testAttemptDurationMillis":"29","executionInfo":{{"strategy":"remote"}},"testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
            out_uri("a.xml")
        );
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        let result = &events.results[0];
        assert_eq!(result.label, "//a:t");
        assert_eq!(result.configuration.as_deref(), Some("cfg1"));
        assert_eq!((result.run, result.shard, result.attempt), (1, 1, 2));
        assert_eq!(result.status.as_deref(), Some("FAILED"));
        assert_eq!(result.cached_locally, Some(true));
        assert_eq!(result.duration_millis, Some(29));
        assert_eq!(result.strategy.as_deref(), Some("remote"));
        assert!(events.summaries.is_empty());
    }

    #[test]
    fn test_events_keep_results_before_a_cut_off_last_line() {
        let stream = format!(
            "{}\n{}",
            test_result("//a:t", &[("test.xml", &out_uri("a.xml"))]),
            r#"{"id":{"testRes"#
        );
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        assert_eq!(events.results[0].label, "//a:t");
        let problem = events.problem.expect("problem");
        assert!(problem.starts_with("line 2: invalid JSON"), "{problem}");
        assert!(!events.ended);
    }

    #[test]
    fn test_events_keep_results_before_a_cut_off_utf8_tail() {
        let mut stream = test_result("//a:t", &[("test.xml", &out_uri("a.xml"))]).into_bytes();
        stream.extend_from_slice(b"\n{\"id\":\"\xE2");
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        let problem = events.problem.expect("problem");
        assert!(
            problem.starts_with("line 2: unreadable stream line"),
            "{problem}"
        );
    }

    #[test]
    fn test_events_reject_a_bad_line_mid_stream() {
        let stream = format!(
            "not json\n{}",
            test_result("//a:t", &[("test.xml", &out_uri("a.xml"))])
        );
        let err = collect_test_events(Cursor::new(stream), || None).expect_err("must fail");
        let BepError::MalformedEvent { line, reason } = err else {
            panic!("want MalformedEvent, got {err:?}");
        };
        assert_eq!(line, 1);
        assert!(reason.starts_with("invalid JSON"), "{reason}");
    }

    #[test]
    fn test_events_without_problem_on_clean_stream() {
        let stream = format!(
            "{}\n\n",
            test_result("//a:t", &[("test.xml", &out_uri("a.xml"))])
        );
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        assert_eq!(events.problem, None);
    }

    #[test]
    fn test_events_capture_executed_result_without_cache_facts() {
        let stream = format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"status":"PASSED","testAttemptDurationMillis":23,"executionInfo":{{"strategy":"linux-sandbox"}},"testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
            out_uri("a.xml")
        );
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        let result = &events.results[0];
        assert_eq!(result.status.as_deref(), Some("PASSED"));
        assert_eq!(
            result.cached_locally, None,
            "an executed run without cachedLocally stays unknown, not false"
        );
        assert_eq!(result.duration_millis, Some(23));
        assert_eq!(result.strategy.as_deref(), Some("linux-sandbox"));
        assert_eq!(result.configuration, None);
    }

    #[test]
    fn test_events_capture_summary_metadata() {
        let stream = [
            test_summary("//a:t", Some(2)),
            r#"{"id":{"testSummary":{"label":"//a:t"}},"testSummary":{"overallStatus":"FLAKY","totalNumCached":1,"runCount":2,"attemptCount":3}}"#.to_owned(),
        ]
        .join("\n");
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.summaries.len(), 2);
        let summary = &events.summaries[1];
        assert_eq!(summary.label, "//a:t");
        assert_eq!(summary.overall_status.as_deref(), Some("FLAKY"));
        assert_eq!(summary.total_num_cached, Some(1));
        assert_eq!(summary.run_count, Some(2));
        assert_eq!(summary.attempt_count, Some(3));
    }

    #[test]
    fn test_events_leave_garbage_leaf_metadata_unknown() {
        let stream = format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"status":"PASSED","cachedLocally":"yes","testAttemptDurationMillis":"soon","executionInfo":{{"strategy":7}},"testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
            out_uri("a.xml")
        );
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert_eq!(events.results.len(), 1);
        let result = &events.results[0];
        assert_eq!(result.cached_locally, None);
        assert_eq!(result.duration_millis, None);
        assert_eq!(result.strategy, None);
    }

    fn configured_test(label: &str) -> String {
        format!(
            r#"{{"id":{{"targetConfigured":{{"label":"{label}"}}}},"configured":{{"testSize":"SMALL"}}}}"#
        )
    }

    fn configured_tagged_test(label: &str, tags: &str) -> String {
        format!(
            r#"{{"id":{{"targetConfigured":{{"label":"{label}"}}}},"configured":{{"testSize":"SMALL","tag":[{tags}]}}}}"#
        )
    }

    #[test]
    fn a_tagged_configured_test_has_no_coverage_collector() {
        let stream = [
            configured_tagged_test("//a:zig", r#""manual","dx-no-coverage-collector""#),
            configured_tagged_test("//a:rust", r#""manual""#),
            configured_test("//a:plain"),
            r#"{"id":{"targetConfigured":{"label":"//a:aspect","aspect":"//x:a.bzl%a"}},"configured":{"testSize":"SMALL","tag":["dx-no-coverage-collector"]}}"#.to_owned(),
        ]
        .join("\n");
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        let labels: Vec<&str> = events
            .no_coverage_collector
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(labels, ["//a:zig"]);
    }

    fn completed(label: &str, success: bool) -> String {
        format!(
            r#"{{"id":{{"targetCompleted":{{"label":"{label}"}}}},"completed":{{"success":{success}}}}}"#
        )
    }

    #[test]
    fn a_completed_test_without_summary_is_filtered() {
        let stream = [
            configured_test("//a:gone"),
            completed("//a:gone", true),
            configured_test("//a:ran"),
            completed("//a:ran", true),
            test_summary("//a:ran", Some(1)),
        ]
        .join("\n");
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        let labels: Vec<&str> = events.filtered.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["//a:gone"]);
    }

    #[test]
    fn failed_and_non_test_completions_are_not_filtered() {
        let stream = [
            configured_test("//a:broken"),
            completed("//a:broken", false),
            completed("//a:lib", true),
        ]
        .join("\n");
        let events = collect_test_events(Cursor::new(stream), || None).expect("events");
        assert!(events.filtered.is_empty(), "{:?}", events.filtered);
        assert_eq!(events.unbuilt.len(), 1);
    }
}
