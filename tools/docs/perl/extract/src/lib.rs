//! Converts the Perl POD driver's JSON into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::fs;

use documentation_ir::proto::{
    DocIr, Extension, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const LANGUAGE: &str = "perl";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "perl.config";
const KEY_INCOMPLETE: &str = "perl.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: perl_extract --json FILE --package NAME --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--json" => &mut options.json,
            "--package" => &mut options.package,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if !slot.is_empty() {
            return Err(format!("{flag} given more than once"));
        }
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
        *slot = value.clone();
        index += 2;
    }
    for (flag, value) in [
        ("--json", &options.json),
        ("--package", &options.package),
        ("--out", &options.out),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.json)
        .map_err(|error| format!("cannot read {}: {error}", options.json))?;
    let shard = convert(&json, &options.package)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

pub use documentation_ir::escape_segment;

fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at} '{key}' is not a string"))
}

fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))?
        .as_u64()
        .ok_or_else(|| format!("{at} '{key}' is not a number"))
}

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))?
        .as_bool()
        .ok_or_else(|| format!("{at} '{key}' is not a boolean"))
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

/// Escapes the `::` separated segments of a Perl name into one id part.
fn qualified(path: &str) -> String {
    path.split("::")
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

/// Normalizes line endings and trims trailing whitespace.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("POD JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("POD JSON is not an object")?;
    let at = "POD JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "POD JSON format {other} is not supported, expected {FORMAT}"
            ));
        }
    }
    let config = format!("perl={};format={FORMAT}", string(root, "perl", at)?);
    let objects = root
        .get("objects")
        .and_then(Value::as_array)
        .ok_or("POD JSON 'objects' is not a list")?;
    let prefix = format!("{LANGUAGE}:{package}:");
    let mut symbols = Vec::new();
    for (index, value) in objects.iter().enumerate() {
        let at = format!("POD JSON object {index}");
        let object = value
            .as_object()
            .ok_or_else(|| format!("{at} is not an object"))?;
        let path = string(object, "path", &at)?;
        let namespace = string(object, "namespace", &at)?;
        if path.is_empty() {
            return Err(format!("{at} has an empty path"));
        }
        let kind = match string(object, "type", &at)?.as_str() {
            "package" => SymbolKind::Module,
            "sub" => SymbolKind::Function,
            other => return Err(format!("{at} has unknown type '{other}'")),
        };
        match string(object, "visibility", &at)?.as_str() {
            "public" => {}
            "private" => continue,
            other => return Err(format!("{at} has unknown visibility '{other}'")),
        }
        let doc = normalize(&string(object, "docstring", &at)?);
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if doc.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, "doc"));
        }
        let relations = (!namespace.is_empty()).then(|| Relations {
            member_of: vec![format!("{prefix}{}", qualified(&namespace))],
            implements: Vec::new(),
            reexported_as: Vec::new(),
        });
        symbols.push(Symbol {
            id: format!("{prefix}{}", qualified(&path)),
            kind: kind as i32,
            signature_text: string(object, "signature", &at)?,
            doc_markdown: doc,
            source: Some(SourceRef {
                file: string(object, "file", &at)?,
                line: number(object, "line", &at)?,
                generated: boolean(object, "generated", &at)?,
            }),
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: path,
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "POD JSON lists no public Perl API of package '{package}'"
        ));
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
