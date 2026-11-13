//! Writes the coverage summary comment of one consumer coverage cell.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use dx_coverage_summary::{Inputs, PROFILE_IDS, Run, render};

#[cfg(test)]
mod main_tests;

#[derive(Debug, Parser)]
#[command(
    name = "coverage_summary",
    about = "Render the coverage summary comment of one CI cell",
    long_about = "coverage_summary reads the coverage step outputs under --runner-temp \
(dx_coverage.rc, dx-reports/coverage/coverage.ndjson, dx_changed_lines.rc, dx_changed_lines.txt \
and dx-reports/<build|test>/profile-<id>.json), writes --runner-temp/coverage_comment.md and \
appends it to --step-summary. An empty --min-changed-line-coverage reports the changed-line gate \
as not configured.\n\nExit codes: 0 success, 2 usage error, 1 write failure."
)]
struct Cli {
    /// Directory holding the coverage step outputs.
    #[arg(long)]
    runner_temp: PathBuf,
    /// File the comment is appended to.
    #[arg(long)]
    step_summary: PathBuf,
    /// Matrix platform of the cell.
    #[arg(long)]
    platform: String,
    /// Invocation id that tells two calls of the workflow apart.
    #[arg(long, default_value = "")]
    invocation_id: String,
    /// Changed-line coverage minimum; empty when not configured.
    #[arg(long, default_value = "")]
    min_changed_line_coverage: String,
    /// Analyzed revision.
    #[arg(long)]
    sha: String,
    /// GitHub server URL.
    #[arg(long)]
    server_url: String,
    /// Repository as owner/name.
    #[arg(long)]
    repository: String,
    /// Workflow run id.
    #[arg(long)]
    run_id: String,
}

fn optional(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn inputs(cli: &Cli) -> Inputs {
    let temp = &cli.runner_temp;
    let reports = temp.join("dx-reports");
    Inputs {
        platform: cli.platform.clone(),
        invocation_id: cli.invocation_id.clone(),
        run: Run {
            sha: cli.sha.clone(),
            server_url: cli.server_url.clone(),
            repository: cli.repository.clone(),
            run_id: cli.run_id.clone(),
        },
        dx_rc: optional(&temp.join("dx_coverage.rc")),
        events: optional(&reports.join("coverage/coverage.ndjson")).unwrap_or_default(),
        changed_lines_configured: !cli.min_changed_line_coverage.is_empty(),
        changed_rc: optional(&temp.join("dx_changed_lines.rc")),
        changed_lines: optional(&temp.join("dx_changed_lines.txt")).unwrap_or_default(),
        profiles: PROFILE_IDS
            .iter()
            .map(|id| {
                let path = reports.join(id).join(format!("profile-{id}.json"));
                ((*id).to_owned(), optional(&path).unwrap_or_default())
            })
            .collect(),
    }
}

fn write(cli: &Cli) -> std::io::Result<()> {
    let body = render(&inputs(cli));
    fs::write(cli.runner_temp.join("coverage_comment.md"), &body)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cli.step_summary)?
        .write_all(body.as_bytes())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match write(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("coverage_summary: {err}");
            ExitCode::FAILURE
        }
    }
}
