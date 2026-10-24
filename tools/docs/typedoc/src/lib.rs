//! Extracts TypeDoc JSON into documentation IR and renders IR as Markdown.

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
mod render;
mod types;

pub use convert::{convert, Settings};
pub use render::{render, Options};

use std::fs;

/// The usage text shown for a rejected invocation.
pub fn usage() -> String {
    [
        "usage: docs_typedoc ir --typedoc FILE --out FILE --package NAME [--source-prefix DIR] [--config TEXT]",
        "       docs_typedoc render --shard FILE... --out FILE --title TEXT [--source-url TEMPLATE]",
    ]
    .join("\n")
}

/// One parsed invocation.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Ir {
        typedoc: String,
        out: String,
        settings: Settings,
    },
    Render {
        shards: Vec<String>,
        out: String,
        options: Options,
    },
}

/// Parses argv into one command, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err("a subcommand is required".to_owned());
    };
    let mut values: Vec<(String, String)> = Vec::new();
    let mut index = 0;
    while index < rest.len() {
        let flag = &rest[index];
        let Some(value) = rest.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        values.push((flag.clone(), value.clone()));
        index += 2;
    }
    let one = |name: &str| -> Result<String, String> {
        let found: Vec<&String> = values
            .iter()
            .filter(|(flag, _)| flag == name)
            .map(|(_, value)| value)
            .collect();
        match found.as_slice() {
            [value] if !value.is_empty() => Ok((*value).clone()),
            [] => Err(format!("{name} is required")),
            [_] => Err(format!("{name} must not be empty")),
            _ => Err(format!("{name} given more than once")),
        }
    };
    let optional = |name: &str| -> String {
        values
            .iter()
            .find(|(flag, _)| flag == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let allowed: &[&str] = match verb.as_str() {
        "ir" => &[
            "--typedoc",
            "--out",
            "--package",
            "--source-prefix",
            "--config",
        ],
        "render" => &["--shard", "--out", "--title", "--source-url"],
        other => return Err(format!("unknown subcommand '{other}'")),
    };
    for (flag, _) in &values {
        if !allowed.contains(&flag.as_str()) {
            return Err(format!("unknown argument '{flag}'"));
        }
    }
    if verb == "ir" {
        return Ok(Command::Ir {
            typedoc: one("--typedoc")?,
            out: one("--out")?,
            settings: Settings {
                package: one("--package")?,
                source_prefix: optional("--source-prefix"),
                config: optional("--config"),
            },
        });
    }
    let shards: Vec<String> = values
        .iter()
        .filter(|(flag, _)| flag == "--shard")
        .map(|(_, value)| value.clone())
        .collect();
    if shards.is_empty() {
        return Err("--shard is required".to_owned());
    }
    Ok(Command::Render {
        shards,
        out: one("--out")?,
        options: Options {
            title: one("--title")?,
            source_url: optional("--source-url"),
        },
    })
}

/// Runs one command, returning the failure text on error.
pub fn run(command: &Command) -> Result<(), String> {
    match command {
        Command::Ir {
            typedoc,
            out,
            settings,
        } => {
            let json = fs::read_to_string(typedoc)
                .map_err(|error| format!("cannot read {typedoc}: {error}"))?;
            let shard = convert(&json, settings)?;
            let bytes = documentation_ir::encode_shard(&shard)
                .map_err(|error| format!("cannot encode IR: {error:?}"))?;
            fs::write(out, bytes).map_err(|error| format!("cannot write {out}: {error}"))
        }
        Command::Render {
            shards,
            out,
            options,
        } => {
            let mut decoded = Vec::new();
            for path in shards {
                let bytes =
                    fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
                decoded.push(
                    documentation_ir::decode_shard(&bytes)
                        .map_err(|error| format!("{path} is not valid IR: {error:?}"))?,
                );
            }
            let page = render(&decoded, options)?;
            fs::write(out, page).map_err(|error| format!("cannot write {out}: {error}"))
        }
    }
}

#[cfg(test)]
mod lib_tests;
