//! Prepares the advisory snapshots one CI run shares with every audit cell.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use dx_advisory_prep::{prepare, PrepareOptions, Summary, DEFAULT_OUT};
use dx_audit::advisory::is_audit_date;
use dx_audit::advisory_prep::{FetchLimits, PrepError, DEFAULT_LIMITS};

#[derive(Debug, Parser)]
#[command(
    name = "advisory_prep",
    about = "Prepare the advisory snapshots one run shares across platforms",
    long_about = "advisory_prep fetches one OSV archive per advisory ecosystem the workspace's \
dependency sets need, converts each archive into one canonical snapshot plus its digest \
sidecar, and writes both under --out. Every platform cell reads those files instead of \
downloading a database of its own. Relative paths anchor at BUILD_WORKSPACE_DIRECTORY (the \
bazel run workspace root), else the current directory.\n\n--workspace DIR reads the dependency \
set locks (default the workspace root); --out DIR writes the snapshots (default .dx/advisory \
under it); --date YYYY-MM-DD stamps \
every sidecar (default today, UTC); --max-time, --connect-timeout, --retries and --retry-delay \
bound one download (defaults 600, 30, 3 and 5 seconds); --archive FAMILY=PATH converts a local \
archive instead of fetching it; --status-file FILE appends status, ecosystems and families \
as key=value lines (status is empty when no dependency set needs an ecosystem).\n\nExit codes: 0 success, 2 usage error, 1 preparation \
failure."
)]
struct Cli {
    /// Workspace holding the dependency set locks.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Directory the snapshots are written to.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Date every sidecar is stamped with.
    #[arg(long)]
    date: Option<String>,
    /// Seconds one download may take.
    #[arg(long)]
    max_time: Option<u32>,
    /// Seconds one download may spend connecting.
    #[arg(long)]
    connect_timeout: Option<u32>,
    /// Retries one download may spend.
    #[arg(long)]
    retries: Option<u32>,
    /// Seconds between two download attempts.
    #[arg(long)]
    retry_delay: Option<u32>,
    /// Convert a local archive instead of fetching it.
    #[arg(long = "archive", value_name = "FAMILY=PATH")]
    archives: Vec<String>,
    /// File the status, ecosystem count and families are appended to.
    #[arg(long)]
    status_file: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match run(cli) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<Vec<String>, PrepError> {
    let root = run_root(std::env::var_os("BUILD_WORKSPACE_DIRECTORY").as_deref());
    let status_file = cli
        .status_file
        .clone()
        .map(|path| anchor(root.as_deref(), path));
    let options = options_from(cli, root.as_deref())?;
    let prepared = prepare(&options)?;
    let summary = Summary::of(&prepared);
    if let Some(path) = status_file {
        append_status(&path, &summary)?;
    }
    if prepared.is_empty() {
        return Ok(vec![
            "no advisory ecosystems needed: no audited dependency lock found".to_owned(),
        ]);
    }
    Ok(prepared
        .into_iter()
        .map(|entry| {
            format!(
                "{}: {} advisories sha256={} path={}",
                entry.family, entry.advisories, entry.identity, entry.path
            )
        })
        .collect())
}

fn append_status(path: &std::path::Path, summary: &Summary) -> Result<(), PrepError> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(summary.render().as_bytes()))
        .map_err(|error| PrepError::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        })
}

fn run_root(value: Option<&OsStr>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    (!path.as_os_str().is_empty() && path.is_absolute()).then_some(path)
}

fn anchor(root: Option<&Path>, path: PathBuf) -> PathBuf {
    match root {
        Some(root) if path.is_relative() => {
            if path == Path::new(".") {
                root.to_path_buf()
            } else {
                root.join(path)
            }
        }
        _ => path,
    }
}

fn options_from(cli: Cli, root: Option<&Path>) -> Result<PrepareOptions, PrepError> {
    let retrieved_at = match cli.date.clone() {
        Some(date) => {
            if !is_audit_date(&date) {
                return Err(PrepError::BadDate { retrieved_at: date });
            }
            date
        }
        None => today_utc(),
    };
    let limits = limits_from(&cli);
    let archives = archives_from(&cli.archives)?
        .into_iter()
        .map(|(family, path)| (family, anchor(root, path)))
        .collect();
    Ok(PrepareOptions {
        workspace: anchor(root, cli.workspace.unwrap_or_else(|| PathBuf::from("."))),
        out: anchor(root, cli.out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT))),
        retrieved_at,
        limits,
        archives,
    })
}

fn limits_from(cli: &Cli) -> FetchLimits {
    FetchLimits {
        connect_timeout_seconds: cli
            .connect_timeout
            .unwrap_or(DEFAULT_LIMITS.connect_timeout_seconds),
        max_seconds: cli.max_time.unwrap_or(DEFAULT_LIMITS.max_seconds),
        retries: cli.retries.unwrap_or(DEFAULT_LIMITS.retries),
        retry_delay_seconds: cli
            .retry_delay
            .unwrap_or(DEFAULT_LIMITS.retry_delay_seconds),
    }
}

fn archives_from(values: &[String]) -> Result<BTreeMap<String, PathBuf>, PrepError> {
    let mut out = BTreeMap::new();
    for value in values {
        let malformed = || PrepError::Usage {
            detail: format!("--archive {value:?} wants FAMILY=PATH"),
        };
        let Some((family, path)) = value.split_once('=') else {
            return Err(malformed());
        };
        if family.is_empty() || path.is_empty() {
            return Err(malformed());
        }
        out.insert(family.to_owned(), PathBuf::from(path));
    }
    Ok(out)
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod main_tests;
