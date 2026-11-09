//! Converts Avro schemas (.avsc, .avpr, or the schemas avro-tools writes for .avdl) into one
//! validated DocIr shard, reading the `doc` attribute of every named type, field and message.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::Value;

const LANGUAGE: &str = "avro";
const KEY_INCOMPLETE: &str = "avro.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub package: String,
    pub out: String,
    /// Schema file to workspace-relative path of the source it documents.
    pub schemas: Vec<(String, String)>,
    /// Directory of schemas avro-tools wrote, to workspace-relative path of the .avdl.
    pub schema_dirs: Vec<(String, String)>,
    /// Workspace-relative paths of sources that are build output.
    pub generated: Vec<String>,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: avro_extract --package NAME --out FILE (--schema FILE=SRC | --schema-dir DIR=SRC)... [--generated SRC]..."
}

fn pair(flag: &str, value: &str) -> Result<(String, String), String> {
    match value.split_once('=') {
        Some((left, right)) if !left.is_empty() && !right.is_empty() => {
            Ok((left.to_owned(), right.to_owned()))
        }
        _ => Err(format!("{flag} '{value}' is not INPUT=SRC")),
    }
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
        match flag {
            "--schema" => options.schemas.push(pair(flag, value)?),
            "--schema-dir" => options.schema_dirs.push(pair(flag, value)?),
            "--generated" => options.generated.push(value.clone()),
            "--package" | "--out" => {
                let slot = if flag == "--package" {
                    &mut options.package
                } else {
                    &mut options.out
                };
                if !slot.is_empty() {
                    return Err(format!("{flag} given more than once"));
                }
                *slot = value.clone();
            }
            _ => return Err(format!("unknown argument '{flag}'")),
        }
        index += 2;
    }
    for (flag, value) in [("--package", &options.package), ("--out", &options.out)] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    if options.schemas.is_empty() && options.schema_dirs.is_empty() {
        return Err("at least one --schema or --schema-dir is required".to_owned());
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let mut inputs: Vec<(String, String, String)> = Vec::new();
    for (file, source) in &options.schemas {
        let text =
            fs::read_to_string(file).map_err(|error| format!("cannot read {file}: {error}"))?;
        inputs.push((text, source.clone(), file.clone()));
    }
    for (dir, source) in &options.schema_dirs {
        let mut files: Vec<_> = fs::read_dir(dir)
            .map_err(|error| format!("cannot read {dir}: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "avsc" || ext == "avpr"))
            .collect();
        files.sort();
        for path in files {
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            inputs.push((text, source.clone(), path.display().to_string()));
        }
    }
    let shard = convert(&inputs, options)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(Path::new(&options.out), bytes)
        .map_err(|error| format!("cannot write {}: {error}", options.out))
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Splits a name into its namespace and simple name, inheriting `enclosing` when it has none.
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

/// How a type reads in a signature.
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
        _ => String::new(),
    }
}

struct Builder<'a> {
    package: &'a str,
    source: String,
    generated: bool,
    /// The lines of the schema text when they map to the source, else empty.
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

    /// The 1-based line of the first `"name": "<name>"` at or after `from`, or 0.
    fn find_line(&self, name: &str, from: u64) -> u64 {
        let quoted = format!("\"{name}\"");
        let start = from.saturating_sub(1) as usize;
        self.lines
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, line)| line.contains("\"name\"") && line.contains(&quoted))
            .map_or(0, |(index, _)| index as u64 + 1)
    }

    #[allow(clippy::too_many_arguments)]
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
        // A type imported by several files appears in each; keep the copy that has docs.
        match self.symbols.get(&id) {
            Some(existing) if !existing.doc_markdown.is_empty() || symbol.doc_markdown.is_empty() => {}
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
            _ => {}
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
                signature.push_str(&format!(" = {default}"));
            }
            let field_line = if line == 0 { 0 } else { self.find_line(name, line) };
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
            // Avro has no per-symbol doc attribute, so a symbol is documented by its enum.
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
        for ty in value.get("types").and_then(Value::as_array).into_iter().flatten() {
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
                signature.push_str(&format!(" throws {}", errors.join(", ")));
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
            for ty in message.get("request").and_then(Value::as_array).into_iter().flatten() {
                self.walk(ty.get("type").unwrap_or(&Value::Null), &namespace);
            }
        }
    }
}

/// Converts schema texts, each with the source it documents and its file name, into one shard.
pub fn convert(
    inputs: &[(String, String, String)],
    options: &Options,
) -> Result<DocIr, String> {
    let package = options.package.as_str();
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let mut symbols: BTreeMap<String, Symbol> = BTreeMap::new();
    for (text, source, file) in inputs {
        let value: Value = serde_json::from_str(text)
            .map_err(|error| format!("{file} is not valid JSON: {error}"))?;
        // Line numbers only mean something when the schema text is the source itself.
        let own_text = file == source || source.ends_with(".avsc") || source.ends_with(".avpr");
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
        return Err(format!(
            "the schemas declare no named Avro type of package '{package}'"
        ));
    }
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols: symbols.into_values().collect(),
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
