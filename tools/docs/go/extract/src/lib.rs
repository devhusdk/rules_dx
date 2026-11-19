//! Converts the Go doc driver's JSON into one validated `DocIr` shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::fmt;
use std::path::Path;

use documentation_ir::json::{JsonError, array, boolean, number, string, strings};
use documentation_ir::proto::{
    DocIr, Extension, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::{Map, Value};

const LANGUAGE: &str = "go";
const SCHEMA: u64 = 1;
const KEY_CONFIG: &str = "go.config";
const KEY_EXCLUDED: &str = "go.excluded";

/// Why one Go doc JSON document could not become a shard.
#[derive(Debug)]
pub enum GoExtractError {
    Read(PathIoError),
    Malformed(serde_json::Error),
    Json(JsonError),
    NotObject { at: String },
    UnsupportedSchema { found: u64 },
    InvalidImportpath { importpath: String },
    NoSymbols,
    UnknownKind { at: String, kind: String },
    UnknownTypeKind { at: String, type_kind: String },
    EmptyName { at: String },
    ReceiverMismatch { at: String, name: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for GoExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::Malformed(error) => write!(f, "Go doc JSON is malformed: {error}"),
            Self::Json(error) => error.fmt(f),
            Self::NotObject { at } => write!(f, "{at} is not an object"),
            Self::UnsupportedSchema { found } => write!(
                f,
                "Go doc JSON schema {found} is not supported, expected {SCHEMA}"
            ),
            Self::InvalidImportpath { importpath } => {
                write!(f, "Go doc JSON has an invalid importpath '{importpath}'")
            }
            Self::NoSymbols => f.write_str("Go doc JSON lists no exported symbols"),
            Self::UnknownKind { at, kind } => write!(f, "{at} has unknown kind '{kind}'"),
            Self::UnknownTypeKind { at, type_kind } => {
                write!(f, "{at} has unknown type_kind '{type_kind}'")
            }
            Self::EmptyName { at } => write!(f, "{at} has an empty name"),
            Self::ReceiverMismatch { at, name } => write!(
                f,
                "{at} '{name}' has a receiver that does not match its kind"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for GoExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::NotObject { .. }
            | Self::UnsupportedSchema { .. }
            | Self::InvalidImportpath { .. }
            | Self::NoSymbols
            | Self::UnknownKind { .. }
            | Self::UnknownTypeKind { .. }
            | Self::EmptyName { .. }
            | Self::ReceiverMismatch { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<JsonError> for GoExtractError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub out: String,
}

/// Runs `go_extract` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("go_extract", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: go_extract --json FILE --out FILE"
}

/// Parses argv into inputs.
///
/// # Errors
///
/// Returns an error for an unknown, missing, empty or repeated flag.
pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(args, &["--json", "--out"])?;
    Ok(Options {
        json: flags.one("--json")?,
        out: flags.one("--out")?,
    })
}

/// Reads, converts, validates and writes one shard.
///
/// # Errors
///
/// Returns an error when the input cannot be read or converted or the shard cannot be written.
pub fn run(options: &Options) -> Result<(), GoExtractError> {
    let json = path_io::read_to_string(Path::new(&options.json)).map_err(GoExtractError::Read)?;
    let shard = convert(&json)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(GoExtractError::Write)
}

fn source(object: &Map<String, Value>, at: &str) -> Result<SourceRef, JsonError> {
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

fn kind_of(kind: &str, type_kind: &str, at: &str) -> Result<SymbolKind, GoExtractError> {
    Ok(match (kind, type_kind) {
        ("func", _) => SymbolKind::Function,
        ("method", _) => SymbolKind::Method,
        ("field", _) => SymbolKind::Field,
        ("const", _) => SymbolKind::Constant,
        ("var", _) => SymbolKind::Variable,
        ("type", "interface") => SymbolKind::Interface,
        ("type", "alias") => SymbolKind::TypeAlias,
        ("type", "struct" | "other") => SymbolKind::Class,
        ("type", other) => {
            return Err(GoExtractError::UnknownTypeKind {
                at: at.to_owned(),
                type_kind: other.to_owned(),
            });
        }
        (other, _) => {
            return Err(GoExtractError::UnknownKind {
                at: at.to_owned(),
                kind: other.to_owned(),
            });
        }
    })
}

fn item_symbol(
    index: usize,
    item: &Value,
    prefix: &str,
    config: &str,
) -> Result<Symbol, GoExtractError> {
    let at = format!("Go doc JSON item {index}");
    let Some(item) = item.as_object() else {
        return Err(GoExtractError::NotObject { at });
    };
    let name = string(item, "name", &at)?;
    let recv = string(item, "recv", &at)?;
    let kind = kind_of(
        &string(item, "kind", &at)?,
        &string(item, "type_kind", &at)?,
        &at,
    )?;
    if name.is_empty() {
        return Err(GoExtractError::EmptyName { at });
    }
    let member = matches!(kind, SymbolKind::Method | SymbolKind::Field);
    if member == recv.is_empty() {
        return Err(GoExtractError::ReceiverMismatch { at, name });
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
    Ok(Symbol {
        id: format!("{prefix}{qualified}"),
        kind: kind as i32,
        signature_text: string(item, "signature", &at)?,
        doc_markdown: string(item, "doc", &at)?,
        source: Some(source(item, &at)?),
        visibility: Visibility::Public as i32,
        relations,
        extensions: vec![extension(KEY_CONFIG, config)],
        ..Symbol::default()
    })
}

/// Converts one driver JSON document into a validated IR shard.
///
/// # Errors
///
/// Returns an error when the document is malformed, has an unsupported schema or shape, or yields invalid IR.
pub fn convert(json: &str) -> Result<DocIr, GoExtractError> {
    let at = "Go doc JSON";
    let root: Value = serde_json::from_str(json).map_err(GoExtractError::Malformed)?;
    let Some(root) = root.as_object() else {
        return Err(GoExtractError::NotObject { at: at.to_owned() });
    };
    match number(root, "schema", at)? {
        SCHEMA => {}
        found => return Err(GoExtractError::UnsupportedSchema { found }),
    }
    let importpath = string(root, "importpath", at)?;
    if importpath.is_empty() || importpath.contains(':') {
        return Err(GoExtractError::InvalidImportpath { importpath });
    }
    let package = string(root, "package", at)?;
    let config = string(root, "config", at)?;
    let excluded = strings(root, "excluded", at)?;
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
    let items = array(root, "items", at)?;
    if items.is_empty() {
        return Err(GoExtractError::NoSymbols);
    }
    for (index, item) in items.iter().enumerate() {
        symbols.push(item_symbol(index, item, &prefix, &config)?);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: importpath,
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(GoExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
