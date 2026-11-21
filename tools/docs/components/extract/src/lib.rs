//! Converts component metadata JSON into one validated `DocIr` shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod convert;

pub use convert::{SCHEMA, Settings, convert};

use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use std::fmt;
use std::path::Path;

/// Why one component metadata document could not become a shard.
#[derive(Debug)]
pub enum ComponentExtractError {
    Read(PathIoError),
    Malformed(serde_json::Error),
    UnsupportedSchema {
        found: String,
    },
    WrongFramework {
        found: String,
        expected: String,
    },
    ComponentsNotArray,
    NoComponents,
    BadName {
        what: &'static str,
        name: String,
    },
    DottedName {
        name: String,
    },
    DuplicateComponent {
        name: String,
    },
    DuplicateMember {
        what: &'static str,
        member: String,
        component: String,
    },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for ComponentExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::Malformed(error) => write!(f, "invalid JSON: {error}"),
            Self::UnsupportedSchema { found } => {
                write!(f, "unsupported schema '{found}', expected '{SCHEMA}'")
            }
            Self::WrongFramework { found, expected } => {
                write!(f, "metadata is for framework '{found}', not '{expected}'")
            }
            Self::ComponentsNotArray => f.write_str("components must be an array"),
            Self::NoComponents => f.write_str("metadata has no components"),
            Self::BadName { what, name } => {
                write!(f, "{what} name '{name}' is empty or has whitespace")
            }
            Self::DottedName { name } => {
                write!(f, "component name '{name}' must not contain '.'")
            }
            Self::DuplicateComponent { name } => write!(f, "duplicate component '{name}'"),
            Self::DuplicateMember {
                what,
                member,
                component,
            } => write!(f, "duplicate {what} '{member}' in component '{component}'"),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ComponentExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::UnsupportedSchema { .. }
            | Self::WrongFramework { .. }
            | Self::ComponentsNotArray
            | Self::NoComponents
            | Self::BadName { .. }
            | Self::DottedName { .. }
            | Self::DuplicateComponent { .. }
            | Self::DuplicateMember { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub meta: String,
    pub out: String,
    pub language: String,
    pub package: String,
    pub source_prefix: String,
    pub config: String,
}

/// Runs `component_extract` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("component_extract", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: component_extract --meta FILE --out FILE --language NAME --package NAME [--source-prefix DIR] [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--meta",
            "--out",
            "--language",
            "--package",
            "--source-prefix",
            "--config",
        ],
    )?;
    Ok(Options {
        meta: flags.one("--meta")?,
        out: flags.one("--out")?,
        language: flags.one("--language")?,
        package: flags.one("--package")?,
        source_prefix: flags.optional("--source-prefix")?.unwrap_or_default(),
        config: flags.optional("--config")?.unwrap_or_default(),
    })
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn run(options: &Options) -> Result<(), ComponentExtractError> {
    let json =
        path_io::read_to_string(Path::new(&options.meta)).map_err(ComponentExtractError::Read)?;
    let shard = convert(
        &json,
        &Settings {
            language: options.language.clone(),
            package: options.package.clone(),
            source_prefix: options.source_prefix.clone(),
            config: options.config.clone(),
        },
    )?;
    documentation_ir::validate_shard(&shard).map_err(ComponentExtractError::InvalidIr)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(ComponentExtractError::Write)
}

#[cfg(test)]
mod lib_tests;
