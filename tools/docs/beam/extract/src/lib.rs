//! Converts the Elixir and Erlang doc drivers' JSON into one validated `DocIr` shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::fmt;
use std::path::Path;

use documentation_ir::json::{JsonError, array, number, string, strings};
use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::Value;

const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "beam.config";
const KEY_INCOMPLETE: &str = "beam.incomplete";

/// Why one BEAM driver JSON document could not become a shard.
#[derive(Debug)]
pub enum BeamExtractError {
    Read(PathIoError),
    InvalidPackage { package: String },
    Malformed(serde_json::Error),
    NotObject { at: String },
    Json(JsonError),
    UnsupportedFormat { found: u64 },
    UnknownLanguage { language: String },
    UnknownKind { at: String, kind: String },
    EmptyName { at: String },
    NoPublicApi { language: String, package: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for BeamExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed(error) => write!(f, "driver JSON is malformed: {error}"),
            Self::NotObject { at } => write!(f, "{at} is not an object"),
            Self::Json(error) => error.fmt(f),
            Self::UnsupportedFormat { found } => write!(
                f,
                "driver JSON format {found} is not supported, expected {FORMAT}"
            ),
            Self::UnknownLanguage { language } => {
                write!(f, "driver JSON has unknown language '{language}'")
            }
            Self::UnknownKind { at, kind } => write!(f, "{at} has unknown kind '{kind}'"),
            Self::EmptyName { at } => write!(f, "{at} has an empty module or name"),
            Self::NoPublicApi { language, package } => write!(
                f,
                "driver JSON lists no public {language} API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BeamExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NotObject { .. }
            | Self::UnsupportedFormat { .. }
            | Self::UnknownLanguage { .. }
            | Self::UnknownKind { .. }
            | Self::EmptyName { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<JsonError> for BeamExtractError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// Runs `beam_extract` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("beam_extract", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: beam_extract --json FILE --package NAME --out FILE"
}

/// Parses argv into inputs.
///
/// # Errors
///
/// Returns an error for an unknown, missing, empty or repeated flag.
pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(args, &["--json", "--package", "--out"])?;
    Ok(Options {
        json: flags.one("--json")?,
        package: flags.one("--package")?,
        out: flags.one("--out")?,
    })
}

/// Reads, converts, validates and writes one shard.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn run(options: &Options) -> Result<(), BeamExtractError> {
    let json = path_io::read_to_string(Path::new(&options.json)).map_err(BeamExtractError::Read)?;
    let shard = convert(&json, &options.package)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(BeamExtractError::Write)
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

fn kind_of(kind: &str, at: &str) -> Result<SymbolKind, BeamExtractError> {
    Ok(match kind {
        "module" => SymbolKind::Module,
        "function" => SymbolKind::Function,
        "macro" => SymbolKind::Macro,
        "type" => SymbolKind::TypeAlias,
        "callback" => SymbolKind::Method,
        other => {
            return Err(BeamExtractError::UnknownKind {
                at: at.to_owned(),
                kind: other.to_owned(),
            });
        }
    })
}

fn module_id(prefix: &str, module: &str) -> String {
    let segments: Vec<String> = module
        .split('.')
        .map(documentation_ir::escape_id_segment)
        .collect();
    format!("{prefix}{}", segments.join("."))
}

fn symbol(
    index: usize,
    value: &Value,
    prefix: &str,
    config: &str,
) -> Result<Symbol, BeamExtractError> {
    let at = format!("driver JSON symbol {index}");
    let item = value
        .as_object()
        .ok_or_else(|| BeamExtractError::NotObject { at: at.clone() })?;
    let kind_name = string(item, "kind", &at)?;
    let kind = kind_of(&kind_name, &at)?;
    let module = string(item, "module", &at)?;
    let name = string(item, "name", &at)?;
    if module.is_empty() || (kind != SymbolKind::Module && name.is_empty()) {
        return Err(BeamExtractError::EmptyName { at });
    }
    let arity = number(item, "arity", &at)?;
    let owner = module_id(prefix, &module);
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
    let mut extensions = vec![extension(KEY_CONFIG, config)];
    if !incomplete.is_empty() {
        extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
    }
    Ok(Symbol {
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
    })
}

/// Converts one driver JSON document into a validated IR shard of one package.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn convert(json: &str, package: &str) -> Result<DocIr, BeamExtractError> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(BeamExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let root: Value = serde_json::from_str(json).map_err(BeamExtractError::Malformed)?;
    let at = "driver JSON";
    let root = root
        .as_object()
        .ok_or_else(|| BeamExtractError::NotObject { at: at.to_owned() })?;
    match number(root, "format", at)? {
        FORMAT => {}
        found => return Err(BeamExtractError::UnsupportedFormat { found }),
    }
    let language = string(root, "language", at)?;
    if language != "elixir" && language != "erlang" {
        return Err(BeamExtractError::UnknownLanguage { language });
    }
    let config = format!(
        "language={language};tool={};version={};format={FORMAT}",
        string(root, "tool", at)?,
        string(root, "version", at)?
    );
    let prefix = format!("{language}:{package}:");
    let items = array(root, "symbols", at)?;
    let mut symbols = Vec::new();
    for (index, value) in items.iter().enumerate() {
        symbols.push(symbol(index, value, &prefix, &config)?);
    }
    if symbols.is_empty() {
        return Err(BeamExtractError::NoPublicApi {
            language,
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language,
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(BeamExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
