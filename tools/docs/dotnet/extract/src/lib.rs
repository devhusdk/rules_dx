//! Converts the C# and F# API facts of the dotnet drivers into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod convert;
mod types;

pub use convert::{convert, parse_sources, Settings, Source};
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
    let mut options = Options::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--api" => &mut options.api,
            "--language" => &mut options.language,
            "--package" => &mut options.package,
            "--srcs-file" => &mut options.srcs_file,
            "--config" => &mut options.config,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if seen.contains(&flag) {
            return Err(format!("{flag} given more than once"));
        }
        seen.push(flag);
        *slot = value.clone();
        index += 2;
    }
    for (flag, value) in [
        ("--api", &options.api),
        ("--language", &options.language),
        ("--package", &options.package),
        ("--srcs-file", &options.srcs_file),
        ("--out", &options.out),
    ] {
        if !seen.contains(&flag) {
            return Err(format!("{flag} is required"));
        }
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
    }
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
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

#[cfg(test)]
mod lib_tests;
