//! Converts the Documenter driver's JSON into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::fs;

use documentation_ir::proto::{
    DocIr, Extension, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const LANGUAGE: &str = "julia";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "julia.config";
const KEY_INCOMPLETE: &str = "julia.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: julia_extract --json FILE --package NAME --out FILE"
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

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| format!("{at} '{key}' is not a boolean"))
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

/// One documented binding of the driver JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Object {
    /// Dotted path of the binding, such as `Geo.Shapes.area`.
    path: String,
    name: String,
    /// Dotted path of the enclosing module.
    module: String,
    kind: SymbolKind,
    signatures: Vec<String>,
    doc: String,
    file: String,
    generated: bool,
    line: u64,
}

fn kind_of(name: &str, at: &str) -> Result<SymbolKind, String> {
    Ok(match name {
        "module" => SymbolKind::Module,
        "struct" | "mutable_struct" | "primitive" => SymbolKind::Class,
        "abstract" => SymbolKind::Interface,
        "function" => SymbolKind::Function,
        "macro" => SymbolKind::Macro,
        "constant" => SymbolKind::Constant,
        "alias" => SymbolKind::TypeAlias,
        "variable" => SymbolKind::Variable,
        other => return Err(format!("{at} has unknown kind '{other}'")),
    })
}

fn object(value: &Value, at: &str) -> Result<Object, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))?;
    let path = string(object, "path", at)?;
    let name = string(object, "name", at)?;
    if path.is_empty() || name.is_empty() {
        return Err(format!("{at} has an empty path or name"));
    }
    Ok(Object {
        path,
        name,
        module: string(object, "module", at)?,
        kind: kind_of(&string(object, "kind", at)?, at)?,
        signatures: strings(object, "signatures", at)?,
        doc: string(object, "doc", at)?,
        file: string(object, "file", at)?,
        generated: boolean(object, "generated", at)?,
        line: number(object, "line", at)?,
    })
}

/// Returns the escaped qualified part of one dotted path.
///
/// A path segment never holds a dot, except the operators `.+`-style names, which the
/// driver reports with the leading dot kept inside one segment.
fn qualified(path: &str) -> String {
    path.split('.')
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

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("Documenter JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("Documenter JSON is not an object")?;
    let at = "Documenter JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "Documenter JSON format {other} is not supported, expected {FORMAT}"
            ));
        }
    }
    let config = format!(
        "julia={};documenter={};format={FORMAT}",
        string(root, "julia", at)?,
        string(root, "documenter", at)?
    );
    let objects: Vec<Object> = field(root, "objects", at)?
        .as_array()
        .ok_or("Documenter JSON 'objects' is not a list")?
        .iter()
        .enumerate()
        .map(|(index, value)| object(value, &format!("Documenter JSON object {index}")))
        .collect::<Result<_, _>>()?;

    let modules: BTreeSet<&str> = objects
        .iter()
        .filter(|o| o.kind == SymbolKind::Module)
        .map(|o| o.path.as_str())
        .collect();
    let prefix = format!("{LANGUAGE}:{package}:");
    let mut symbols = Vec::new();
    for object in &objects {
        let doc = normalize(&object.doc);
        let mut incomplete = Vec::new();
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        let member_of: Vec<String> = if modules.contains(object.module.as_str()) {
            vec![format!("{prefix}{}", qualified(&object.module))]
        } else {
            Vec::new()
        };
        let relations = if member_of.is_empty() {
            None
        } else {
            Some(Relations {
                member_of,
                implements: Vec::new(),
                reexported_as: Vec::new(),
            })
        };
        let signature_text = if object.signatures.is_empty() {
            match object.kind {
                SymbolKind::Module => format!("module {}", object.path),
                _ => object.path.clone(),
            }
        } else {
            object.signatures.join("\n")
        };
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if !incomplete.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
        }
        symbols.push(Symbol {
            id: format!("{prefix}{}", qualified(&object.path)),
            kind: object.kind as i32,
            signature_text,
            doc_markdown: doc,
            source: Some(SourceRef {
                file: object.file.clone(),
                line: object.line,
                generated: object.generated,
            }),
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: object.name.clone(),
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "Documenter JSON lists no public Julia API of package '{package}'"
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
