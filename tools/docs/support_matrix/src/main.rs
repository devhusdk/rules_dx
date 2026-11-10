//! Writes `docs/support-matrix.md`.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::Path;
use std::process::ExitCode;

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{
    MATRIX_PATH, Source, UNSUPPORTED_PATH, UPDATE_COMMAND, generate, parse_facts, unsupported_json,
};

const FACTS: &str = "tools/docs/support_matrix/facts.json";

fn write(workspace: &Path, path: &str, text: &str) -> Result<(), String> {
    let target = workspace.join(path);
    let staging = workspace.join(format!("{path}.tmp"));
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
    let matrix = generate(&facts, &source)?;
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
            write(workspace, MATRIX_PATH, &matrix)?;
            write(
                workspace,
                UNSUPPORTED_PATH,
                &unsupported_json(&parse_facts(&facts)?),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("usage: support_matrix --write (got {other:?})")),
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
