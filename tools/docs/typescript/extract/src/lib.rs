//! Converts TypeDoc JSON into one validated DocIr shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod convert;
mod types;

pub use convert::{Settings, convert};

use std::fs;

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub typedoc: String,
    pub out: String,
    pub package: String,
    pub source_prefix: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: typescript_extract --typedoc FILE --out FILE --package NAME [--source-prefix DIR] [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--typedoc",
            "--out",
            "--package",
            "--source-prefix",
            "--config",
        ],
    )?;
    Ok(Options {
        typedoc: flags.one("--typedoc")?,
        out: flags.one("--out")?,
        package: flags.one("--package")?,
        source_prefix: flags.optional("--source-prefix")?.unwrap_or_default(),
        config: flags.optional("--config")?.unwrap_or_default(),
    })
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.typedoc)
        .map_err(|error| format!("cannot read {}: {error}", options.typedoc))?;
    let shard = convert(
        &json,
        &Settings {
            package: options.package.clone(),
            source_prefix: options.source_prefix.clone(),
            config: options.config.clone(),
        },
    )?;
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    documentation_ir::tool::write_shard(&options.out, &shard)
}

#[cfg(test)]
mod lib_tests;
