#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use documentation_ir::json::{JsonError, array, field, number, string};
use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::{Map, Value};

const LANGUAGE: &str = "fortran";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "fortran.config";
const KEY_INCOMPLETE: &str = "fortran.incomplete";

#[derive(Debug)]
pub enum FortranExtractError {
    Read(PathIoError),
    InvalidPackage { package: String },
    Malformed(serde_json::Error),
    NotObject { at: String },
    Json(JsonError),
    UnsupportedFormat { found: u64 },
    UnknownKind { at: String, kind: String },
    ReturnsNotObject { at: String },
    EmptyPathOrName { at: String },
    RepeatedPath { at: String, path: String },
    NoPublicApi { package: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for FortranExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed(error) => write!(f, "FORD JSON is malformed: {error}"),
            Self::NotObject { at } => write!(f, "{at} is not an object"),
            Self::Json(error) => error.fmt(f),
            Self::UnsupportedFormat { found } => write!(
                f,
                "FORD JSON format {found} is not supported, expected {FORMAT}"
            ),
            Self::UnknownKind { at, kind } => write!(f, "{at} has unknown kind '{kind}'"),
            Self::ReturnsNotObject { at } => {
                write!(f, "{at} 'returns' is neither null nor an object")
            }
            Self::EmptyPathOrName { at } => write!(f, "{at} has an empty path or name"),
            Self::RepeatedPath { at, path } => write!(f, "{at} repeats the path '{path}'"),
            Self::NoPublicApi { package } => write!(
                f,
                "FORD JSON lists no public Fortran API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FortranExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NotObject { .. }
            | Self::UnsupportedFormat { .. }
            | Self::UnknownKind { .. }
            | Self::ReturnsNotObject { .. }
            | Self::EmptyPathOrName { .. }
            | Self::RepeatedPath { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

impl From<JsonError> for FortranExtractError {
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
    documentation_ir::tool::run_process("fortran_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: fortran_extract --json FILE --package NAME --out FILE"
}

pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(args, &["--json", "--package", "--out"])?;
    Ok(Options {
        json: flags.one("--json")?,
        package: flags.one("--package")?,
        out: flags.one("--out")?,
    })
}

pub fn run(options: &Options) -> Result<(), FortranExtractError> {
    let json =
        path_io::read_to_string(Path::new(&options.json)).map_err(FortranExtractError::Read)?;
    let shard = convert(&json, &options.package)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(FortranExtractError::Write)
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

fn symbol_id(package: &str, path: &str) -> String {
    let qualified: Vec<String> = path
        .split('.')
        .map(documentation_ir::escape_id_segment)
        .collect();
    format!("{LANGUAGE}:{package}:{}", qualified.join("."))
}

fn kind_of(kind: &str, signature: &str, at: &str) -> Result<SymbolKind, FortranExtractError> {
    Ok(match kind {
        "module" => SymbolKind::Module,
        "type" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "function" | "subroutine" => SymbolKind::Function,
        "binding" => SymbolKind::Method,
        "component" => SymbolKind::Field,
        "variable" if signature.contains(", parameter") => SymbolKind::Constant,
        "variable" => SymbolKind::Variable,
        other => {
            return Err(FortranExtractError::UnknownKind {
                at: at.to_owned(),
                kind: other.to_owned(),
            });
        }
    })
}

fn params(
    object: &Map<String, Value>,
    at: &str,
    incomplete: &mut Vec<String>,
) -> Result<Vec<Param>, FortranExtractError> {
    let list = array(object, "params", at)?;
    let mut undocumented = Vec::new();
    let mut out = Vec::new();
    for (index, value) in list.iter().enumerate() {
        let at = format!("{at} param {index}");
        let value = value
            .as_object()
            .ok_or_else(|| FortranExtractError::NotObject { at: at.clone() })?;
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

fn returns(
    object: &Map<String, Value>,
    at: &str,
    incomplete: &mut Vec<String>,
) -> Result<Option<Returns>, FortranExtractError> {
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
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            Err(FortranExtractError::ReturnsNotObject { at: at.to_owned() })
        }
    }
}

pub fn convert(json: &str, package: &str) -> Result<DocIr, FortranExtractError> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(FortranExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let root: Value = serde_json::from_str(json).map_err(FortranExtractError::Malformed)?;
    let at = "FORD JSON";
    let root = root
        .as_object()
        .ok_or_else(|| FortranExtractError::NotObject { at: at.to_owned() })?;
    match number(root, "format", at)? {
        FORMAT => {}
        found => return Err(FortranExtractError::UnsupportedFormat { found }),
    }
    let config = format!("ford={};format={FORMAT}", string(root, "ford", at)?);
    let objects = array(root, "objects", at)?;

    let mut paths = BTreeSet::new();
    let mut symbols = Vec::new();
    for (index, value) in objects.iter().enumerate() {
        let at = format!("FORD JSON object {index}");
        let object = value
            .as_object()
            .ok_or_else(|| FortranExtractError::NotObject { at: at.clone() })?;
        let path = string(object, "path", &at)?;
        let name = string(object, "name", &at)?;
        if path.is_empty() || name.is_empty() {
            return Err(FortranExtractError::EmptyPathOrName { at });
        }
        if !paths.insert(path.clone()) {
            return Err(FortranExtractError::RepeatedPath { at, path });
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
        return Err(FortranExtractError::NoPublicApi {
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(FortranExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
