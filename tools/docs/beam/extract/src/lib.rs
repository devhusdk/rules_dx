//! Converts the Elixir and Erlang doc drivers' JSON into one validated DocIr shard.

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
    DocIr, Extension, Param, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "beam.config";
const KEY_INCOMPLETE: &str = "beam.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: beam_extract --json FILE --package NAME --out FILE"
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

fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, String> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| format!("{at} '{key}' is not a list"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at} '{key}' holds a non-string"))
        })
        .collect()
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

fn kind_of(kind: &str, at: &str) -> Result<SymbolKind, String> {
    Ok(match kind {
        "module" => SymbolKind::Module,
        "function" => SymbolKind::Function,
        "macro" => SymbolKind::Macro,
        "type" => SymbolKind::TypeAlias,
        "callback" => SymbolKind::Method,
        other => return Err(format!("{at} has unknown kind '{other}'")),
    })
}

fn module_id(prefix: &str, module: &str) -> String {
    let segments: Vec<String> = module
        .split('.')
        .map(documentation_ir::escape_id_segment)
        .collect();
    format!("{prefix}{}", segments.join("."))
}

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("driver JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("driver JSON is not an object")?;
    let at = "driver JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "driver JSON format {other} is not supported, expected {FORMAT}"
            ));
        }
    }
    let language = string(root, "language", at)?;
    if language != "elixir" && language != "erlang" {
        return Err(format!("driver JSON has unknown language '{language}'"));
    }
    let config = format!(
        "language={language};tool={};version={};format={FORMAT}",
        string(root, "tool", at)?,
        string(root, "version", at)?
    );
    let prefix = format!("{language}:{package}:");
    let items = field(root, "symbols", at)?
        .as_array()
        .ok_or("driver JSON 'symbols' is not a list")?;
    let mut symbols = Vec::new();
    for (index, value) in items.iter().enumerate() {
        let at = format!("driver JSON symbol {index}");
        let item = value
            .as_object()
            .ok_or_else(|| format!("{at} is not an object"))?;
        let kind_name = string(item, "kind", &at)?;
        let kind = kind_of(&kind_name, &at)?;
        let module = string(item, "module", &at)?;
        let name = string(item, "name", &at)?;
        if module.is_empty() || (kind != SymbolKind::Module && name.is_empty()) {
            return Err(format!("{at} has an empty module or name"));
        }
        let arity = number(item, "arity", &at)?;
        let owner = module_id(&prefix, &module);
        let (id, display, member_of) = if kind == SymbolKind::Module {
            (owner.clone(), module.clone(), Vec::new())
        } else {
            (
                format!(
                    "{owner}.{}@{kind_name}-{arity}",
                    documentation_ir::escape_id_segment(&name)
                ),
                format!("{module}.{name}/{arity}"),
                vec![owner.clone()],
            )
        };
        let doc = normalize(&string(item, "doc", &at)?);
        let mut incomplete = Vec::new();
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        let params = strings(item, "params", &at)?
            .into_iter()
            .map(|name| Param {
                name,
                ..Param::default()
            })
            .collect();
        let file = string(item, "file", &at)?;
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if !incomplete.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
        }
        symbols.push(Symbol {
            id,
            kind: kind as i32,
            signature_text: string(item, "signature", &at)?,
            doc_markdown: doc,
            params,
            source: Some(SourceRef {
                file,
                line: number(item, "line", &at)?,
                generated: item
                    .get("generated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            visibility: Visibility::Public as i32,
            relations: if member_of.is_empty() {
                None
            } else {
                Some(Relations {
                    member_of,
                    ..Relations::default()
                })
            },
            extensions,
            display_name: display,
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "driver JSON lists no public {language} API of package '{package}'"
        ));
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language,
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
