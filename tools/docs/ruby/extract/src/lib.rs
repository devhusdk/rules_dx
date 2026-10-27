//! Converts the YARD driver's JSON into one validated DocIr shard.

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

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};
use serde_json::{Map, Value};

const LANGUAGE: &str = "ruby";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "ruby.config";
const KEY_INCOMPLETE: &str = "ruby.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: ruby_extract --json FILE --package NAME --out FILE"
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

/// Escapes one id segment: every byte outside `[A-Za-z0-9_]` becomes `~` plus two hex digits.
pub fn escape_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("~{byte:02X}"));
        }
    }
    out
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

/// One YARD tag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tag {
    tag: String,
    name: String,
    types: Vec<String>,
    text: String,
}

/// Which accessors one `attr_*` declaration defines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Attribute {
    name: String,
    read: bool,
    write: bool,
}

/// The type of one YARD code object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Module,
    Class,
    Method,
    Constant,
}

/// One YARD code object of the driver JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Object {
    path: String,
    kind: Type,
    name: String,
    namespace: String,
    singleton: bool,
    visibility: String,
    signature: String,
    docstring: String,
    tags: Vec<Tag>,
    file: String,
    generated: bool,
    line: u64,
    owner: String,
    superclass: String,
    mixins: Vec<String>,
    parameters: Vec<String>,
    attribute: Option<Attribute>,
    value: String,
}

fn tags(object: &Map<String, Value>, at: &str) -> Result<Vec<Tag>, String> {
    let list = field(object, "tags", at)?
        .as_array()
        .ok_or_else(|| format!("{at} 'tags' is not a list"))?;
    list.iter()
        .enumerate()
        .map(|(index, tag)| {
            let at = format!("{at} tag {index}");
            let tag = tag
                .as_object()
                .ok_or_else(|| format!("{at} is not an object"))?;
            Ok(Tag {
                tag: string(tag, "tag", &at)?,
                name: string(tag, "name", &at)?,
                types: strings(tag, "types", &at)?,
                text: string(tag, "text", &at)?,
            })
        })
        .collect()
}

fn attribute(object: &Map<String, Value>, at: &str) -> Result<Option<Attribute>, String> {
    match field(object, "attribute", at)? {
        Value::Null => Ok(None),
        Value::Object(attribute) => {
            let at = format!("{at} attribute");
            Ok(Some(Attribute {
                name: string(attribute, "name", &at)?,
                read: boolean(attribute, "read", &at)?,
                write: boolean(attribute, "write", &at)?,
            }))
        }
        _ => Err(format!("{at} 'attribute' is neither null nor an object")),
    }
}

fn object(value: &Value, at: &str) -> Result<Object, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))?;
    let kind = match string(object, "type", at)?.as_str() {
        "module" => Type::Module,
        "class" => Type::Class,
        "method" => Type::Method,
        "constant" => Type::Constant,
        other => return Err(format!("{at} has unknown type '{other}'")),
    };
    let singleton = match string(object, "scope", at)?.as_str() {
        "class" => true,
        "instance" | "" => false,
        other => return Err(format!("{at} has unknown scope '{other}'")),
    };
    let path = string(object, "path", at)?;
    let name = string(object, "name", at)?;
    if path.is_empty() || name.is_empty() {
        return Err(format!("{at} has an empty path or name"));
    }
    let mut mixins = strings(object, "includes", at)?;
    mixins.extend(strings(object, "extends", at)?);
    Ok(Object {
        path,
        kind,
        name,
        namespace: string(object, "namespace", at)?,
        singleton,
        visibility: string(object, "visibility", at)?,
        signature: string(object, "signature", at)?,
        docstring: string(object, "docstring", at)?,
        tags: tags(object, at)?,
        file: string(object, "file", at)?,
        generated: boolean(object, "generated", at)?,
        line: number(object, "line", at)?,
        owner: string(object, "owner", at)?,
        superclass: string(object, "superclass", at)?,
        mixins,
        parameters: strings(object, "parameters", at)?,
        attribute: attribute(object, at)?,
        value: string(object, "value", at)?,
    })
}

/// Returns the qualified-name segments of one namespace path such as `A::B`.
fn namespace_segments(path: &str) -> Vec<&str> {
    if path.is_empty() {
        Vec::new()
    } else {
        path.split("::").collect()
    }
}

/// Returns the qualified part of one object's id, escaped.
fn qualified(object: &Object, name: &str) -> String {
    let mut segments: Vec<String> = namespace_segments(&object.namespace)
        .into_iter()
        .map(escape_segment)
        .collect();
    if object.kind == Type::Method && object.singleton {
        segments.push("self".to_owned());
    }
    segments.push(escape_segment(name));
    segments.join(".")
}

/// Returns the qualified part of one namespace path's id, escaped.
fn namespace_qualified(path: &str) -> String {
    namespace_segments(path)
        .into_iter()
        .map(escape_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn is_hidden_itself(object: &Object) -> bool {
    object.visibility != "public"
        || object
            .tags
            .iter()
            .any(|tag| tag.tag == "private" || (tag.tag == "api" && tag.text.trim() == "private"))
}

/// Normalizes CRLF line endings and trims trailing whitespace.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

fn parameter_name(parameter: &str) -> Option<String> {
    if parameter.starts_with('&') {
        return None;
    }
    let name = parameter
        .trim_start_matches('*')
        .trim_end_matches(':')
        .to_owned();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The documented shape of one emitted symbol before ids are resolved.
struct Draft<'a> {
    object: &'a Object,
    name: String,
    kind: SymbolKind,
    signature: String,
}

fn draft(object: &Object) -> Option<Draft<'_>> {
    let (kind, name, signature) = match (object.kind, &object.attribute) {
        (Type::Module, _) => (
            SymbolKind::Module,
            object.name.clone(),
            format!("module {}", object.path),
        ),
        (Type::Class, _) => {
            let signature = if object.superclass.is_empty() {
                format!("class {}", object.path)
            } else {
                format!("class {} < {}", object.path, object.superclass)
            };
            (SymbolKind::Class, object.name.clone(), signature)
        }
        (Type::Constant, _) => {
            let value = object.value.replace("\r\n", "\n");
            let first = value.lines().next().unwrap_or_default().trim_end();
            (
                SymbolKind::Constant,
                object.name.clone(),
                format!("{} = {first}", object.path),
            )
        }
        (Type::Method, Some(attribute)) => {
            let writer = object.name.ends_with('=');
            if writer && attribute.read {
                return None;
            }
            let (macro_name, name) = match (attribute.read, attribute.write) {
                (true, true) => ("attr_accessor", attribute.name.clone()),
                (true, false) => ("attr_reader", attribute.name.clone()),
                _ => ("attr_writer", format!("{}=", attribute.name)),
            };
            let signature = format!("{macro_name} :{}", attribute.name);
            (SymbolKind::Property, name, signature)
        }
        (Type::Method, None) => {
            let mut signature = object.signature.replace("\r\n", "\n");
            if object.singleton && !signature.starts_with("def self.") {
                if let Some(rest) = signature.strip_prefix("def ") {
                    signature = format!("def self.{rest}");
                }
            }
            (SymbolKind::Method, object.name.clone(), signature)
        }
    };
    Some(Draft {
        object,
        name,
        kind,
        signature,
    })
}

fn params_and_incomplete(draft: &Draft, incomplete: &mut Vec<String>) -> Vec<Param> {
    let object = draft.object;
    if draft.kind != SymbolKind::Method {
        return Vec::new();
    }
    let mut params = Vec::new();
    let mut undocumented = Vec::new();
    let mut untyped = Vec::new();
    for name in object.parameters.iter().filter_map(|p| parameter_name(p)) {
        let tag = object
            .tags
            .iter()
            .find(|tag| tag.tag == "param" && tag.name == name);
        match tag {
            None => undocumented.push(name.clone()),
            Some(tag) if tag.types.is_empty() => untyped.push(name.clone()),
            Some(_) => {}
        }
        params.push(Param {
            name,
            r#type: tag.map(|t| t.types.join(", ")).unwrap_or_default(),
            doc: tag.map(|t| normalize(&t.text)).unwrap_or_default(),
        });
    }
    if !undocumented.is_empty() {
        incomplete.push(format!("params={}", undocumented.join(",")));
    }
    if !untyped.is_empty() {
        incomplete.push(format!("param_types={}", untyped.join(",")));
    }
    params
}

fn returns_and_incomplete(draft: &Draft, incomplete: &mut Vec<String>) -> Option<Returns> {
    let object = draft.object;
    let callable = matches!(draft.kind, SymbolKind::Method | SymbolKind::Property);
    if !callable {
        return None;
    }
    let tag = object.tags.iter().find(|tag| tag.tag == "return");
    let writer_only = draft.kind == SymbolKind::Property && draft.name.ends_with('=');
    let exempt = writer_only || (object.name == "initialize" && !object.singleton);
    if !exempt && tag.is_none_or(|tag| tag.types.is_empty()) {
        incomplete.push("return_type".to_owned());
    }
    tag.map(|tag| Returns {
        r#type: tag.types.join(", "),
        doc: normalize(&tag.text),
    })
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
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("YARD JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("YARD JSON is not an object")?;
    let at = "YARD JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "YARD JSON format {other} is not supported, expected {FORMAT}"
            ))
        }
    }
    let config = format!(
        "ruby={};yard={};format={FORMAT}",
        string(root, "ruby", at)?,
        string(root, "yard", at)?
    );
    let objects: Vec<Object> = field(root, "objects", at)?
        .as_array()
        .ok_or("YARD JSON 'objects' is not a list")?
        .iter()
        .enumerate()
        .map(|(index, value)| object(value, &format!("YARD JSON object {index}")))
        .collect::<Result<_, _>>()?;

    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for object in &objects {
        if matches!(object.kind, Type::Module | Type::Class) {
            owners.insert(object.path.as_str(), object.owner.as_str());
        }
    }
    let namespace_id = |path: &str| -> Option<String> {
        let owner = owners.get(path)?;
        let package = if owner.is_empty() { package } else { owner };
        Some(format!(
            "{LANGUAGE}:{package}:{}",
            namespace_qualified(path)
        ))
    };
    let hidden_namespaces: BTreeSet<&str> = objects
        .iter()
        .filter(|object| matches!(object.kind, Type::Module | Type::Class))
        .filter(|object| is_hidden_itself(object))
        .map(|object| object.path.as_str())
        .collect();
    let hidden = |object: &Object| -> bool {
        if is_hidden_itself(object) {
            return true;
        }
        let mut namespace = object.namespace.as_str();
        while !namespace.is_empty() {
            if hidden_namespaces.contains(namespace) {
                return true;
            }
            namespace = namespace.rsplit_once("::").map_or("", |(parent, _)| parent);
        }
        false
    };

    let prefix = format!("{LANGUAGE}:{package}:");
    let mut symbols = Vec::new();
    for object in objects.iter().filter(|o| o.owner.is_empty() && !hidden(o)) {
        let Some(draft) = draft(object) else {
            continue;
        };
        let mut incomplete = Vec::new();
        let doc = normalize(&object.docstring);
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        let params = params_and_incomplete(&draft, &mut incomplete);
        let returns = returns_and_incomplete(&draft, &mut incomplete);
        let member_of: Vec<String> = namespace_id(&object.namespace).into_iter().collect();
        let mut implements: Vec<String> = object
            .mixins
            .iter()
            .filter_map(|path| namespace_id(path))
            .collect();
        implements.sort();
        implements.dedup();
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
            id: format!("{prefix}{}", qualified(object, &draft.name)),
            kind: draft.kind as i32,
            signature_text: draft.signature,
            doc_markdown: doc,
            params,
            returns,
            source: Some(SourceRef {
                file: object.file.clone(),
                line: object.line,
                generated: object.generated,
            }),
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: object.path.clone(),
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "YARD JSON lists no public Ruby API of package '{package}'"
        ));
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
