#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod convert;
mod types;

pub use convert::{Settings, convert};
pub use documentation_ir::tool::{Source, parse_sources};
pub use types::SCHEMA;

use documentation_ir::tool::{FlagError, PathIoError, SourcesError, WriteShardError, path_io};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum DotnetExtractError {
    Flag(FlagError),
    UnknownLanguage {
        language: String,
    },
    Io(PathIoError),
    Sources(SourcesError),
    MalformedApi(serde_json::Error),
    UnsupportedSchema {
        schema: String,
    },
    NoSchema,
    ApiShape(serde_json::Error),
    LanguageMismatch {
        found: String,
        expected: String,
    },
    NoItems,
    DuplicateDocId {
        doc_id: String,
    },
    UnknownParent {
        doc_id: String,
        parent: String,
    },
    ParentCycle {
        doc_id: String,
    },
    IdCollision {
        other: String,
        doc_id: String,
        id: String,
    },
    EmptyName {
        doc_id: String,
    },
    UnknownKind {
        kind: String,
    },
    UndeclaredSource {
        doc_id: String,
        file: String,
    },
    NoId {
        doc_id: String,
    },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for DotnetExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::UnknownLanguage { language } => {
                write!(f, "--language must be csharp or fsharp, got '{language}'")
            }
            Self::Io(error) => error.fmt(f),
            Self::Sources(error) => error.fmt(f),
            Self::MalformedApi(error) => write!(f, "API JSON is malformed: {error}"),
            Self::UnsupportedSchema { schema } => write!(
                f,
                "API JSON schema '{schema}' is not supported, expected '{SCHEMA}'"
            ),
            Self::NoSchema => f.write_str("API JSON has no schema"),
            Self::ApiShape(error) => write!(f, "API JSON does not match {SCHEMA}: {error}"),
            Self::LanguageMismatch { found, expected } => {
                write!(f, "API JSON language '{found}' does not match '{expected}'")
            }
            Self::NoItems => f.write_str("API JSON has no public items"),
            Self::DuplicateDocId { doc_id } => write!(f, "duplicate doc id '{doc_id}'"),
            Self::UnknownParent { doc_id, parent } => {
                write!(f, "'{doc_id}' has unknown parent '{parent}'")
            }
            Self::ParentCycle { doc_id } => write!(f, "parent cycle at '{doc_id}'"),
            Self::IdCollision { other, doc_id, id } => write!(
                f,
                "id collision after case folding between '{other}' and '{doc_id}': {id}"
            ),
            Self::EmptyName { doc_id } => write!(f, "'{doc_id}' has an empty name"),
            Self::UnknownKind { kind } => write!(f, "unknown item kind '{kind}'"),
            Self::UndeclaredSource { doc_id, file } => write!(
                f,
                "'{doc_id}' is declared in '{file}', which is not a declared source"
            ),
            Self::NoId { doc_id } => write!(f, "no id for '{doc_id}'"),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DotnetExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Flag(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Sources(error) => Some(error),
            Self::MalformedApi(error) | Self::ApiShape(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::UnknownLanguage { .. }
            | Self::UnsupportedSchema { .. }
            | Self::NoSchema
            | Self::LanguageMismatch { .. }
            | Self::NoItems
            | Self::DuplicateDocId { .. }
            | Self::UnknownParent { .. }
            | Self::ParentCycle { .. }
            | Self::IdCollision { .. }
            | Self::EmptyName { .. }
            | Self::UnknownKind { .. }
            | Self::UndeclaredSource { .. }
            | Self::NoId { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<FlagError> for DotnetExtractError {
    fn from(error: FlagError) -> Self {
        Self::Flag(error)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub api: String,
    pub language: String,
    pub package: String,
    pub srcs_file: String,
    pub config: String,
    pub out: String,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("dotnet_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: dotnet_extract --api FILE --language csharp|fsharp --package NAME --srcs-file FILE --out FILE [--config TEXT]"
}

pub fn parse(args: &[String]) -> Result<Options, DotnetExtractError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--api",
            "--language",
            "--package",
            "--srcs-file",
            "--config",
            "--out",
        ],
    )?;
    let options = Options {
        api: flags.one("--api")?,
        language: flags.one("--language")?,
        package: flags.one("--package")?,
        srcs_file: flags.one("--srcs-file")?,
        config: flags.optional("--config")?.unwrap_or_default(),
        out: flags.one("--out")?,
    };
    if options.language != "csharp" && options.language != "fsharp" {
        return Err(DotnetExtractError::UnknownLanguage {
            language: options.language,
        });
    }
    Ok(options)
}

pub fn run(options: &Options) -> Result<(), DotnetExtractError> {
    let read =
        |path: &str| path_io::read_to_string(Path::new(path)).map_err(DotnetExtractError::Io);
    let json = read(&options.api)?;
    let srcs = read(&options.srcs_file)?;
    let shard = convert(
        &json,
        &Settings {
            language: options.language.clone(),
            package: options.package.clone(),
            config: options.config.clone(),
            sources: parse_sources(&srcs).map_err(DotnetExtractError::Sources)?,
        },
    )?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(DotnetExtractError::Write)
}

#[cfg(test)]
mod lib_tests;
