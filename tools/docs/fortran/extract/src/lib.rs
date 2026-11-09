//! Converts the FORD driver's JSON into one validated DocIr shard.

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
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const LANGUAGE: &str = "fortran";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "fortran.config";
const KEY_INCOMPLETE: &str = "fortran.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: fortran_extract --json FILE --package NAME --out FILE"
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

/// Returns the id of one dotted object path, with every segment escaped.
fn symbol_id(package: &str, path: &str) -> String {
    let qualified: Vec<String> = path
        .split('.')
        .map(documentation_ir::escape_id_segment)
        .collect();
    format!("{LANGUAGE}:{package}:{}", qualified.join("."))
}

fn kind_of(kind: &str, signature: &str, at: &str) -> Result<SymbolKind, String> {
    Ok(match kind {
        "module" => SymbolKind::Module,
        "type" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "function" | "subroutine" => SymbolKind::Function,
        "binding" => SymbolKind::Method,
        "component" => SymbolKind::Field,
        "variable" if signature.contains(", parameter") => SymbolKind::Constant,
        "variable" => SymbolKind::Variable,
        other => return Err(format!("{at} has unknown kind '{other}'")),
    })
}

fn params(object: &Map<String, Value>, at: &str, incomplete: &mut Vec<String>) -> Result<Vec<Param>, String> {
    let list = field(object, "params", at)?
        .as_array()
        .ok_or_else(|| format!("{at} 'params' is not a list"))?;
    let mut undocumented = Vec::new();
    let mut out = Vec::new();
    for (index, value) in list.iter().enumerate() {
        let at = format!("{at} param {index}");
        let value = value
            .as_object()
            .ok_or_else(|| format!("{at} is not an object"))?;
        let name = string(value, "name", &at)?;
        let doc = normalize(&string(value, "doc", &at)?);
        if doc.is_empty() {
            undocumented.push(name.clone());
        }
        out.push(Param {
            name,
            r#type: string(value, "type", &at)?,
            doc,
        });
    }
    if !undocumented.is_empty() {
        incomplete.push(format!("params={}", undocumented.join(",")));
    }
    Ok(out)
}

fn returns(object: &Map<String, Value>, at: &str, incomplete: &mut Vec<String>) -> Result<Option<Returns>, String> {
    match field(object, "returns", at)? {
        Value::Null => Ok(None),
        Value::Object(returns) => {
            let at = format!("{at} returns");
            let doc = normalize(&string(returns, "doc", &at)?);
            if doc.is_empty() {
                incomplete.push("return_doc".to_owned());
            }
            Ok(Some(Returns {
                r#type: string(returns, "type", &at)?,
                doc,
            }))
        }
        _ => Err(format!("{at} 'returns' is neither null nor an object")),
    }
}

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("FORD JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("FORD JSON is not an object")?;
    let at = "FORD JSON";
    let format = number(root, "format", at)?;
    if format != FORMAT {
        return Err(format!(
            "FORD JSON format {format} is not supported, expected {FORMAT}"
        ));
    }
    let config = format!("ford={};format={FORMAT}", string(root, "ford", at)?);
    let objects = field(root, "objects", at)?
        .as_array()
        .ok_or("FORD JSON 'objects' is not a list")?;

    let mut paths = BTreeSet::new();
    let mut symbols = Vec::new();
    for (index, value) in objects.iter().enumerate() {
        let at = format!("FORD JSON object {index}");
        let object = value
            .as_object()
            .ok_or_else(|| format!("{at} is not an object"))?;
        let path = string(object, "path", &at)?;
        let name = string(object, "name", &at)?;
        if path.is_empty() || name.is_empty() {
            return Err(format!("{at} has an empty path or name"));
        }
        if !paths.insert(path.clone()) {
            return Err(format!("{at} repeats the path '{path}'"));
        }
        let signature = string(object, "signature", &at)?;
        let kind = kind_of(&string(object, "kind", &at)?, &signature, &at)?;
        let mut incomplete = Vec::new();
        let doc = normalize(&string(object, "doc", &at)?);
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        let params = params(object, &at, &mut incomplete)?;
        let returns = returns(object, &at, &mut incomplete)?;
        let parent = string(object, "parent", &at)?;
        let extends = string(object, "extends", &at)?;
        let mut implements = Vec::new();
        if !extends.is_empty() {
            let module = path.rsplit_once('.').map_or("", |(module, _)| module);
            implements.push(symbol_id(package, &format!("{module}.{extends}")));
        }
        let member_of: Vec<String> = if parent.is_empty() {
            Vec::new()
        } else {
            vec![symbol_id(package, &parent)]
        };
        let relations = if member_of.is_empty() && implements.is_empty() {
            None
        } else {
            Some(Relations {
                member_of,
                implements,
                reexported_as: Vec::new(),
            })
        };
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if !incomplete.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
        }
        symbols.push(Symbol {
            id: symbol_id(package, &path),
            kind: kind as i32,
            signature_text: signature,
            doc_markdown: doc,
            params,
            returns,
            source: Some(SourceRef {
                file: string(object, "file", &at)?,
                line: number(object, "line", &at)?,
                generated: false,
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
            "FORD JSON lists no public Fortran API of package '{package}'"
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
