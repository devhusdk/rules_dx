#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use documentation_ir::json::{JsonError, boolean, field, number, string, strings};
use documentation_ir::proto::{
    DocIr, Extension, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::Value;

const LANGUAGE: &str = "julia";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "julia.config";
const KEY_INCOMPLETE: &str = "julia.incomplete";

#[derive(Debug)]
pub enum JuliaExtractError {
    Read(PathIoError),
    InvalidPackage { package: String },
    Malformed(serde_json::Error),
    NotObject { at: String },
    ObjectsNotList,
    Json(JsonError),
    UnsupportedFormat { found: u64 },
    UnknownKind { at: String, kind: String },
    EmptyPathOrName { at: String },
    NoPublicApi { package: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for JuliaExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed(error) => write!(f, "Documenter JSON is malformed: {error}"),
            Self::NotObject { at } => write!(f, "{at} is not an object"),
            Self::ObjectsNotList => f.write_str("Documenter JSON 'objects' is not a list"),
            Self::Json(error) => error.fmt(f),
            Self::UnsupportedFormat { found } => write!(
                f,
                "Documenter JSON format {found} is not supported, expected {FORMAT}"
            ),
            Self::UnknownKind { at, kind } => write!(f, "{at} has unknown kind '{kind}'"),
            Self::EmptyPathOrName { at } => write!(f, "{at} has an empty path or name"),
            Self::NoPublicApi { package } => write!(
                f,
                "Documenter JSON lists no public Julia API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for JuliaExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NotObject { .. }
            | Self::ObjectsNotList
            | Self::UnsupportedFormat { .. }
            | Self::UnknownKind { .. }
            | Self::EmptyPathOrName { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<JsonError> for JuliaExtractError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("julia_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: julia_extract --json FILE --package NAME --out FILE"
}

pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(args, &["--json", "--package", "--out"])?;
    Ok(Options {
        json: flags.one("--json")?,
        package: flags.one("--package")?,
        out: flags.one("--out")?,
    })
}

pub fn run(options: &Options) -> Result<(), JuliaExtractError> {
    let json =
        path_io::read_to_string(Path::new(&options.json)).map_err(JuliaExtractError::Read)?;
    let shard = convert(&json, &options.package)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(JuliaExtractError::Write)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Object {
    path: String,
    name: String,
    module: String,
    kind: SymbolKind,
    signatures: Vec<String>,
    doc: String,
    file: String,
    generated: bool,
    line: u64,
}

fn kind_of(name: &str, at: &str) -> Result<SymbolKind, JuliaExtractError> {
    Ok(match name {
        "module" => SymbolKind::Module,
        "struct" | "mutable_struct" | "primitive" => SymbolKind::Class,
        "abstract" => SymbolKind::Interface,
        "function" => SymbolKind::Function,
        "macro" => SymbolKind::Macro,
        "constant" => SymbolKind::Constant,
        "alias" => SymbolKind::TypeAlias,
        "variable" => SymbolKind::Variable,
        other => {
            return Err(JuliaExtractError::UnknownKind {
                at: at.to_owned(),
                kind: other.to_owned(),
            });
        }
    })
}

fn object(value: &Value, at: &str) -> Result<Object, JuliaExtractError> {
    let object = value
        .as_object()
        .ok_or_else(|| JuliaExtractError::NotObject { at: at.to_owned() })?;
    let path = string(object, "path", at)?;
    let name = string(object, "name", at)?;
    if path.is_empty() || name.is_empty() {
        return Err(JuliaExtractError::EmptyPathOrName { at: at.to_owned() });
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

fn qualified(path: &str) -> String {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
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

fn symbol(object: &Object, modules: &BTreeSet<&str>, prefix: &str, config: &str) -> Symbol {
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
            SymbolKind::Unspecified
            | SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Function
            | SymbolKind::Method
            | SymbolKind::Field
            | SymbolKind::Property
            | SymbolKind::Constant
            | SymbolKind::Enum
            | SymbolKind::EnumVariant
            | SymbolKind::TypeAlias
            | SymbolKind::Macro
            | SymbolKind::Variable
            | SymbolKind::Component
            | SymbolKind::Event
            | SymbolKind::Slot => object.path.clone(),
        }
    } else {
        object.signatures.join("\n")
    };
    let mut extensions = vec![extension(KEY_CONFIG, config)];
    if !incomplete.is_empty() {
        extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
    }
    Symbol {
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
    }
}

pub fn convert(json: &str, package: &str) -> Result<DocIr, JuliaExtractError> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(JuliaExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let root: Value = serde_json::from_str(json).map_err(JuliaExtractError::Malformed)?;
    let at = "Documenter JSON";
    let root = root
        .as_object()
        .ok_or_else(|| JuliaExtractError::NotObject { at: at.to_owned() })?;
    match number(root, "format", at)? {
        FORMAT => {}
        found => return Err(JuliaExtractError::UnsupportedFormat { found }),
    }
    let config = format!(
        "julia={};documenter={};format={FORMAT}",
        string(root, "julia", at)?,
        string(root, "documenter", at)?
    );
    let objects: Vec<Object> = field(root, "objects", at)?
        .as_array()
        .ok_or(JuliaExtractError::ObjectsNotList)?
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
        symbols.push(symbol(object, &modules, &prefix, &config));
    }
    if symbols.is_empty() {
        return Err(JuliaExtractError::NoPublicApi {
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(JuliaExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
