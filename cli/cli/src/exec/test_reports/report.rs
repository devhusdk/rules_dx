use super::*;

/// Prints each outcome in the run's output mode and counts the ones that were not superseded.
pub(super) fn emit_outcomes(
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
pub(super) const TEXT_CASE_LIMIT: usize = 3;

/// Prints `dx: failed: <target>: <cases> (log: <path>)` for one failed result.
pub(super) fn write_failed_line(err: &mut dyn Write, outcome: &TestOutcome) {
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
pub(super) struct ReportSink<'a> {
    pub(super) out: &'a mut dyn Write,
    pub(super) err: &'a mut dyn Write,
    pub(super) output: OutputMode,
    pub(super) workspace: &'a Path,
    pub(super) stdout_report: bool,
}

impl ReportSink<'_> {
    /// Writes one named report and announces it; false when the write failed and was reported.
    pub(super) fn write(
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
    pub(super) fn failed(&mut self, detail: &str, next: Option<&str>) -> Result<(), i32> {
        report_failed_with_next(self.out, self.err, self.output, detail, next)
    }
}

/// The verb's report document, or why it could not be rendered.
pub(super) fn render_report(
    verb: WorkflowVerb,
    suites: &[(String, Vec<JunitCase>)],
    lcov_documents: &[String],
    detail: &str,
    format: StandardFormat,
    sources_document: Option<&str>,
) -> Result<String, dx_error::DxError> {
    if format == StandardFormat::Sources {
        return match sources_document {
            Some(document) if detail.is_empty() => Ok(document.to_owned()),
            _ => Err(dx_error::DxError::failure(if detail.is_empty() {
                "no coverage source account is available"
            } else {
                detail
            })),
        };
    }
    match verb {
        WorkflowVerb::Test => {
            render_junit(suites).map_err(|error| dx_error::DxError::failure(error.to_string()))
        }
        WorkflowVerb::Coverage => {
            if lcov_documents.is_empty() {
                Err(dx_error::DxError::failure(detail))
            } else {
                let mut combined = lcov_documents.join("\n");
                if !combined.ends_with('\n') {
                    combined.push('\n');
                }
                Ok(combined)
            }
        }
        _ => Err(dx_error::DxError::failure(detail)), // LCOV_EXCL_LINE - reason: defensive arm, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    }
}

/// Reports why the results are incomplete, when evidence was missing or invalid.
pub(super) fn announce_incomplete(
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
pub(super) fn coverage_threshold(
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
pub(super) const WORST_FILES: usize = 10;

/// Prints the total coverage line (when no minimum prints it) and the least covered files.
pub(super) fn coverage_summary(
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
pub(super) fn account_sources(
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
pub(super) fn sources_report(sources: &Sources, account: &dx_lcov::CellAccount) -> String {
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
pub(super) fn coverage_sources(
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
pub(super) fn coverage_event(
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
pub(super) fn read_events(
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
