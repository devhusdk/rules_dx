use super::affected::{named_base, PlanError};
use super::common::{
    emit_built, CODE_CHANGED_LINE_BELOW_MINIMUM, CODE_CHANGED_LINE_INCOMPLETE,
    CODE_CHANGED_LINE_UNAVAILABLE,
};
use dx_lcov::{all_lines, changed_account, parse_added_lines, AddedLines, CellAccount};
use dx_output::{ChangedLineCoverageEvent, OutputMode};
use dx_plan::resolve::QueryRunner;
use std::io::Write;
use std::path::Path;

/// What the changed-line gate needs from one `dx coverage` run.
pub(super) struct ChangedGate<'a> {
    pub(super) minimum: Option<u32>,
    pub(super) account: Option<&'a CellAccount>,
    pub(super) lcov_documents: &'a [String],
    pub(super) workspace: &'a Path,
    pub(super) runner: &'a dyn QueryRunner,
    pub(super) output: OutputMode,
}

fn git_output(
    runner: &dyn QueryRunner,
    workspace: &Path,
    what: &str,
    args: &[&str],
) -> Result<Vec<u8>, String> {
    let mut argv = vec!["git".to_owned()];
    argv.extend(args.iter().map(ToString::to_string));
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| format!("git {what} failed: {error}"))?;
    if result.code != Some(0) {
        let detail = dx_output::first_diagnostic_line(&result.stderr, 200, "no Git diagnostic");
        return Err(format!("git {what} failed: {detail}"));
    }
    Ok(result.stdout)
}

/// The lines the working tree adds since `merge_base`, with every untracked file counted in full.
pub(super) fn changed_lines(
    workspace: &Path,
    runner: &dyn QueryRunner,
    merge_base: &str,
) -> Result<AddedLines, String> {
    let diff = git_output(
        runner,
        workspace,
        "diff",
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-U0",
            "-M",
            "--relative",
            merge_base,
        ],
    )?;
    let mut added = parse_added_lines(&String::from_utf8_lossy(&diff));
    let untracked = git_output(
        runner,
        workspace,
        "ls-files",
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for raw in untracked.split(|byte| *byte == 0) {
        let path = String::from_utf8_lossy(raw);
        if path.is_empty() || path == ".dx" || path.starts_with(".dx/") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(workspace.join(path.as_ref())) {
            let lines = all_lines(&text);
            if !lines.is_empty() {
                added.insert(path.into_owned(), lines);
            }
        }
    }
    Ok(added)
}

fn percent(covered: u64, eligible: u64) -> String {
    format!(
        "{:.2}% ({covered}/{eligible} lines)",
        100.0 * covered as f64 / eligible as f64
    )
}

/// One gate verdict, ready to print.
struct Verdict {
    event: ChangedLineCoverageEvent,
    error: Option<(&'static str, String)>,
    lines: Vec<String>,
}

fn verdict(gate: &ChangedGate<'_>, minimum: u32) -> Verdict {
    let mut event = ChangedLineCoverageEvent {
        base: String::new(),
        merge_base: String::new(),
        minimum,
        covered_lines: 0,
        eligible_lines: 0,
        uncovered: Vec::new(),
        uninstrumented: Vec::new(),
        result: "unavailable".to_owned(),
        dx_version: dx_adopt::DX_VERSION.to_owned(),
    };
    let Some(account) = gate.account else {
        event.result = "incomplete".to_owned();
        return Verdict {
            event,
            error: Some((
                CODE_CHANGED_LINE_INCOMPLETE,
                "no coverage source account is available".to_owned(),
            )),
            lines: Vec::new(),
        };
    };
    let unavailable = |event: ChangedLineCoverageEvent, message: String| Verdict {
        event,
        error: Some((CODE_CHANGED_LINE_UNAVAILABLE, message)),
        lines: Vec::new(),
    };
    let (branch, merge_base) = match named_base(gate.workspace, gate.runner) {
        Ok(named) => named,
        Err(PlanError::Git(message)) => return unavailable(event, message),
        Err(PlanError::Resolve(error)) => return unavailable(event, error.to_string()),
    };
    event.base = branch;
    event.merge_base = merge_base.clone();
    let added = match changed_lines(gate.workspace, gate.runner, &merge_base) {
        Ok(added) => added,
        Err(message) => return unavailable(event, message),
    };
    let changed = changed_account(account, gate.lcov_documents, &added, &|path| {
        std::fs::read_to_string(gate.workspace.join(path)).map_err(|error| dx_lcov::LcovError::Io {
            message: format!("{path}: {error}"),
        })
    });
    let result = changed.result(minimum);
    event.result = result.name().to_owned();
    event.covered_lines = changed.covered_lines;
    event.eligible_lines = changed.eligible_lines;
    event.uncovered = changed
        .uncovered
        .iter()
        .map(|line| (line.path.clone(), line.line))
        .collect();
    event.uninstrumented = changed.uninstrumented.clone();
    let against = format!("against merge-base {merge_base}");
    let (covered, eligible) = (changed.covered_lines, changed.eligible_lines);
    let (error, lines) = match result {
        dx_lcov::ChangedResult::Pass => (
            None,
            vec![format!(
                "changed-line coverage {} meets minimum {minimum}% {against}",
                percent(covered, eligible)
            )],
        ),
        dx_lcov::ChangedResult::NotApplicable => (
            None,
            vec![format!(
                "changed-line coverage not applicable: no changed executable lines {against}"
            )],
        ),
        dx_lcov::ChangedResult::Fail => (
            Some((
                CODE_CHANGED_LINE_BELOW_MINIMUM,
                format!(
                    "changed-line coverage {} below minimum {minimum}% {against}",
                    percent(covered, eligible)
                ),
            )),
            changed
                .uncovered
                .iter()
                .map(|line| format!("changed_line_uncovered: {}:{}", line.path, line.line))
                .collect(),
        ),
        dx_lcov::ChangedResult::Incomplete => (
            Some((
                CODE_CHANGED_LINE_INCOMPLETE,
                format!(
                    "changed sources absent from the coverage report {against}: {}",
                    changed.uninstrumented.join(", ")
                ),
            )),
            Vec::new(),
        ),
    };
    Verdict {
        event,
        error,
        lines,
    }
}

/// Gates the lines changed since the merge-base on `--min-changed-line-coverage`; true without the flag.
pub(super) fn changed_line_threshold(
    gate: &ChangedGate<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let Some(minimum) = gate.minimum else {
        return Ok(true);
    };
    let Verdict {
        event,
        error,
        lines,
    } = verdict(gate, minimum);
    if gate.output == OutputMode::Json {
        emit_built(out, dx_output::changed_line_coverage_event(&event))?;
        if let Some((code, message)) = &error {
            emit_built(out, dx_output::error_event(code, message, None, None, None))?;
        }
    } else {
        if let Some((code, message)) = &error {
            let _ = writeln!(err, "dx: {code}: {message}");
        }
        for line in &lines {
            let _ = writeln!(err, "dx: {line}");
        }
    }
    Ok(error.is_none())
}

#[cfg(test)]
#[path = "changed_coverage_tests.rs"]
mod changed_coverage_tests;
