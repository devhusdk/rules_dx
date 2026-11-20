#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok,
    clippy::wildcard_enum_match_arm
)]

//! The test and coverage reports of `dx test` and `dx coverage`: test outcomes, `JUnit`,
//! LCOV, coverage sources and the changed-line gate.

use crate::coverage_candidate::{Candidate, Unavailable, candidate, tracked_orphans};
use dx_bep::{
    ArtifactReader, OutputLocations, TestEvents, TestResultFile, TestResultRecord,
    collect_test_events,
};
use dx_exec_core::common::{
    BAZEL_NO_TESTS, CODE_COVERAGE_BELOW_MINIMUM, CODE_COVERAGE_SOURCES,
    CODE_COVERAGE_SOURCES_INCOMPLETE, CODE_NO_COVERAGE_COLLECTOR, CODE_TESTS_FAILED,
    CODE_TESTS_FILTERED, CODE_WORKSPACE_WITHOUT_TESTS, FsArtifacts, bazel_exit_event,
    bazel_failure_code, bep_error_code, collect_output_account, emit_built, emit_event, kept_child,
    report_dx_error, report_failed_with_next, write_report_document,
};
use dx_exec_core::failure::CODE_NO_TESTS;
use dx_exec_reports::build_failures::report_build_failures;
use dx_exec_reports::failure_reports::write_failure_reports;
use dx_exec_reports::skipped_targets::{group_skipped, skipped_notice};
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

mod account;
mod changed_coverage;
mod coverage_candidate;
use account::BAZEL_TESTS_FAILED;
use account::{
    Evidence, Summaries, account_stream, attach_log, evidence_gap, filtered_notice,
    final_coverage_attempts, is_coverage_file, is_coverage_output, is_failed,
    mark_superseded_attempts, outcome_for, parse_cases, read_or_record, result_where,
    results_document, stable_exec_path, summary_statuses, tally_cases,
};
pub use account::{TestReportsRequest, output_locations};
pub use changed_coverage::changed_lines;
mod collect;
use collect::{
    Collected, Sources, announce_no_collector, collect_lcov, collect_sources, collect_test_xml,
};
mod report;
use report::{
    ReportSink, account_sources, announce_incomplete, coverage_event, coverage_sources,
    coverage_summary, coverage_threshold, emit_outcomes, read_events, render_report,
    sources_report,
};

struct Ctx<'a> {
    invocation: &'a Invocation,
    workspace: &'a Path,
    verb: WorkflowVerb,
    bep: &'a Path,
    bazel_code: i32,
    query_runner: &'a dyn QueryRunner,
    whole_repo: bool,
}

struct RunInputs<'a> {
    stdout_report: bool,
    planned_reports: &'a [PlannedReport],
    results_reports: &'a [PlannedReport],
    run_id: &'a str,
    failure_reports_ok: bool,
    actions: Option<ActionCounts>,
}

struct Gathered {
    events: TestEvents,
    suites: Vec<(String, Vec<JunitCase>)>,
    lcov_documents: Vec<String>,
    sources: Sources,
    reported: Vec<TestOutcome>,
    no_collector: Vec<(String, String)>,
    complete: bool,
    detail: String,
}

struct Emitted {
    counts: TestCounts,
    reports_ok: bool,
    gates_ok: bool,
}

pub fn execute_test_reports(request: TestReportsRequest<'_>) -> i32 {
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
    let ctx = Ctx {
        invocation,
        workspace,
        verb,
        bep,
        bazel_code,
        query_runner,
        whole_repo,
    };
    let (failure_count, failure_reports_ok) = match failure_phase(
        invocation,
        workspace,
        bep,
        bazel_code,
        failure_reports,
        out,
        err,
    ) {
        Ok(phase) => phase,
        Err(code) => return code,
    };
    let mut gathered = match gather(&ctx, failure_count, results_complete, actions, out, err) {
        Ok(gathered) => gathered,
        Err(code) => return code,
    };
    let run = RunInputs {
        stdout_report,
        planned_reports,
        results_reports,
        run_id,
        failure_reports_ok,
        actions,
    };
    let emitted = match emit_phase(&ctx, &run, &mut gathered, out, err) {
        Ok(emitted) => emitted,
        Err(code) => return code,
    };
    *results_complete = gathered.complete && emitted.reports_ok;
    finish(&ctx, &run, &gathered, &emitted, out, err)
}

fn gather(
    ctx: &Ctx<'_>,
    failure_count: usize,
    results_complete: &mut bool,
    actions: Option<ActionCounts>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Gathered, i32> {
    let Ctx {
        invocation,
        workspace,
        verb,
        bep,
        bazel_code,
        query_runner,
        whole_repo,
    } = *ctx;
    let read = read_events(invocation, workspace, verb, bep, bazel_code, query_runner);
    let sources = if verb == WorkflowVerb::Coverage
        && read.is_ok()
        && (bazel_code == 0 || bazel_code == BAZEL_TESTS_FAILED)
    {
        collect_sources(bep, workspace)
    } else {
        Sources::default()
    };
    dx_exec_core::common::remove_scratch(bep);
    let (events, stream_problem) = match read {
        Ok(read) => read,
        Err(error) => {
            return Err(report_dx_error(
                invocation.output,
                out,
                err,
                bep_error_code(&error),
                &error,
            ));
        }
    };
    if verb == WorkflowVerb::Test
        && bazel_code == BAZEL_NO_TESTS
        && whole_repo
        && failure_count == 0
        && events.results.is_empty()
        && events.skipped.is_empty()
    {
        *results_complete = true;
        return Err(no_tests_in_workspace(invocation.output, out, err, actions));
    }
    let reader = FsArtifacts;
    let mut evidence = Evidence::default();
    if let Some(problem) = stream_problem {
        evidence.record(problem);
    }
    let summaries = summary_statuses(&events);
    let Collected {
        mut suites,
        lcov_documents,
        sources,
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
    Ok(Gathered {
        events,
        suites,
        lcov_documents,
        sources,
        reported,
        no_collector,
        complete,
        detail,
    })
}

fn emit_phase(
    ctx: &Ctx<'_>,
    run: &RunInputs<'_>,
    gathered: &mut Gathered,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Emitted, i32> {
    let Ctx {
        invocation,
        workspace,
        verb,
        bazel_code,
        query_runner,
        whole_repo,
        ..
    } = *ctx;
    let Gathered {
        events,
        suites,
        lcov_documents,
        sources,
        reported,
        no_collector,
        complete,
        detail,
    } = gathered;
    let complete = *complete;
    let counts = emit_outcomes(invocation.output, out, err, reported, &events.filtered)
        .map_err(|exit| kept_child(err, bazel_code, exit))?;
    let groups = group_skipped(&events.skipped, invocation, workspace, query_runner);
    skipped_notice(invocation.output, out, err, &groups)
        .map_err(|exit| kept_child(err, bazel_code, exit))?;
    let mut sink = ReportSink {
        out: &mut *out,
        err: &mut *err,
        output: invocation.output,
        workspace,
        stdout_report: run.stdout_report,
    };
    let mut reports_ok = run.failure_reports_ok;
    match write_results_reports(
        &mut sink,
        run.results_reports,
        reported,
        complete,
        bazel_code,
        run.run_id,
    ) {
        Ok(landed) => reports_ok &= landed,
        Err(exit) => return Err(kept_child(err, bazel_code, exit)),
    }
    let account = (verb == WorkflowVerb::Coverage && detail.is_empty()).then(|| {
        account_coverage(
            sources,
            lcov_documents,
            invocation,
            workspace,
            query_runner,
            whole_repo,
        )
    });
    let sources_document = account
        .as_ref()
        .map(|account| sources_report(sources, account));
    let planned = PlannedRun {
        verb,
        suites,
        lcov_documents,
        detail,
        sources_document: sources_document.as_deref(),
        complete,
    };
    match write_planned_reports(&mut sink, run.planned_reports, &planned, reports_ok) {
        Ok(ok) => reports_ok = ok,
        Err(exit) => return Err(kept_child(err, bazel_code, exit)),
    }
    announce_incomplete(invocation.output, out, err, detail)
        .map_err(|exit| kept_child(err, bazel_code, exit))?;
    let strict = dx_output::meets_threshold(dx_output::Severity::Warning, invocation.fail_on);
    let collector_ok = announce_no_collector(invocation.output, out, err, no_collector, strict)
        .map_err(|exit| kept_child(err, bazel_code, exit))?;
    let inputs = CoverageInputs {
        invocation,
        verb,
        workspace,
        query_runner,
        account: account.as_ref(),
        sources,
        lcov_documents,
    };
    let (threshold_ok, sources_ok, changed_ok) =
        coverage_gates(&inputs, out, err).map_err(|exit| kept_child(err, bazel_code, exit))?;
    Ok(Emitted {
        counts,
        reports_ok,
        gates_ok: threshold_ok && collector_ok && sources_ok && changed_ok,
    })
}

fn finish(
    ctx: &Ctx<'_>,
    run: &RunInputs<'_>,
    gathered: &Gathered,
    emitted: &Emitted,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Ctx {
        invocation,
        bazel_code,
        ..
    } = *ctx;
    let reported = &gathered.reported;
    let counts = &emitted.counts;
    let code = if bazel_code != 0 {
        bazel_code
    } else {
        i32::from(!(gathered.complete && emitted.reports_ok && emitted.gates_ok))
    };
    let no_tests = bazel_code == BAZEL_NO_TESTS && reported.is_empty();
    if no_tests {
        dx_output::write_stderr(
            err,
            format_args!("dx: {CODE_NO_TESTS}: no test targets match the scope\n"),
        );
        if let Some(next) = dx_exec_core::guidance::next_action(CODE_NO_TESTS) {
            dx_output::write_stderr(err, format_args!("{next}\n"));
        }
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_exit_events(out, invocation, bazel_code, counts, no_tests) {
            return kept_child(err, bazel_code, exit);
        }
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(gathered.complete && emitted.reports_ok),
                run_id: (!run.results_reports.is_empty()).then(|| run.run_id.to_owned()),
                tests: (!reported.is_empty()).then(|| counts.clone()),
                actions: run.actions,
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return kept_child(err, bazel_code, exit);
        }
    }
    code
}

fn failure_phase(
    invocation: &Invocation,
    workspace: &Path,
    bep: &Path,
    bazel_code: i32,
    failure_reports: &[PlannedReport],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(usize, bool), i32> {
    let failure_count = if bazel_code != 0 {
        match report_build_failures(bep, invocation.output, out, err) {
            Ok(reported) => reported.count,
            Err(exit) => return Err(kept_child(err, bazel_code, exit)),
        }
    } else {
        0
    };
    match write_failure_reports(
        invocation,
        workspace,
        bep,
        bazel_code,
        failure_reports,
        out,
        err,
    ) {
        Ok(landed) => Ok((failure_count, landed)),
        Err(exit) => Err(kept_child(err, bazel_code, exit)),
    }
}

fn write_results_reports(
    sink: &mut ReportSink<'_>,
    results_reports: &[PlannedReport],
    reported: &[TestOutcome],
    complete: bool,
    bazel_code: i32,
    run_id: &str,
) -> Result<bool, i32> {
    let mut landed_all = true;
    if !results_reports.is_empty() {
        let document = match results_document(reported, complete, bazel_code, run_id) {
            Ok(document) => document,
            Err(error) => {
                sink.failed(&format!("cannot build the results report: {error}"), None)?;
                return Ok(false);
            }
        };
        for planned in results_reports {
            landed_all &= sink.write("results", &planned.destination, &document, complete)?;
        }
    }
    Ok(landed_all)
}

fn account_coverage(
    sources: &mut Sources,
    lcov_documents: &[String],
    invocation: &Invocation,
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    whole_repo: bool,
) -> CellAccount {
    let mut account = account_sources(sources, lcov_documents, workspace);
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
    account
}

struct PlannedRun<'a> {
    verb: WorkflowVerb,
    suites: &'a [(String, Vec<JunitCase>)],
    lcov_documents: &'a [String],
    detail: &'a str,
    sources_document: Option<&'a str>,
    complete: bool,
}

fn write_planned_reports(
    sink: &mut ReportSink<'_>,
    planned_reports: &[PlannedReport],
    run: &PlannedRun<'_>,
    mut reports_ok: bool,
) -> Result<bool, i32> {
    for planned in planned_reports {
        let name = planned.format.name();
        let document = match render_report(
            run.verb,
            run.suites,
            run.lcov_documents,
            run.detail,
            planned.format,
            run.sources_document,
        ) {
            Ok(document) => document,
            Err(reason) => {
                reports_ok = false;
                let message = format!("failed to render {name} report: {reason}");
                let next = (!run.detail.is_empty() && reason.message() == run.detail)
                    .then_some(dx_exec_core::guidance::EVIDENCE_GAP_NEXT);
                sink.failed(&message, next)?;
                continue;
            }
        };
        let complete_so_far = run.complete && reports_ok;
        reports_ok &= sink.write(name, &planned.destination, &document, complete_so_far)?;
    }
    Ok(reports_ok)
}

struct CoverageInputs<'a> {
    invocation: &'a Invocation,
    verb: WorkflowVerb,
    workspace: &'a Path,
    query_runner: &'a dyn QueryRunner,
    account: Option<&'a CellAccount>,
    sources: &'a Sources,
    lcov_documents: &'a [String],
}

fn coverage_gates(
    inputs: &CoverageInputs<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(bool, bool, bool), i32> {
    let CoverageInputs {
        invocation,
        verb,
        workspace,
        query_runner,
        account,
        sources,
        lcov_documents,
    } = *inputs;
    let coverage = verb == WorkflowVerb::Coverage;
    let threshold_ok = if coverage {
        coverage_threshold(
            invocation.min_coverage,
            account,
            sources,
            invocation.output,
            out,
            err,
        )?
    } else {
        true
    };
    if coverage && let Some(account) = account {
        coverage_summary(
            account,
            invocation.min_coverage.is_none(),
            invocation.output,
            err,
        );
    }
    let sources_ok = match account {
        Some(account) => {
            let enforce = invocation.min_coverage.is_some();
            coverage_sources(sources, account, invocation.output, enforce, out, err)? || !enforce
        }
        None => true,
    };
    if let Some(account) = account {
        coverage_event(
            sources,
            account,
            invocation.min_coverage,
            invocation.output,
            out,
        )?;
    }
    let changed_ok = if coverage {
        let gate = crate::changed_coverage::ChangedGate {
            minimum: invocation.min_changed_line_coverage,
            account,
            lcov_documents,
            workspace,
            runner: query_runner,
            output: invocation.output,
        };
        crate::changed_coverage::changed_line_threshold(&gate, out, err)?
    } else {
        true
    };
    Ok((threshold_ok, sources_ok, changed_ok))
}

fn emit_exit_events(
    out: &mut dyn Write,
    invocation: &Invocation,
    bazel_code: i32,
    counts: &TestCounts,
    no_tests: bool,
) -> Result<(), i32> {
    if bazel_code == BAZEL_TESTS_FAILED && counts.failed > 0 {
        let message = if counts.failed == 1 {
            "1 test failed; read the test_outcome events".to_owned()
        } else {
            format!(
                "{} tests failed; read the test_outcome events",
                counts.failed
            )
        };
        emit_built(
            out,
            bazel_exit_event(CODE_TESTS_FAILED, &message, bazel_code, "execute"),
        )
    } else if no_tests {
        emit_built(
            out,
            bazel_exit_event(
                CODE_NO_TESTS,
                "no test targets match the scope",
                bazel_code,
                "execute",
            ),
        )
    } else if bazel_code != 0 {
        emit_built(
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
    } else {
        Ok(())
    }
}

/// The workspace has no test targets: `dx test //...` passes with a notice.
fn no_tests_in_workspace(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    actions: Option<ActionCounts>,
) -> i32 {
    let message = "the workspace has no test targets";
    match output {
        OutputMode::Json => {
            if let Err(exit) = emit_built(
                out,
                notice_event(&NoticeEvent::info(CODE_WORKSPACE_WITHOUT_TESTS, message)),
            ) {
                return exit;
            }
            let finished = command_finished(
                0,
                &FinishedCounts {
                    results_complete: Some(true),
                    actions,
                    ..FinishedCounts::default()
                },
            );
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
        }
        OutputMode::Text { .. } => {
            dx_output::write_stderr(err, format_args!("dx: {message}\n"));
        }
        OutputMode::Diff => {}
    }
    0
}

#[cfg(test)]
#[path = "test_reports_tests.rs"]
mod tests;
