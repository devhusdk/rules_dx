//! Converts the YARD driver's JSON into one validated `DocIr` shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use documentation_ir::json::{JsonError, boolean, field, number, string, strings};
use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::{Map, Value};

const LANGUAGE: &str = "ruby";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "ruby.config";
const KEY_INCOMPLETE: &str = "ruby.incomplete";

/// Why one YARD JSON document could not become a shard.
#[derive(Debug)]
pub enum RubyExtractError {
    Read(PathIoError),
    InvalidPackage { package: String },
    Malformed(serde_json::Error),
    NotObject { at: String },
    NotList { at: String, key: &'static str },
    BadAttribute { at: String },
    Json(JsonError),
    UnsupportedFormat { found: u64 },
    UnknownType { at: String, kind: String },
    UnknownScope { at: String, scope: String },
    EmptyPathOrName { at: String },
    NoPublicApi { package: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for RubyExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed(error) => write!(f, "YARD JSON is malformed: {error}"),
            Self::NotObject { at } => write!(f, "{at} is not an object"),
            Self::NotList { at, key } => write!(f, "{at} '{key}' is not a list"),
            Self::BadAttribute { at } => {
                write!(f, "{at} 'attribute' is neither null nor an object")
            }
            Self::Json(error) => error.fmt(f),
            Self::UnsupportedFormat { found } => write!(
                f,
                "YARD JSON format {found} is not supported, expected {FORMAT}"
            ),
            Self::UnknownType { at, kind } => write!(f, "{at} has unknown type '{kind}'"),
            Self::UnknownScope { at, scope } => write!(f, "{at} has unknown scope '{scope}'"),
            Self::EmptyPathOrName { at } => write!(f, "{at} has an empty path or name"),
            Self::NoPublicApi { package } => write!(
                f,
                "YARD JSON lists no public Ruby API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RubyExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NotObject { .. }
            | Self::NotList { .. }
            | Self::BadAttribute { .. }
            | Self::UnsupportedFormat { .. }
            | Self::UnknownType { .. }
            | Self::UnknownScope { .. }
            | Self::EmptyPathOrName { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<JsonError> for RubyExtractError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

fn not_object(at: &str) -> RubyExtractError {
    RubyExtractError::NotObject { at: at.to_owned() }
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// Runs `ruby_extract` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("ruby_extract", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: ruby_extract --json FILE --package NAME --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(args, &["--json", "--package", "--out"])?;
    Ok(Options {
        json: flags.one("--json")?,
        package: flags.one("--package")?,
        out: flags.one("--out")?,
    })
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn run(options: &Options) -> Result<(), RubyExtractError> {
    let json = path_io::read_to_string(Path::new(&options.json)).map_err(RubyExtractError::Read)?;
    let shard = convert(&json, &options.package)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(RubyExtractError::Write)
}

pub use documentation_ir::escape_segment;

/// One YARD tag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tag {
    kind: String,
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

fn tags(object: &Map<String, Value>, at: &str) -> Result<Vec<Tag>, RubyExtractError> {
    let list = field(object, "tags", at)?
        .as_array()
        .ok_or_else(|| RubyExtractError::NotList {
            at: at.to_owned(),
            key: "tags",
        })?;
    list.iter()
        .enumerate()
        .map(|(index, tag)| {
            let at = format!("{at} tag {index}");
            let tag = tag.as_object().ok_or_else(|| not_object(&at))?;
            Ok(Tag {
                kind: string(tag, "tag", &at)?,
                name: string(tag, "name", &at)?,
                types: strings(tag, "types", &at)?,
                text: string(tag, "text", &at)?,
            })
        })
        .collect()
}

fn attribute(object: &Map<String, Value>, at: &str) -> Result<Option<Attribute>, RubyExtractError> {
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
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            Err(RubyExtractError::BadAttribute { at: at.to_owned() })
        }
    }
}

fn object(value: &Value, at: &str) -> Result<Object, RubyExtractError> {
    let object = value.as_object().ok_or_else(|| not_object(at))?;
    let kind = match string(object, "type", at)?.as_str() {
        "module" => Type::Module,
        "class" => Type::Class,
        "method" => Type::Method,
        "constant" => Type::Constant,
        other => {
            return Err(RubyExtractError::UnknownType {
                at: at.to_owned(),
                kind: other.to_owned(),
            });
        }
    };
    let singleton = match string(object, "scope", at)?.as_str() {
        "class" => true,
        "instance" | "" => false,
        other => {
            return Err(RubyExtractError::UnknownScope {
                at: at.to_owned(),
                scope: other.to_owned(),
            });
        }
    };
    let path = string(object, "path", at)?;
    let name = string(object, "name", at)?;
    if path.is_empty() || name.is_empty() {
        return Err(RubyExtractError::EmptyPathOrName { at: at.to_owned() });
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
        .map(documentation_ir::escape_id_segment)
        .collect();
    if object.kind == Type::Method && object.singleton {
        segments.push("self".to_owned());
    }
    segments.push(documentation_ir::escape_id_segment(name));
    segments.join(".")
}

/// Returns the qualified part of one namespace path's id, escaped.
fn namespace_qualified(path: &str) -> String {
    namespace_segments(path)
        .into_iter()
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn is_hidden_itself(object: &Object) -> bool {
    object.visibility != "public"
        || object
            .tags
            .iter()
            .any(|tag| tag.kind == "private" || (tag.kind == "api" && tag.text.trim() == "private"))
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
    if name.is_empty() { None } else { Some(name) }
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
            if object.singleton
                && !signature.starts_with("def self.")
                && let Some(rest) = signature.strip_prefix("def ")
            {
                signature = format!("def self.{rest}");
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

fn params_and_incomplete(draft: &Draft<'_>, incomplete: &mut Vec<String>) -> Vec<Param> {
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
            .find(|tag| tag.kind == "param" && tag.name == name);
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

fn returns_and_incomplete(draft: &Draft<'_>, incomplete: &mut Vec<String>) -> Option<Returns> {
    let object = draft.object;
    let callable = matches!(draft.kind, SymbolKind::Method | SymbolKind::Property);
    if !callable {
        return None;
    }
    let tag = object.tags.iter().find(|tag| tag.kind == "return");
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

fn parse_document(root: &Map<String, Value>) -> Result<(String, Vec<Object>), RubyExtractError> {
    let at = "YARD JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        found => return Err(RubyExtractError::UnsupportedFormat { found }),
    }
    let config = format!(
        "ruby={};yard={};format={FORMAT}",
        string(root, "ruby", at)?,
        string(root, "yard", at)?
    );
    let objects: Vec<Object> = field(root, "objects", at)?
        .as_array()
        .ok_or_else(|| RubyExtractError::NotList {
            at: at.to_owned(),
            key: "objects",
        })?
        .iter()
        .enumerate()
        .map(|(index, value)| object(value, &format!("YARD JSON object {index}")))
        .collect::<Result<_, _>>()?;
    Ok((config, objects))
}

fn hidden_namespaces_of(objects: &[Object]) -> BTreeSet<&str> {
    objects
        .iter()
        .filter(|object| matches!(object.kind, Type::Module | Type::Class))
        .filter(|object| is_hidden_itself(object))
        .map(|object| object.path.as_str())
        .collect()
}

fn is_hidden(object: &Object, hidden_namespaces: &BTreeSet<&str>) -> bool {
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
}

/// Converts one driver JSON document into a validated IR shard of one package.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn convert(json: &str, package: &str) -> Result<DocIr, RubyExtractError> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(RubyExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let root: Value = serde_json::from_str(json).map_err(RubyExtractError::Malformed)?;
    let root = root.as_object().ok_or_else(|| not_object("YARD JSON"))?;
    let (config, objects) = parse_document(root)?;

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
    let hidden_namespaces = hidden_namespaces_of(&objects);
    let hidden = |object: &Object| is_hidden(object, &hidden_namespaces);

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
        return Err(RubyExtractError::NoPublicApi {
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(RubyExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
