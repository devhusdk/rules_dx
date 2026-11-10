use super::*;

/// The aspect that writes coverage source manifests, as the build events name it.
pub(super) const SOURCES_ASPECT_SUFFIX: &str = "coverage_sources.bzl%dx_coverage_sources_aspect";

/// The coverage source manifests a run reported, and every problem that left one missing.
#[derive(Default)]
pub(super) struct Sources {
    pub(super) manifests: Vec<SourceManifest>,
    pub(super) missing: Vec<String>,
    pub(super) invalid: Vec<String>,
    /// `bazel <version>` from the build events' `started` event.
    pub(super) collector: Option<String>,
    /// The checkout measured, when the workspace is a Git work tree.
    pub(super) candidate: Option<Candidate>,
    /// Why the tracked-source check did not run, when it is not an accounting error.
    pub(super) tracked_unavailable: Option<String>,
}

/// Reads the coverage source manifests from the run's build events.
pub(super) fn collect_sources(bep: &Path, workspace: &Path) -> Sources {
    let mut sources = Sources {
        collector: dx_bep::stream_build_tool_version(bep)
            .ok()
            .flatten()
            .map(|version| format!("bazel {version}")),
        ..Sources::default()
    };
    let account = match collect_output_account(bep, SOURCES_OUTPUT_GROUP, workspace) {
        Ok(account) => account,
        Err(error) => {
            sources
                .missing
                .push(format!("coverage source manifests are unreadable: {error}"));
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
pub(super) struct Collected {
    pub(super) suites: Vec<(String, Vec<JunitCase>)>,
    pub(super) lcov_documents: Vec<String>,
    pub(super) sources: Sources,
    pub(super) reported: Vec<TestOutcome>,
    pub(super) gap: Option<String>,
    /// Sorted, deduplicated (label, language) pairs that ran under coverage without a collector.
    pub(super) no_collector: Vec<(String, String)>,
}

/// Reads every result's test.xml into suites and outcomes for `dx test`.
pub(super) fn collect_test_xml(
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
pub(super) fn collect_lcov(
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
pub(super) fn no_collector_labels(events: &TestEvents) -> Vec<(String, String)> {
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
pub(super) fn announce_no_collector(
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
