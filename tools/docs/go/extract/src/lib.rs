//! Converts the Go doc driver's JSON into one validated DocIr shard.

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
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};
use serde_json::{Map, Value};

const LANGUAGE: &str = "go";
const SCHEMA: u64 = 1;
const KEY_CONFIG: &str = "go.config";
const KEY_EXCLUDED: &str = "go.excluded";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: go_extract --json FILE --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--json" => &mut options.json,
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
    for (flag, value) in [("--json", &options.json), ("--out", &options.out)] {
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
    let shard = convert(&json)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

fn field<'a>(object: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))
}

fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    field(object, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at} '{key}' is not a string"))
}

fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, String> {
    field(object, key, at)?
        .as_u64()
        .ok_or_else(|| format!("{at} '{key}' is not a number"))
}

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| format!("{at} '{key}' is not a boolean"))
}

fn source(object: &Map<String, Value>, at: &str) -> Result<SourceRef, String> {
    Ok(SourceRef {
        file: string(object, "file", at)?,
        line: number(object, "line", at)?,
        generated: boolean(object, "generated", at)?,
    })
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

fn kind_of(kind: &str, type_kind: &str, at: &str) -> Result<SymbolKind, String> {
    Ok(match (kind, type_kind) {
        ("func", _) => SymbolKind::Function,
        ("method", _) => SymbolKind::Method,
        ("field", _) => SymbolKind::Field,
        ("const", _) => SymbolKind::Constant,
        ("var", _) => SymbolKind::Variable,
        ("type", "interface") => SymbolKind::Interface,
        ("type", "alias") => SymbolKind::TypeAlias,
        ("type", "struct" | "other") => SymbolKind::Class,
        ("type", other) => return Err(format!("{at} has unknown type_kind '{other}'")),
        (other, _) => return Err(format!("{at} has unknown kind '{other}'")),
    })
}

/// Converts one driver JSON document into a validated IR shard.
pub fn convert(json: &str) -> Result<DocIr, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("Go doc JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("Go doc JSON is not an object")?;
    let at = "Go doc JSON";
    match number(root, "schema", at)? {
        SCHEMA => {}
        other => {
            return Err(format!(
                "Go doc JSON schema {other} is not supported, expected {SCHEMA}"
            ))
        }
    }
    let importpath = string(root, "importpath", at)?;
    if importpath.is_empty() || importpath.contains(':') {
        return Err(format!(
            "Go doc JSON has an invalid importpath '{importpath}'"
        ));
    }
    let package = string(root, "package", at)?;
    let config = string(root, "config", at)?;
    let excluded: Vec<String> = field(root, "excluded", at)?
        .as_array()
        .ok_or("Go doc JSON 'excluded' is not a list")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or("Go doc JSON 'excluded' holds a non-string")
        })
        .collect::<Result<_, _>>()?;
    let prefix = format!("{LANGUAGE}:{importpath}:");
    let mut symbols = vec![Symbol {
        id: format!("{prefix}{package}"),
        kind: SymbolKind::Module as i32,
        signature_text: format!("package {package} // import \"{importpath}\""),
        doc_markdown: string(root, "doc", at)?,
        source: Some(source(root, at)?),
        visibility: Visibility::Public as i32,
        extensions: vec![
            extension(KEY_CONFIG, &config),
            extension(KEY_EXCLUDED, &excluded.join("\n")),
        ],
        ..Symbol::default()
    }];
    let items = field(root, "items", at)?
        .as_array()
        .ok_or("Go doc JSON 'items' is not a list")?;
    if items.is_empty() {
        return Err("Go doc JSON lists no exported symbols".to_owned());
    }
    for (index, item) in items.iter().enumerate() {
        let at = format!("Go doc JSON item {index}");
        let item = item
            .as_object()
            .ok_or_else(|| format!("{at} is not an object"))?;
        let name = string(item, "name", &at)?;
        let recv = string(item, "recv", &at)?;
        let kind = kind_of(
            &string(item, "kind", &at)?,
            &string(item, "type_kind", &at)?,
            &at,
        )?;
        if name.is_empty() {
            return Err(format!("{at} has an empty name"));
        }
        let member = matches!(kind, SymbolKind::Method | SymbolKind::Field);
        if member == recv.is_empty() {
            return Err(format!(
                "{at} '{name}' has a receiver that does not match its kind"
            ));
        }
        let (qualified, relations) = if member {
            (
                format!("{recv}.{name}"),
                Some(Relations {
                    member_of: vec![format!("{prefix}{recv}")],
                    ..Relations::default()
                }),
            )
        } else {
            (name, None)
        };
        symbols.push(Symbol {
            id: format!("{prefix}{qualified}"),
            kind: kind as i32,
            signature_text: string(item, "signature", &at)?,
            doc_markdown: string(item, "doc", &at)?,
            source: Some(source(item, &at)?),
            visibility: Visibility::Public as i32,
            relations,
            extensions: vec![extension(KEY_CONFIG, &config)],
            ..Symbol::default()
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: LANGUAGE.to_owned(),
        package: importpath,
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
