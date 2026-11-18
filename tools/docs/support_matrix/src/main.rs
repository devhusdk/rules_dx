//! Writes `docs/support-matrix.md`.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde as _;
use serde_json as _;
use std::fmt;
use std::path::Path;
use std::process::ExitCode;
use yaml_serde as _;

use dx_atomic_fs::path_io::{self, PathIoError};

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{
    Facts, MATRIX_PATH, Source, SupportMatrixError, UNSUPPORTED_PATH, UPDATE_COMMAND, generate,
    parse_facts, unsupported_json,
};

const FACTS: &str = "tools/docs/support_matrix/facts.json";

#[derive(Debug)]
enum UpdateError {
    Matrix(SupportMatrixError),
    Io(PathIoError),
    NoWorkspace,
    Unlisted(Vec<String>),
    Usage(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Matrix(source) => source.fmt(f),
            Self::Io(source) => source.fmt(f),
            Self::NoWorkspace => write!(f, "--write needs `{UPDATE_COMMAND}`"),
            Self::Unlisted(dirs) => write!(
                f,
                "top-level dirs {dirs:?} are neither rows nor NOT_LANGUAGES in tools/docs/support_matrix/languages.bzl"
            ),
            Self::Usage(got) => write!(f, "usage: support_matrix --write (got {got:?})"),
        }
    }
}

impl std::error::Error for UpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Matrix(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::NoWorkspace | Self::Unlisted(_) | Self::Usage(_) => None,
        }
    }
}

impl From<SupportMatrixError> for UpdateError {
    fn from(source: SupportMatrixError) -> Self {
        Self::Matrix(source)
    }
}

impl From<PathIoError> for UpdateError {
    fn from(source: PathIoError) -> Self {
        Self::Io(source)
    }
}

fn write(workspace: &Path, path: &str, text: &str) -> Result<(), PathIoError> {
    let target = workspace.join(path);
    dx_atomic_fs::write_atomic(&target, text.as_bytes()).map_err(|source| PathIoError::Write {
        path: target,
        source,
    })
}

fn unlisted_dirs(workspace: &Path, facts: &Facts) -> Result<Vec<String>, PathIoError> {
    let mut out = Vec::new();
    for path in path_io::read_dir(workspace)? {
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if !path.is_dir() || name.starts_with('.') || name.starts_with("bazel-") {
            continue;
        }
        let listed =
            facts.rows.iter().any(|row| row.name == name) || facts.not_languages.contains(&name);
        if !listed {
            out.push(name);
        }
    }
    Ok(out)
}

fn run() -> Result<ExitCode, UpdateError> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let source = RunfilesSource::create()?;
    let facts = source.read(FACTS)?;
    let matrix = generate(&facts, &source)?;
    match mode.as_str() {
        "--write" => {
            let Ok(workspace) = std::env::var("BUILD_WORKSPACE_DIRECTORY") else {
                return Err(UpdateError::NoWorkspace);
            };
            let workspace = Path::new(&workspace);
            let facts = parse_facts(&facts)?;
            let unlisted = unlisted_dirs(workspace, &facts)?;
            if !unlisted.is_empty() {
                return Err(UpdateError::Unlisted(unlisted));
            }
            write(workspace, MATRIX_PATH, &matrix)?;
            write(workspace, UNSUPPORTED_PATH, &unsupported_json(&facts))?;
            Ok(ExitCode::SUCCESS)
        }
        other => Err(UpdateError::Usage(other.to_owned())),
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
