//! Converts the `index.json` of `dart doc` into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::fs;

use documentation_ir::proto::{DocIr, Extension, Relations, Symbol, SymbolKind, Visibility};
use serde_json::{Map, Value};

const LANGUAGE: &str = "dart";
const KEY_CONFIG: &str = "dart.config";
const KEY_INCOMPLETE: &str = "dart.incomplete";
const CONFIG: &str = "dartdoc=index.json";

/// The `Object` members dartdoc lists under every mixin.
const OBJECT_MEMBERS: [&str; 5] = [
    "hashCode",
    "noSuchMethod",
    "operator ==",
    "runtimeType",
    "toString",
];

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub index: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: dart_extract --index FILE --package NAME --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--index" => &mut options.index,
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
        ("--index", &options.index),
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
    let json = fs::read_to_string(&options.index)
        .map_err(|error| format!("cannot read {}: {error}", options.index))?;
    let shard = convert(&json, &options.package)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

/// One entry of the dartdoc index.
struct Entry {
    name: String,
    kind: u64,
    href: String,
    desc: String,
    parent: Option<String>,
}

fn text(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{at} has no string '{key}'"))
}

fn entry(value: &Value, at: &str) -> Result<Entry, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))?;
    let parent = match object.get("enclosedBy") {
        None | Some(Value::Null) => None,
        Some(Value::Object(parent)) => Some(text(parent, "href", &format!("{at} enclosedBy"))?),
        Some(_) => return Err(format!("{at} 'enclosedBy' is not an object")),
    };
    Ok(Entry {
        name: text(object, "name", at)?,
        kind: object
            .get("kind")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{at} has no numeric 'kind'"))?,
        href: text(object, "href", at)?,
        desc: text(object, "desc", at).unwrap_or_default(),
        parent,
    })
}

/// Maps a dartdoc `Kind` number to an IR kind and the declaration keyword.
fn kind_of(kind: u64, parent_kind: Option<u64>) -> Option<(SymbolKind, &'static str)> {
    Some(match kind {
        9 => (SymbolKind::Module, "library"),
        3 => (SymbolKind::Class, "class"),
        11 => (SymbolKind::Interface, "mixin"),
        5 => (SymbolKind::Enum, "enum"),
        6 => (SymbolKind::Class, "extension"),
        7 => (SymbolKind::Class, "extension type"),
        21 => (SymbolKind::TypeAlias, "typedef"),
        8 => (SymbolKind::Function, "function"),
        10 => (SymbolKind::Method, "method"),
        2 => (SymbolKind::Method, "constructor"),
        0 | 16 => (SymbolKind::Property, "property"),
        20 => (SymbolKind::Variable, "variable"),
        19 => (SymbolKind::Constant, "const"),
        1 if parent_kind == Some(5) => (SymbolKind::EnumVariant, "enum value"),
        1 => (SymbolKind::Constant, "const"),
        _ => return None,
    })
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

/// Converts one `index.json` document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("dartdoc index is malformed: {error}"))?;
    let list = root.as_array().ok_or("dartdoc index is not a list")?;
    let entries: Vec<Entry> = list
        .iter()
        .enumerate()
        .map(|(index, value)| entry(value, &format!("dartdoc index entry {index}")))
        .collect::<Result<_, _>>()?;
    let by_href: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.href.as_str(), e)).collect();

    let prefix = format!("{LANGUAGE}:{package}:");
    let mut ids: BTreeMap<&str, String> = BTreeMap::new();
    let mut symbols = Vec::new();
    for entry in &entries {
        let parent = match &entry.parent {
            Some(href) => Some(
                *by_href
                    .get(href.as_str())
                    .ok_or_else(|| format!("'{}' is enclosed by unknown '{href}'", entry.name))?,
            ),
            None => None,
        };
        let Some((kind, keyword)) = kind_of(entry.kind, parent.map(|p| p.kind)) else {
            continue;
        };
        if parent.is_some_and(|p| p.kind == 11) && OBJECT_MEMBERS.contains(&entry.name.as_str()) {
            continue;
        }
        let segment = documentation_ir::escape_segment(&entry.name);
        let qualified = match parent.and_then(|p| ids.get(p.href.as_str())) {
            Some(parent_id) => format!("{parent_id}.{segment}"),
            None if parent.is_some() => continue,
            None => segment,
        };
        ids.insert(entry.href.as_str(), qualified.clone());
        let member_of = parent
            .and_then(|p| ids.get(p.href.as_str()))
            .map(|id| format!("{prefix}{id}"))
            .into_iter()
            .collect::<Vec<_>>();
        let mut extensions = vec![extension(KEY_CONFIG, CONFIG)];
        if entry.desc.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, "doc"));
        }
        symbols.push(Symbol {
            id: format!("{prefix}{qualified}"),
            kind: kind as i32,
            signature_text: format!("{keyword} {}", entry.name),
            doc_markdown: entry.desc.trim().to_owned(),
            visibility: Visibility::Public as i32,
            relations: if member_of.is_empty() {
                None
            } else {
                Some(Relations {
                    member_of,
                    implements: Vec::new(),
                    reexported_as: Vec::new(),
                })
            },
            extensions,
            display_name: entry.name.clone(),
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "dartdoc index lists no public Dart API of package '{package}'"
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
