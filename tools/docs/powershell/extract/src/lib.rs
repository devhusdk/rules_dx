//! Converts the PowerShell API facts of extract.ps1 into one validated DocIr shard.

mod convert;
mod exports;
mod types;

pub use convert::{Settings, clean, convert, signature};
pub use exports::wildcard_match;
pub use types::SCHEMA;

use std::fs;

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub api: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: powershell_extract --api FILE --out FILE --package NAME [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut flags =
        documentation_ir::tool::flags(args, &["--api", "--out", "--package", "--config"])?;
    Ok(Options {
        api: flags.one("--api")?,
        out: flags.one("--out")?,
        package: flags.one("--package")?,
        config: flags.optional("--config")?.unwrap_or_default(),
    })
}

/// Reads, converts, validates and writes one shard. Nothing is written on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.api)
        .map_err(|error| format!("cannot read {}: {error}", options.api))?;
    let shard = convert(
        &json,
        &Settings {
            package: options.package.clone(),
            config: options.config.clone(),
        },
    )?;
    documentation_ir::tool::write_shard(&options.out, &shard)
}

#[cfg(test)]
mod lib_tests;
