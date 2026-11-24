#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::path::Path;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::Value;

const LANGUAGE: &str = "avro";
const KEY_INCOMPLETE: &str = "avro.incomplete";

#[derive(Debug)]
pub enum ArgsError {
    Flag(FlagError),
    NotPair { flag: String, value: String },
    NoSchema,
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::NotPair { flag, value } => write!(f, "{flag} '{value}' is not INPUT=SRC"),
            Self::NoSchema => f.write_str("at least one --schema or --schema-dir is required"),
        }
    }
}

impl std::error::Error for ArgsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Flag(error) => Some(error),
            Self::NotPair { .. } | Self::NoSchema => None,
        }
    }
}

impl From<FlagError> for ArgsError {
    fn from(error: FlagError) -> Self {
        Self::Flag(error)
    }
}

#[derive(Debug)]
pub enum AvroExtractError {
    Read(PathIoError),
    InvalidPackage {
        package: String,
    },
    InvalidJson {
        file: String,
        source: serde_json::Error,
    },
    NoNamedType {
        package: String,
    },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for AvroExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::InvalidJson { file, source } => write!(f, "{file} is not valid JSON: {source}"),
            Self::NoNamedType { package } => write!(
                f,
                "the schemas declare no named Avro type of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for AvroExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::InvalidJson { source, .. } => Some(source),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. } | Self::NoNamedType { .. } | Self::InvalidIr(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub package: String,
    pub out: String,
    pub schemas: Vec<(String, String)>,
    pub schema_dirs: Vec<(String, String)>,
    pub generated: Vec<String>,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("avro_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: avro_extract --package NAME --out FILE (--schema FILE=SRC | --schema-dir DIR=SRC)... [--generated SRC]..."
}

fn pair(flag: &str, value: &str) -> Result<(String, String), ArgsError> {
    match value.split_once('=') {
        Some((left, right)) if !left.is_empty() && !right.is_empty() => {
            Ok((left.to_owned(), right.to_owned()))
        }
        _ => Err(ArgsError::NotPair {
            flag: flag.to_owned(),
            value: value.to_owned(),
        }),
    }
}

pub fn parse(args: &[String]) -> Result<Options, ArgsError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--schema",
            "--schema-dir",
            "--generated",
            "--package",
            "--out",
        ],
    )?;
    let options = Options {
        package: flags.one("--package")?,
        out: flags.one("--out")?,
        schemas: flags
            .many("--schema")
            .iter()
            .map(|value| pair("--schema", value))
            .collect::<Result<_, _>>()?,
        schema_dirs: flags
            .many("--schema-dir")
            .iter()
            .map(|value| pair("--schema-dir", value))
            .collect::<Result<_, _>>()?,
        generated: flags.many("--generated"),
    };
    if options.schemas.is_empty() && options.schema_dirs.is_empty() {
        return Err(ArgsError::NoSchema);
    }
    Ok(options)
}

pub fn run(options: &Options) -> Result<(), AvroExtractError> {
    let mut inputs: Vec<(String, String, String)> = Vec::new();
    for (file, source) in &options.schemas {
        let text = path_io::read_to_string(Path::new(file)).map_err(AvroExtractError::Read)?;
        inputs.push((text, source.clone(), file.clone()));
    }
    for (dir, source) in &options.schema_dirs {
        let files = path_io::read_dir(Path::new(dir)).map_err(AvroExtractError::Read)?;
        for path in files.iter().filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "avsc" || ext == "avpr")
        }) {
            let text = path_io::read_to_string(path).map_err(AvroExtractError::Read)?;
            inputs.push((text, source.clone(), path.display().to_string()));
        }
    }
    let shard = convert(&inputs, options)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(AvroExtractError::Write)
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn split_name(value: &Value, enclosing: &str) -> (String, String) {
    let name = str_of(value, "name");
    if let Some((namespace, simple)) = name.rsplit_once('.') {
        return (namespace.to_owned(), simple.to_owned());
    }
    match value.get("namespace").and_then(Value::as_str) {
        Some(namespace) => (namespace.to_owned(), name.to_owned()),
        None => (enclosing.to_owned(), name.to_owned()),
    }
}

fn full_name(namespace: &str, simple: &str) -> String {
    if namespace.is_empty() {
        simple.to_owned()
    } else {
        format!("{namespace}.{simple}")
    }
}

fn type_text(value: &Value, enclosing: &str) -> String {
    match value {
        Value::String(name) => name.clone(),
        Value::Array(members) => members
            .iter()
            .map(|member| type_text(member, enclosing))
            .collect::<Vec<_>>()
            .join(" | "),
        Value::Object(_) => match str_of(value, "type") {
            "array" => format!(
                "array<{}>",
                type_text(value.get("items").unwrap_or(&Value::Null), enclosing)
            ),
            "map" => format!(
                "map<string, {}>",
                type_text(value.get("values").unwrap_or(&Value::Null), enclosing)
            ),
            "record" | "error" | "enum" | "fixed" => {
                let (namespace, simple) = split_name(value, enclosing);
                full_name(&namespace, &simple)
            }
            "" => value
                .get("type")
                .map_or_else(String::new, |inner| type_text(inner, enclosing)),
            primitive => primitive.to_owned(),
        },
        Value::Null | Value::Bool(_) | Value::Number(_) => String::new(),
    }
}

struct Builder<'a> {
    package: &'a str,
    source: String,
    generated: bool,
    lines: Vec<String>,
    symbols: BTreeMap<String, Symbol>,
}

impl Builder<'_> {
    fn id_of(&self, namespace: &str, simple: &str, member: Option<&str>) -> String {
        let mut segments: Vec<&str> = namespace.split('.').filter(|s| !s.is_empty()).collect();
        segments.push(simple);
        if let Some(member) = member {
            segments.push(member);
        }
        format!(
            "{LANGUAGE}:{}:{}",
            self.package,
            segments
                .iter()
                .map(|s| documentation_ir::escape_id_segment(s))
                .collect::<Vec<_>>()
                .join(".")
        )
    }

    fn find_line(&self, name: &str, from: u64) -> u64 {
        let quoted = format!("\"{name}\"");
        let start = usize::try_from(from.saturating_sub(1)).unwrap_or(usize::MAX);
        self.lines
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, line)| line.contains("\"name\"") && line.contains(&quoted))
            .map_or(0, |(index, _)| index as u64 + 1)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct input of the call"
    )]
    fn push(
        &mut self,
        id: String,
        display: String,
        kind: SymbolKind,
        signature: String,
        doc: &str,
        line: u64,
        parent: Option<String>,
        params: Vec<Param>,
        returns: Option<Returns>,
    ) {
        let doc = doc.trim().to_owned();
        let mut extensions = Vec::new();
        if doc.is_empty() {
            extensions.push(Extension {
                key: KEY_INCOMPLETE.to_owned(),
                value: b"doc".to_vec(),
            });
        }
        let symbol = Symbol {
            id: id.clone(),
            kind: kind as i32,
            signature_text: signature,
            doc_markdown: doc,
            params,
            returns,
            source: Some(SourceRef {
                file: self.source.clone(),
                line,
                generated: self.generated,
            }),
            visibility: Visibility::Public as i32,
            relations: parent.map(|parent| Relations {
                member_of: vec![parent],
                implements: Vec::new(),
                reexported_as: Vec::new(),
            }),
            extensions,
            display_name: display,
            ..Symbol::default()
        };
        match self.symbols.get(&id) {
            Some(existing)
                if !existing.doc_markdown.is_empty() || symbol.doc_markdown.is_empty() => {}
            _ => {
                self.symbols.insert(id, symbol);
            }
        }
    }

    fn walk(&mut self, value: &Value, enclosing: &str) {
        match value {
            Value::Array(members) => {
                for member in members {
                    self.walk(member, enclosing);
                }
            }
            Value::Object(_) => {
                if value.get("protocol").is_some() {
                    self.protocol(value, enclosing);
                    return;
                }
                match str_of(value, "type") {
                    "record" | "error" => self.record(value, enclosing),
                    "enum" => self.enumeration(value, enclosing),
                    "fixed" => self.fixed(value, enclosing),
                    "array" => self.walk(value.get("items").unwrap_or(&Value::Null), enclosing),
                    "map" => self.walk(value.get("values").unwrap_or(&Value::Null), enclosing),
                    "" => self.walk(value.get("type").unwrap_or(&Value::Null), enclosing),
                    _ => {}
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    fn record(&mut self, value: &Value, enclosing: &str) {
        let (namespace, simple) = split_name(value, enclosing);
        if simple.is_empty() {
            return;
        }
        let id = self.id_of(&namespace, &simple, None);
        let line = self.find_line(&simple, 1);
        let keyword = if str_of(value, "type") == "error" {
            "error"
        } else {
            "record"
        };
        self.push(
            id.clone(),
            full_name(&namespace, &simple),
            SymbolKind::Class,
            format!("{keyword} {simple}"),
            str_of(value, "doc"),
            line,
            None,
            Vec::new(),
            None,
        );
        let fields = value.get("fields").and_then(Value::as_array);
        for field in fields.into_iter().flatten() {
            let name = str_of(field, "name");
            if name.is_empty() {
                continue;
            }
            let ty = field.get("type").unwrap_or(&Value::Null);
            let mut signature = format!("{} {name}", type_text(ty, &namespace));
            if let Some(default) = field.get("default") {
                write!(signature, " = {default}").unwrap_or_default();
            }
            let field_line = if line == 0 {
                0
            } else {
                self.find_line(name, line)
            };
            self.push(
                self.id_of(&namespace, &simple, Some(name)),
                format!("{}.{name}", full_name(&namespace, &simple)),
                SymbolKind::Field,
                signature,
                str_of(field, "doc"),
                field_line,
                Some(id.clone()),
                Vec::new(),
                None,
            );
            self.walk(ty, &namespace);
        }
    }

    fn enumeration(&mut self, value: &Value, enclosing: &str) {
        let (namespace, simple) = split_name(value, enclosing);
        if simple.is_empty() {
            return;
        }
        let id = self.id_of(&namespace, &simple, None);
        let line = self.find_line(&simple, 1);
        self.push(
            id.clone(),
            full_name(&namespace, &simple),
            SymbolKind::Enum,
            format!("enum {simple}"),
            str_of(value, "doc"),
            line,
            None,
            Vec::new(),
            None,
        );
        let symbols = value.get("symbols").and_then(Value::as_array);
        for symbol in symbols.into_iter().flatten().filter_map(Value::as_str) {
            self.push(
                self.id_of(&namespace, &simple, Some(symbol)),
                format!("{}.{symbol}", full_name(&namespace, &simple)),
                SymbolKind::EnumVariant,
                symbol.to_owned(),
                "",
                line,
                Some(id.clone()),
                Vec::new(),
                None,
            );
        }
    }

    fn fixed(&mut self, value: &Value, enclosing: &str) {
        let (namespace, simple) = split_name(value, enclosing);
        if simple.is_empty() {
            return;
        }
        let size = value.get("size").and_then(Value::as_u64).unwrap_or(0);
        let line = self.find_line(&simple, 1);
        self.push(
            self.id_of(&namespace, &simple, None),
            full_name(&namespace, &simple),
            SymbolKind::TypeAlias,
            format!("fixed {simple}({size})"),
            str_of(value, "doc"),
            line,
            None,
            Vec::new(),
            None,
        );
    }

    fn protocol(&mut self, value: &Value, enclosing: &str) {
        let name = str_of(value, "protocol");
        let namespace = value
            .get("namespace")
            .and_then(Value::as_str)
            .unwrap_or(enclosing)
            .to_owned();
        let (namespace, simple) = match name.rsplit_once('.') {
            Some((ns, simple)) => (ns.to_owned(), simple.to_owned()),
            None => (namespace, name.to_owned()),
        };
        if simple.is_empty() {
            return;
        }
        let id = self.id_of(&namespace, &simple, None);
        self.push(
            id.clone(),
            full_name(&namespace, &simple),
            SymbolKind::Interface,
            format!("protocol {simple}"),
            str_of(value, "doc"),
            0,
            None,
            Vec::new(),
            None,
        );
        for ty in value
            .get("types")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.walk(ty, &namespace);
        }
        let messages = value.get("messages").and_then(Value::as_object);
        for (message_name, message) in messages.into_iter().flatten() {
            let request = message.get("request").and_then(Value::as_array);
            let params: Vec<Param> = request
                .into_iter()
                .flatten()
                .map(|param| Param {
                    name: str_of(param, "name").to_owned(),
                    r#type: type_text(param.get("type").unwrap_or(&Value::Null), &namespace),
                    doc: str_of(param, "doc").to_owned(),
                })
                .collect();
            let response = type_text(message.get("response").unwrap_or(&Value::Null), &namespace);
            let mut signature = format!(
                "{message_name}({}) returns {response}",
                params
                    .iter()
                    .map(|p| format!("{} {}", p.r#type, p.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let errors: Vec<String> = message
                .get("errors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|error| type_text(error, &namespace))
                .filter(|text| text != "string")
                .collect();
            if !errors.is_empty() {
                write!(signature, " throws {}", errors.join(", ")).unwrap_or_default();
            }
            if message.get("one-way").and_then(Value::as_bool) == Some(true) {
                signature.push_str(" (one-way)");
            }
            self.push(
                self.id_of(&namespace, &simple, Some(message_name)),
                format!("{}.{message_name}", full_name(&namespace, &simple)),
                SymbolKind::Method,
                signature,
                str_of(message, "doc"),
                0,
                Some(id.clone()),
                params,
                Some(Returns {
                    r#type: response,
                    doc: String::new(),
                }),
            );
            for ty in message
                .get("request")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                self.walk(ty.get("type").unwrap_or(&Value::Null), &namespace);
            }
        }
    }
}

pub fn convert(
    inputs: &[(String, String, String)],
    options: &Options,
) -> Result<DocIr, AvroExtractError> {
    let package = options.package.as_str();
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(AvroExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let mut symbols: BTreeMap<String, Symbol> = BTreeMap::new();
    for (text, source, file) in inputs {
        let value: Value =
            serde_json::from_str(text).map_err(|source| AvroExtractError::InvalidJson {
                file: file.clone(),
                source,
            })?;
        let own_text = file == source
            || Path::new(source)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("avsc"))
            || Path::new(source)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("avpr"));
        let mut builder = Builder {
            package,
            source: source.clone(),
            generated: options.generated.contains(source),
            lines: if own_text {
                text.lines().map(str::to_owned).collect()
            } else {
                Vec::new()
            },
            symbols: BTreeMap::new(),
        };
        builder.walk(&value, "");
        for (id, symbol) in builder.symbols {
            match symbols.get(&id) {
                Some(existing)
                    if !existing.doc_markdown.is_empty() || symbol.doc_markdown.is_empty() => {}
                _ => {
                    symbols.insert(id, symbol);
                }
            }
        }
    }
    if symbols.is_empty() {
        return Err(AvroExtractError::NoNamedType {
            package: package.to_owned(),
        });
    }
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols: symbols.into_values().collect(),
    };
    documentation_ir::validate_shard(&shard).map_err(AvroExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
