//! Converts the LDoc filter's JSON into one validated DocIr shard.

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
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const LANGUAGE: &str = "lua";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "lua.config";
const KEY_INCOMPLETE: &str = "lua.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: lua_extract --json FILE --package NAME --out FILE"
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

fn list<'a>(object: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a Vec<Value>, String> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| format!("{at} '{key}' is not a list"))
}

fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, String> {
    list(object, key, at)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at} '{key}' holds a non-string"))
        })
        .collect()
}

fn entries<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    at: &str,
) -> Result<Vec<&'a Map<String, Value>>, String> {
    list(object, key, at)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_object()
                .ok_or_else(|| format!("{at} '{key}' entry {index} is not an object"))
        })
        .collect()
}

/// Returns the symbol kind of one LDoc module type.
fn module_kind(name: &str, at: &str) -> Result<Option<SymbolKind>, String> {
    Ok(match name {
        "module" | "script" => Some(SymbolKind::Module),
        "classmod" => Some(SymbolKind::Class),
        // Examples and topics carry prose, not API.
        "example" | "topic" => None,
        other => return Err(format!("{at} has unknown module type '{other}'")),
    })
}

/// Returns the symbol kind of one LDoc item type, or None for items that are not API.
fn item_kind(name: &str, item_name: &str, at: &str) -> Result<Option<SymbolKind>, String> {
    Ok(match name {
        "function" if item_name.contains(':') => Some(SymbolKind::Method),
        "function" => Some(SymbolKind::Function),
        "factory" | "constructor" => Some(SymbolKind::Function),
        "table" => Some(SymbolKind::Variable),
        "field" => Some(SymbolKind::Field),
        "lfunction" | "annotation" => None,
        other => return Err(format!("{at} has unknown item type '{other}'")),
    })
}

/// Returns the escaped qualified part of one dotted name.
fn qualified(name: &str) -> String {
    name.split('.')
        .filter(|segment| !segment.is_empty())
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

/// Normalizes CRLF line endings and trims trailing whitespace.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

/// Joins the summary and the description into one markdown body.
fn body(summary: &str, description: &str) -> String {
    let summary = normalize(summary);
    let description = normalize(description);
    match (summary.is_empty(), description.is_empty()) {
        (_, true) => summary,
        (true, false) => description,
        (false, false) => format!("{summary}\n\n{description}"),
    }
}

/// Returns the item name without a leading `module.` prefix.
fn local_name<'a>(module: &str, name: &'a str) -> &'a str {
    name.strip_prefix(module)
        .and_then(|rest| rest.strip_prefix('.'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(name)
}

/// Converts one LDoc filter document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("LDoc JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("LDoc JSON is not an object")?;
    let at = "LDoc JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "LDoc JSON format {other} is not supported, expected {FORMAT}"
            ));
        }
    }
    let config = format!(
        "lua={};ldoc={};format={FORMAT}",
        string(root, "lua", at)?,
        string(root, "ldoc", at)?
    );
    let prefix = format!("{LANGUAGE}:{package}:");
    let mut symbols = Vec::new();
    for module in entries(root, "modules", at)? {
        let at = "LDoc JSON module";
        let name = string(module, "name", at)?;
        if name.is_empty() {
            return Err(format!("{at} has an empty name"));
        }
        let at = format!("{at} '{name}'");
        let Some(kind) = module_kind(&string(module, "kind", &at)?, &at)? else {
            continue;
        };
        let file = string(module, "file", &at)?;
        let generated = field(module, "generated", &at)?
            .as_bool()
            .ok_or_else(|| format!("{at} 'generated' is not a boolean"))?;
        let module_id = format!("{prefix}{}", qualified(&name));
        let doc = body(
            &string(module, "summary", &at)?,
            &string(module, "description", &at)?,
        );
        symbols.push(symbol(
            module_id.clone(),
            kind,
            format!("module {name}"),
            name.clone(),
            doc,
            Vec::new(),
            None,
            SourceRef {
                file: file.clone(),
                line: number(module, "line", &at)?,
                generated,
            },
            None,
            &config,
        ));
        for item in entries(module, "items", &at)? {
            let item_at = format!("{at} item");
            let item_name = string(item, "name", &item_at)?;
            if item_name.is_empty() {
                return Err(format!("{item_at} has an empty name"));
            }
            let item_at = format!("{at} item '{item_name}'");
            let Some(item_kind) = item_kind(&string(item, "kind", &item_at)?, &item_name, &item_at)?
            else {
                continue;
            };
            let short = local_name(&name, &item_name);
            let args = string(item, "args", &item_at)?;
            let params: Vec<Param> = entries(item, "params", &item_at)?
                .into_iter()
                .map(|param| {
                    Ok(Param {
                        name: string(param, "name", &item_at)?,
                        r#type: string(param, "type", &item_at)?,
                        doc: normalize(&string(param, "doc", &item_at)?),
                    })
                })
                .collect::<Result<_, String>>()?;
            let returns = entries(item, "returns", &item_at)?;
            let returns = match returns.as_slice() {
                [] => None,
                many => Some(Returns {
                    r#type: many
                        .iter()
                        .map(|r| string(r, "type", &item_at))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(", "),
                    doc: many
                        .iter()
                        .map(|r| string(r, "doc", &item_at).map(|d| normalize(&d)))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter(|d| !d.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                }),
            };
            let signature = match item_kind {
                SymbolKind::Function | SymbolKind::Method => {
                    format!("function {name}.{short}{args}")
                }
                _ => format!("{name}.{short}"),
            };
            symbols.push(symbol(
                format!("{prefix}{}.{}", qualified(&name), qualified(short)),
                item_kind,
                signature,
                short.to_owned(),
                body(
                    &string(item, "summary", &item_at)?,
                    &string(item, "description", &item_at)?,
                ),
                params,
                returns,
                SourceRef {
                    file: file.clone(),
                    line: number(item, "line", &item_at)?,
                    generated,
                },
                Some(module_id.clone()),
                &config,
            ));
            // Usage snippets are examples; they travel with the symbol just pushed.
            let examples = strings(item, "usage", &item_at)?;
            if let Some(last) = symbols.last_mut() {
                last.examples = examples.iter().map(|e| normalize(e)).collect();
            }
        }
    }
    if symbols.is_empty() {
        return Err(format!(
            "LDoc JSON lists no public Lua API of package '{package}'"
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

#[allow(clippy::too_many_arguments)]
fn symbol(
    id: String,
    kind: SymbolKind,
    signature_text: String,
    display_name: String,
    doc: String,
    params: Vec<Param>,
    returns: Option<Returns>,
    source: SourceRef,
    member_of: Option<String>,
    config: &str,
) -> Symbol {
    let mut incomplete = Vec::new();
    if doc.is_empty() {
        incomplete.push("doc");
    }
    let mut extensions = vec![extension(KEY_CONFIG, config)];
    if !incomplete.is_empty() {
        extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
    }
    Symbol {
        id,
        kind: kind as i32,
        signature_text,
        doc_markdown: doc,
        params,
        returns,
        source: Some(source),
        visibility: Visibility::Public as i32,
        relations: member_of.map(|parent| Relations {
            member_of: vec![parent],
            implements: Vec::new(),
            reexported_as: Vec::new(),
        }),
        extensions,
        display_name,
        ..Symbol::default()
    }
}

#[cfg(test)]
mod lib_tests;
