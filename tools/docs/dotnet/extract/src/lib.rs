//! Converts the C# and F# API facts of the dotnet drivers into one validated DocIr shard.

mod convert;
mod types;

pub use convert::{Settings, Source, convert, parse_sources};
pub use types::SCHEMA;

use std::fs;

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub api: String,
    pub language: String,
    pub package: String,
    pub srcs_file: String,
    pub config: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: dotnet_extract --api FILE --language csharp|fsharp --package NAME --srcs-file FILE --out FILE [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
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
        return Err(format!(
            "--language must be csharp or fsharp, got '{}'",
            options.language
        ));
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard. Nothing is written on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.api)
        .map_err(|error| format!("cannot read {}: {error}", options.api))?;
    let srcs = fs::read_to_string(&options.srcs_file)
        .map_err(|error| format!("cannot read {}: {error}", options.srcs_file))?;
    let shard = convert(
        &json,
        &Settings {
            language: options.language.clone(),
            package: options.package.clone(),
            config: options.config.clone(),
            sources: parse_sources(&srcs)?,
        },
    )?;
    documentation_ir::tool::write_shard(&options.out, &shard)
}

#[cfg(test)]
mod lib_tests;
