#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod convert;
mod exports;
mod types;

pub use convert::{Settings, clean, convert, signature};
pub use exports::wildcard_match;
pub use types::SCHEMA;

use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub struct Duplicate {
    pub rel: String,
    pub line: u64,
    pub name: String,
    pub reason: &'static str,
    pub first: String,
    pub first_rel: String,
    pub first_line: u64,
}

#[derive(Debug)]
pub enum PowershellExtractError {
    Flag(FlagError),
    Io(PathIoError),
    MalformedApi(serde_json::Error),
    UnsupportedSchema { schema: String },
    NoSchema,
    ApiShape(serde_json::Error),
    BinaryRootModule { rel: String, root: String },
    BadFunctionsToExport { rel: String, value: String },
    DynamicExport { rel: String },
    UndefinedExport { rel: String, pattern: String },
    NoExports,
    DuplicateFunction(Box<Duplicate>),
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for PowershellExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::MalformedApi(error) => write!(f, "API JSON is malformed: {error}"),
            Self::UnsupportedSchema { schema } => write!(
                f,
                "API JSON schema '{schema}' is not supported, expected '{SCHEMA}'"
            ),
            Self::NoSchema => f.write_str("API JSON has no schema"),
            Self::ApiShape(error) => write!(f, "API JSON does not match {SCHEMA}: {error}"),
            Self::BinaryRootModule { rel, root } => write!(
                f,
                "{rel}: RootModule '{root}' is a binary module, which is not supported"
            ),
            Self::BadFunctionsToExport { rel, value } => write!(
                f,
                "{rel}: FunctionsToExport '{value}' must be '*' or a list"
            ),
            Self::DynamicExport { rel } => write!(
                f,
                "{rel}: Export-ModuleMember -Function is not a literal list"
            ),
            Self::UndefinedExport { rel, pattern } => write!(
                f,
                "{rel}: exports '{pattern}' but no source file defines it"
            ),
            Self::NoExports => f.write_str("the module exports no functions"),
            Self::DuplicateFunction(duplicate) => {
                let Duplicate {
                    rel,
                    line,
                    name,
                    reason,
                    first,
                    first_rel,
                    first_line,
                } = duplicate.as_ref();
                write!(
                    f,
                    "{rel}:{line}: function '{name}' {reason} '{first}' at {first_rel}:{first_line}"
                )
            }
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PowershellExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Flag(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::MalformedApi(error) | Self::ApiShape(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::UnsupportedSchema { .. }
            | Self::NoSchema
            | Self::BinaryRootModule { .. }
            | Self::BadFunctionsToExport { .. }
            | Self::DynamicExport { .. }
            | Self::UndefinedExport { .. }
            | Self::NoExports
            | Self::DuplicateFunction(_)
            | Self::InvalidIr(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub api: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("powershell_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: powershell_extract --api FILE --out FILE --package NAME [--config TEXT]"
}

pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags =
        documentation_ir::tool::flags(args, &["--api", "--out", "--package", "--config"])?;
    Ok(Options {
        api: flags.one("--api")?,
        out: flags.one("--out")?,
        package: flags.one("--package")?,
        config: flags.optional("--config")?.unwrap_or_default(),
    })
}

pub fn run(options: &Options) -> Result<(), PowershellExtractError> {
    let json =
        path_io::read_to_string(Path::new(&options.api)).map_err(PowershellExtractError::Io)?;
    let shard = convert(
        &json,
        &Settings {
            package: options.package.clone(),
            config: options.config.clone(),
        },
    )?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(PowershellExtractError::Write)
}

#[cfg(test)]
mod lib_tests;
