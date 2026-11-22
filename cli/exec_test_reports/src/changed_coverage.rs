use dx_error::DxError;
use dx_exec_core::common::{
    CODE_CHANGED_LINE_BELOW_MINIMUM, CODE_CHANGED_LINE_INCOMPLETE, CODE_CHANGED_LINE_UNAVAILABLE,
    emit_built, git_output,
};
use dx_lcov::{AddedLines, CellAccount, all_lines, changed_account, parse_added_lines};
use dx_output::{ChangedLineCoverageEvent, OutputMode};
use dx_plan::resolve::QueryRunner;
use dx_plan::resolve::base::{PlanError, named_base};
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

/// The lines the working tree adds since `merge_base`, with every untracked file counted in full.
///
/// # Errors
///
/// Returns `Err` when a git command fails or its output cannot be read.
pub fn changed_lines(
    workspace: &Path,
    runner: &dyn QueryRunner,
    merge_base: &str,
) -> Result<AddedLines, DxError> {
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
            "--src-prefix=a/",
            "--dst-prefix=b/",
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
        if path.is_empty() || dx_adopt::is_dx_state(&path) {
            continue;
        }
        let full = workspace.join(path.as_ref());
        let meta = match std::fs::metadata(&full) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(DxError::from_io(format!("{path}: {error}"), &error)),
        };
        if !meta.is_file() {
            continue;
        }
        let bytes = std::fs::read(&full)
            .map_err(|error| DxError::from_io(format!("{path}: {error}"), &error))?;
        let lines = all_lines(&String::from_utf8_lossy(&bytes));
        if !lines.is_empty() {
            added.insert(path.into_owned(), lines);
        }
    }
    Ok(added)
}

#[expect(clippy::cast_precision_loss, reason = "display only")]
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

fn outcome_report(
    result: dx_lcov::ChangedResult,
    changed: &dx_lcov::ChangedAccount,
    minimum: u32,
    merge_base: &str,
) -> (Option<(&'static str, String)>, Vec<String>) {
    let against = format!("against merge-base {merge_base}");
    let (covered, eligible) = (changed.covered_lines, changed.eligible_lines);
    match result {
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
    }
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
        "incomplete".clone_into(&mut event.result);
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
    let (branch, merge_base) = match named_base(gate.workspace, gate.runner, None) {
        Ok(named) => named,
        Err(PlanError::Git(message)) => return unavailable(event, message),
        Err(PlanError::Resolve(error)) => return unavailable(event, error.to_string()),
    };
    event.base = branch;
    event.merge_base.clone_from(&merge_base);
    let added = match changed_lines(gate.workspace, gate.runner, &merge_base) {
        Ok(added) => added,
        Err(error) => return unavailable(event, error.message().to_owned()),
    };
    let changed = changed_account(account, gate.lcov_documents, &added, &|path| {
        std::fs::read(gate.workspace.join(path))
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|error| dx_lcov::LcovError::Io {
                message: format!("{path}: {error}"),
            })
    });
    if !changed.errors.is_empty() {
        return unavailable(event, changed.errors.join("; "));
    }
    let result = changed.result(minimum);
    result.name().clone_into(&mut event.result);
    event.covered_lines = changed.covered_lines;
    event.eligible_lines = changed.eligible_lines;
    event.uncovered = changed
        .uncovered
        .iter()
        .map(|line| (line.path.clone(), line.line))
        .collect();
    event.uninstrumented.clone_from(&changed.uninstrumented);
    let (error, lines) = outcome_report(result, &changed, minimum, &merge_base);
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
            dx_output::write_stderr(err, format_args!("dx: {message} ({code})\n"));
        }
        for line in &lines {
            dx_output::write_stderr(err, format_args!("dx: {line}\n"));
        }
    }
    Ok(error.is_none())
}
