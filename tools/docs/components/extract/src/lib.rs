//! Converts component metadata JSON into one validated DocIr shard.

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

pub use convert::{convert, Settings, SCHEMA};

use std::fs;

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

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: component_extract --meta FILE --out FILE --language NAME --package NAME [--source-prefix DIR] [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut values: Vec<(&str, &str)> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if ![
            "--meta",
            "--out",
            "--language",
            "--package",
            "--source-prefix",
            "--config",
        ]
        .contains(&flag)
        {
            return Err(format!("unknown argument '{flag}'"));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        values.push((flag, value.as_str()));
        index += 2;
    }
    let count = |name: &str| values.iter().filter(|(flag, _)| *flag == name).count();
    let one = |name: &str| -> Result<String, String> {
        match values.iter().find(|(flag, _)| *flag == name) {
            _ if count(name) > 1 => Err(format!("{name} given more than once")),
            Some((_, value)) if !value.is_empty() => Ok((*value).to_owned()),
            Some(_) => Err(format!("{name} must not be empty")),
            None => Err(format!("{name} is required")),
        }
    };
    let optional = |name: &str| -> Result<String, String> {
        if count(name) > 1 {
            return Err(format!("{name} given more than once"));
        }
        Ok(values
            .iter()
            .find(|(flag, _)| *flag == name)
            .map(|(_, value)| (*value).to_owned())
            .unwrap_or_default())
    };
    Ok(Options {
        meta: one("--meta")?,
        out: one("--out")?,
        language: one("--language")?,
        package: one("--package")?,
        source_prefix: optional("--source-prefix")?,
        config: optional("--config")?,
    })
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.meta)
        .map_err(|error| format!("cannot read {}: {error}", options.meta))?;
    let shard = convert(
        &json,
        &Settings {
            language: options.language.clone(),
            package: options.package.clone(),
            source_prefix: options.source_prefix.clone(),
            config: options.config.clone(),
        },
    )?;
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

#[cfg(test)]
mod lib_tests;
