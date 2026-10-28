//! Converts a JVM doc adapter's "jvm api v1" JSON into one validated DocIr shard.
//!
//! Id rules: Kotlin top-level declarations use the Kotlin FQ name, never `FileKt`. An
//! overload list holds erased simple types without `?` or generics. An extension's receiver
//! type comes first, so `fun Circle.grow(by: Double)` is `grow(Circle,Double)`. A companion
//! object is `Circle.Companion`. Kinds `object`, `trait`, `typealias`, `function` and
//! `property` are accepted only for the languages that have them.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use documentation_ir::proto::{
    DocIr, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{escape_segment, SCHEMA_MAJOR, SCHEMA_MINOR};
use serde_json::{Map, Value};

const FORMAT: u64 = 1;
const LANGUAGES: [&str; 3] = ["java", "kotlin", "scala"];

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub language: String,
    pub package: String,
    pub srcs_manifest: String,
    pub dep_shards: Vec<String>,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: jvm_convert --json FILE --language java|kotlin|scala --package NAME \
     --srcs-manifest FILE [--dep-shard FILE]... --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
        if flag == "--dep-shard" {
            options.dep_shards.push(value.clone());
            index += 2;
            continue;
        }
        let slot = match flag {
            "--json" => &mut options.json,
            "--language" => &mut options.language,
            "--package" => &mut options.package,
            "--srcs-manifest" => &mut options.srcs_manifest,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        if !slot.is_empty() {
            return Err(format!("{flag} given more than once"));
        }
        *slot = value.clone();
        index += 2;
    }
    for (flag, value) in [
        ("--json", &options.json),
        ("--language", &options.language),
        ("--package", &options.package),
        ("--srcs-manifest", &options.srcs_manifest),
        ("--out", &options.out),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    if !LANGUAGES.contains(&options.language.as_str()) {
        return Err(format!(
            "--language '{}' is not supported",
            options.language
        ));
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard, leaving no output on failure.
pub fn run(options: &Options) -> Result<(), String> {
    let read = |path: &str| fs::read(path).map_err(|error| format!("cannot read {path}: {error}"));
    let json = String::from_utf8(read(&options.json)?)
        .map_err(|_| format!("{} is not UTF-8", options.json))?;
    let manifest = String::from_utf8(read(&options.srcs_manifest)?)
        .map_err(|_| format!("{} is not UTF-8", options.srcs_manifest))?;
    let mut deps = Vec::new();
    for path in &options.dep_shards {
        let shard = documentation_ir::decode_shard(&read(path)?)
            .map_err(|error| format!("cannot decode dep shard {path}: {error:?}"))?;
        deps.push(shard);
    }
    let shard = convert(&json, &options.language, &options.package, &manifest, &deps)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    let out = Path::new(&options.out);
    let temp = out.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    fs::rename(&temp, out).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("cannot write {}: {error}", options.out)
    })
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

fn optional<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    object.get(key).filter(|value| !value.is_null())
}

fn strings(value: &Value, what: &str, at: &str) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{at} '{what}' is not a list"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at} '{what}' holds a non-string"))
        })
        .collect()
}

struct Kind {
    kind: SymbolKind,
    callable: bool,
}

const JAVA_KINDS: &[&str] = &[
    "constructor",
    "method",
    "field",
    "constant",
    "class",
    "record",
    "interface",
    "annotation",
    "enum",
    "enum_constant",
    "package",
];
const KOTLIN_KINDS: &[&str] = &[
    "constructor",
    "function",
    "property",
    "class",
    "object",
    "interface",
    "annotation",
    "enum",
    "enum_constant",
    "typealias",
    "package",
];
const SCALA_KINDS: &[&str] = &[
    "constructor",
    "function",
    "property",
    "class",
    "object",
    "trait",
    "typealias",
    "package",
];

fn kind_of(name: &str, language: &str, at: &str) -> Result<Kind, String> {
    let (kind, callable) = match name {
        "constructor" | "method" => (SymbolKind::Method, true),
        "function" => (SymbolKind::Function, true),
        "field" => (SymbolKind::Field, false),
        "property" => (SymbolKind::Property, false),
        "constant" => (SymbolKind::Constant, false),
        "class" | "record" | "object" => (SymbolKind::Class, false),
        "interface" | "annotation" | "trait" => (SymbolKind::Interface, false),
        "enum" => (SymbolKind::Enum, false),
        "enum_constant" => (SymbolKind::EnumVariant, false),
        "typealias" => (SymbolKind::TypeAlias, false),
        "package" => (SymbolKind::Module, false),
        other => return Err(format!("{at} has unknown kind '{other}'")),
    };
    let allowed = match language {
        "java" => JAVA_KINDS,
        "kotlin" => KOTLIN_KINDS,
        _ => SCALA_KINDS,
    };
    if !allowed.contains(&name) {
        return Err(format!("{at} has kind '{name}', not valid for {language}"));
    }
    Ok(Kind { kind, callable })
}

/// The reader-facing key of a symbol: `a.B.m(int,String)`.
fn key_of(qualified: &str, overload: Option<&[String]>) -> String {
    match overload {
        Some(types) => format!("{qualified}({})", types.join(",")),
        None => qualified.to_owned(),
    }
}

fn id_of(prefix: &str, qualified: &str, overload: Option<&[String]>) -> String {
    let mut segments: Vec<&str> = qualified.split('.').collect();
    let last = segments.pop().unwrap_or_default();
    let mut id = String::from(prefix);
    for segment in segments {
        id.push_str(&escape_segment(segment));
        id.push('.');
    }
    id.push_str(&escape_segment(&key_of(last, overload)));
    id
}

fn unescape(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

struct Manifest(BTreeMap<String, (String, bool)>);

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut map = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        let [exec, rel, generated] = parts.as_slice() else {
            return Err(format!("srcs manifest line {} is malformed", index + 1));
        };
        map.insert(
            (*exec).replace('\\', "/"),
            ((*rel).replace('\\', "/"), *generated == "1"),
        );
    }
    Ok(Manifest(map))
}

struct Resolver {
    keys: BTreeMap<String, String>,
}

impl Resolver {
    fn resolve(&self, target: &str) -> Option<&String> {
        self.keys.get(target.trim())
    }

    /// Rewrites `[label](jvm:target)` links to `dx:` links or code spans.
    fn rewrite(&self, doc: &str) -> String {
        let mut out = String::with_capacity(doc.len());
        let mut opens: Vec<usize> = Vec::new();
        let mut rest = doc;
        while let Some(ch) = rest.chars().next() {
            if ch == '[' {
                opens.push(out.len());
            } else if ch == ']' {
                let open = opens.pop();
                if let (Some(open), Some(after)) = (open, rest[1..].strip_prefix("(jvm:")) {
                    if let Some(end) = target_end(after) {
                        let target = &after[..end];
                        let label = out[open + 1..].replace('`', "");
                        out.truncate(open);
                        match self.resolve(target) {
                            Some(id) => out.push_str(&format!("[{label}](dx:{id})")),
                            None => out.push_str(&format!("`{label}`")),
                        }
                        rest = &after[end + 1..];
                        continue;
                    }
                }
            }
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
        out
    }
}

fn target_end(after: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in after.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(index),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

struct Record {
    id: String,
    key: String,
    symbol: Symbol,
    member_of: Option<String>,
    implements: Vec<String>,
    doc: String,
}

fn record(
    item: &Value,
    index: usize,
    prefix: &str,
    language: &str,
    manifest: &Manifest,
) -> Result<Record, String> {
    let at = format!("JVM doc JSON symbol {index}");
    let item = item
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))?;
    let qualified = string(item, "qualified", &at)?;
    if qualified.is_empty() || qualified.split('.').any(str::is_empty) {
        return Err(format!("{at} has an invalid name '{qualified}'"));
    }
    let kind = kind_of(&string(item, "kind", &at)?, language, &at)?;
    let overload = match optional(item, "overload") {
        Some(value) => Some(strings(value, "overload", &at)?),
        None => None,
    };
    if kind.callable != overload.is_some() {
        return Err(format!(
            "{at} '{qualified}' has an overload list that does not match its kind"
        ));
    }
    let file = string(item, "file", &at)?.replace('\\', "/");
    let Some((rel, generated)) = manifest.0.get(&file) else {
        return Err(format!(
            "{at} '{qualified}' names {file}, not in the srcs manifest"
        ));
    };
    let line = field(item, "line", &at)?
        .as_u64()
        .filter(|line| *line > 0)
        .ok_or_else(|| format!("{at} '{qualified}' has no positive line"))?;
    let types = overload.as_deref();
    let key = key_of(&qualified, types);
    let id = id_of(prefix, &qualified, types);
    let display_name = match types {
        Some(types) => format!("{qualified}({})", types.join(", ")),
        None => qualified.clone(),
    };
    let mut params = Vec::new();
    if let Some(list) = optional(item, "params") {
        for param in list
            .as_array()
            .ok_or_else(|| format!("{at} 'params' is not a list"))?
        {
            let param = param
                .as_object()
                .ok_or_else(|| format!("{at} 'params' holds a non-object"))?;
            params.push(Param {
                name: string(param, "name", &at)?,
                r#type: string(param, "type", &at)?,
                doc: string(param, "doc", &at)?,
            });
        }
    }
    let returns = match optional(item, "returns") {
        Some(value) => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{at} 'returns' is not an object"))?;
            Some(Returns {
                r#type: string(object, "type", &at)?,
                doc: string(object, "doc", &at)?,
            })
        }
        None => None,
    };
    let member_of = match optional(item, "member_of") {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| format!("{at} 'member_of' is not a string"))?
                .to_owned(),
        ),
        None => None,
    };
    let implements = match optional(item, "implements") {
        Some(value) => strings(value, "implements", &at)?,
        None => Vec::new(),
    };
    let symbol = Symbol {
        id: id.clone(),
        kind: kind.kind as i32,
        signature_text: string(item, "signature", &at)?,
        params,
        returns,
        source: Some(SourceRef {
            file: rel.clone(),
            line,
            generated: *generated,
        }),
        visibility: Visibility::Public as i32,
        display_name,
        ..Symbol::default()
    };
    Ok(Record {
        id,
        key,
        symbol,
        member_of,
        implements,
        doc: string(item, "doc", &at)?,
    })
}

/// Converts one adapter JSON document into a validated IR shard.
pub fn convert(
    json: &str,
    language: &str,
    package: &str,
    srcs_manifest: &str,
    deps: &[DocIr],
) -> Result<DocIr, String> {
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("JVM doc JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("JVM doc JSON is not an object")?;
    let at = "JVM doc JSON";
    match field(root, "format", at)?.as_u64() {
        Some(FORMAT) => {}
        other => {
            return Err(format!(
                "JVM doc JSON format {other:?} is not supported, expected {FORMAT}"
            ))
        }
    }
    let found = string(root, "language", at)?;
    if found != language {
        return Err(format!(
            "JVM doc JSON language '{found}' does not match '{language}'"
        ));
    }
    string(root, "tool", at)?;
    string(root, "tool_version", at)?;
    if package.is_empty() || package.contains(':') {
        return Err(format!("invalid package '{package}'"));
    }
    let manifest = parse_manifest(srcs_manifest)?;
    let items = field(root, "symbols", at)?
        .as_array()
        .ok_or("JVM doc JSON 'symbols' is not a list")?;
    if items.is_empty() {
        return Err("JVM doc JSON lists no public symbols".to_owned());
    }
    let prefix = format!("{language}:{package}:");
    let mut records = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let record = record(item, index, &prefix, language, &manifest)?;
        if let Some(first) = seen.insert(record.id.clone(), record.key.clone()) {
            return Err(format!(
                "duplicate id {}: '{first}' and '{}'",
                record.id, record.key
            ));
        }
        records.push(record);
    }
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    for dep in deps {
        if !LANGUAGES.contains(&dep.language.as_str()) {
            return Err(format!(
                "dep shard language '{}' is not a JVM language",
                dep.language
            ));
        }
        let dep_prefix = format!("{}:{}:", dep.language, dep.package);
        for symbol in &dep.symbols {
            if let Some(key) = symbol.id.strip_prefix(&dep_prefix).and_then(unescape) {
                keys.insert(key, symbol.id.clone());
            }
        }
    }
    for record in &records {
        keys.insert(record.key.clone(), record.id.clone());
    }
    let resolver = Resolver { keys };
    let mut symbols = Vec::new();
    for mut record in records {
        let member_of: Vec<String> = record
            .member_of
            .iter()
            .filter_map(|target| resolver.resolve(target).cloned())
            .collect();
        let implements: BTreeSet<String> = record
            .implements
            .iter()
            .filter_map(|target| resolver.resolve(target).cloned())
            .collect();
        if !member_of.is_empty() || !implements.is_empty() {
            record.symbol.relations = Some(Relations {
                member_of,
                implements: implements.into_iter().collect(),
                ..Relations::default()
            });
        }
        record.symbol.doc_markdown = resolver.rewrite(&record.doc);
        symbols.push(record.symbol);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: language.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
