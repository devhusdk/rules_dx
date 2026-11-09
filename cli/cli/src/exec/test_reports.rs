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

mod account;
use account::BAZEL_TESTS_FAILED;
use account::*;
pub(crate) use account::{TestReportsRequest, output_locations};
mod collect;
use collect::*;
mod report;
use report::*;

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
