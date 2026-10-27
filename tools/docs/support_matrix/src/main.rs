//! Checks, writes, or lists the gaps of `docs/support-matrix.md`.

use std::path::Path;
use std::process::ExitCode;

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{gaps, generate, parse_facts, Source, MATRIX_PATH, UPDATE_COMMAND};

const FACTS: &str = "tools/docs/support_matrix/facts.json";

fn write(workspace: &Path, text: &str) -> Result<(), String> {
    let target = workspace.join(MATRIX_PATH);
    let staging = workspace.join(format!("{MATRIX_PATH}.tmp"));
    std::fs::write(&staging, text)
        .map_err(|error| format!("write {}: {error}", staging.display()))?;
    std::fs::rename(&staging, &target)
        .map_err(|error| format!("rename {}: {error}", target.display()))
}

fn unlisted_dirs(workspace: &Path, facts_json: &str) -> Result<Vec<String>, String> {
    let facts = parse_facts(facts_json)?;
    let entries = std::fs::read_dir(workspace)
        .map_err(|error| format!("list {}: {error}", workspace.display()))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().is_dir() || name.starts_with('.') || name.starts_with("bazel-") {
            continue;
        }
        let listed =
            facts.rows.iter().any(|row| row.name == name) || facts.not_languages.contains(&name);
        if !listed {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

fn run() -> Result<ExitCode, String> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let source = RunfilesSource::create()?;
    let facts = source.read(FACTS)?;
    let matrix = generate(&facts, &source, dx_adopt::new::SUPPORTED_NEW_LANGUAGES)?;
    match mode.as_str() {
        "--write" => {
            let Ok(workspace) = std::env::var("BUILD_WORKSPACE_DIRECTORY") else {
                return Err(format!("--write needs `{UPDATE_COMMAND}`"));
            };
            let workspace = Path::new(&workspace);
            let unlisted = unlisted_dirs(workspace, &facts)?;
            if !unlisted.is_empty() {
                return Err(format!(
                    "top-level dirs {unlisted:?} are neither rows nor NOT_LANGUAGES in tools/docs/support_matrix/languages.bzl"
                ));
            }
            write(workspace, &matrix.markdown)?;
            Ok(ExitCode::SUCCESS)
        }
        "--gaps" => {
            let ledger = parse_facts(&facts)?.gap_issues;
            for gap in &matrix.gaps {
                match gaps::issue_for(gap, &ledger) {
                    Some(issue) => println!("{gap} #{issue}"),
                    None => println!("{gap}"),
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!(
            "usage: support_matrix --write | --gaps (got {other:?})"
        )),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("support_matrix: {error}");
            ExitCode::FAILURE
        }
    }
}
