//! Converts the JSON of `ldc2 -D -X` into one validated DocIr shard.
//!
//! The compiler JSON carries the ddoc comment, kind, protection and position of every
//! declaration, but only mangled types. The signature text is therefore cut from the source
//! at the declaration line, and the ddoc `Params:`, `Returns:` and `Examples:` sections fill in
//! the parameter docs, the return doc and the examples.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use documentation_ir::proto::{
    DocIr, Extension, Param, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;

const LANGUAGE: &str = "d";
const KEY_CONFIG: &str = "d.config";

/// One source file of the library: where the compiler read it, and where it lives in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub rel: String,
    pub generated: bool,
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub api: String,
    pub sources: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: d_extract --api FILE --sources FILE --out FILE --package NAME [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !["--api", "--sources", "--out", "--package", "--config"].contains(&flag) {
            return Err(format!("unknown argument '{flag}'"));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if values.insert(flag, value.as_str()).is_some() {
            return Err(format!("{flag} given more than once"));
        }
        index += 2;
    }
    let one = |name: &str| match values.get(name) {
        Some(value) if !value.is_empty() => Ok((*value).to_owned()),
        Some(_) => Err(format!("{name} must not be empty")),
        None => Err(format!("{name} is required")),
    };
    Ok(Options {
        api: one("--api")?,
        sources: one("--sources")?,
        out: one("--out")?,
        package: one("--package")?,
        config: values
            .get("--config")
            .map(|value| (*value).to_owned())
            .unwrap_or_default(),
    })
}

/// Parses a srcs manifest of `exec_path<TAB>rel<TAB>generated` lines.
pub fn parse_sources(text: &str) -> Result<Vec<Source>, String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.as_slice() {
                [exec_path, rel, generated] => Ok(Source {
                    exec_path: (*exec_path).to_owned(),
                    rel: (*rel).to_owned(),
                    generated: *generated == "1",
                }),
                _ => Err(format!("bad srcs manifest line '{line}'")),
            }
        })
        .collect()
}

/// Reads, converts, validates and writes one shard. Nothing is written on error.
pub fn run(options: &Options) -> Result<(), String> {
    let read = |path: &str| {
        fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))
    };
    let json = read(&options.api)?;
    let sources = parse_sources(&read(&options.sources)?)?;
    let mut texts = BTreeMap::new();
    for source in &sources {
        texts.insert(source.exec_path.clone(), read(&source.exec_path)?);
    }
    let shard = convert(&json, &sources, &texts, &options.package, &options.config)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

struct Context<'a> {
    package: &'a str,
    config: String,
    sources: &'a [Source],
    texts: &'a BTreeMap<String, String>,
}

/// Converts the compiler JSON into a validated IR shard. `texts` maps each exec path to its text.
pub fn convert(
    json: &str,
    sources: &[Source],
    texts: &BTreeMap<String, String>,
    package: &str,
    config: &str,
) -> Result<DocIr, String> {
    let root: Value = serde_json::from_str(json).map_err(|error| format!("bad JSON: {error}"))?;
    let modules = root
        .as_array()
        .ok_or_else(|| "compiler JSON is not an array".to_owned())?;
    let mut text = format!("ldc-json=1;package={package}");
    if !config.is_empty() {
        text.push_str(&format!(";config={config}"));
    }
    let context = Context {
        package,
        config: text,
        sources,
        texts,
    };
    let mut symbols = Vec::new();
    for module in modules.iter().filter(|m| text_of(m, "kind") == "module") {
        let file = text_of(module, "file");
        let Some(source) = sources.iter().find(|s| s.exec_path == file) else {
            return Err(format!("module file '{file}' is not in the srcs manifest"));
        };
        let name = text_of(module, "name");
        if name.is_empty() {
            return Err(format!("module in '{file}' has no name"));
        }
        let id = qualified("", name);
        symbols.push(symbol(
            &context,
            module,
            source,
            &id,
            SymbolKind::Module,
            name.to_owned(),
        ));
        members(&context, module, source, &id, false, &mut symbols);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for symbol in &mut symbols {
        let count = seen.entry(symbol.id.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            symbol.id = format!("{}@{}", symbol.id, count);
        }
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

fn text_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn qualified(parent: &str, name: &str) -> String {
    let escaped = name
        .split('.')
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".");
    if parent.is_empty() {
        escaped
    } else {
        format!("{parent}.{escaped}")
    }
}

fn is_public(value: &Value) -> bool {
    matches!(text_of(value, "protection"), "" | "public")
}

fn storage(value: &Value, class: &str) -> bool {
    value
        .get("storageClass")
        .and_then(Value::as_array)
        .is_some_and(|all| all.iter().any(|item| item.as_str() == Some(class)))
}

fn members(
    context: &Context<'_>,
    parent: &Value,
    source: &Source,
    parent_id: &str,
    in_aggregate: bool,
    out: &mut Vec<Symbol>,
) {
    let Some(children) = parent.get("members").and_then(Value::as_array) else {
        return;
    };
    for child in children.iter().filter(|child| is_public(child)) {
        let name = text_of(child, "name");
        if name.is_empty() {
            continue;
        }
        let kind = match text_of(child, "kind") {
            "function" if in_aggregate => SymbolKind::Method,
            "function" => SymbolKind::Function,
            "constructor" => SymbolKind::Method,
            "class" | "struct" | "union" => SymbolKind::Class,
            "interface" => SymbolKind::Interface,
            "enum" => SymbolKind::Enum,
            "enum member" => SymbolKind::EnumVariant,
            "alias" => SymbolKind::TypeAlias,
            "variable" if storage(child, "enum") || storage(child, "immutable") => {
                SymbolKind::Constant
            }
            "variable" if in_aggregate => SymbolKind::Field,
            "variable" => SymbolKind::Variable,
            "template" => template_kind(child),
            _ => continue,
        };
        let id = qualified(parent_id, name);
        let aggregate = matches!(
            kind,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum
        );
        out.push(symbol(
            context,
            child,
            source,
            &id,
            kind,
            name.to_owned(),
        ));
        if aggregate {
            members(context, child, source, &id, true, out);
        } else if text_of(child, "kind") == "template" {
            for inner in child
                .get("members")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|inner| text_of(inner, "name") == name)
            {
                members(context, inner, source, &id, true, out);
            }
        }
    }
}

fn template_kind(template: &Value) -> SymbolKind {
    let inner = template
        .get("members")
        .and_then(Value::as_array)
        .and_then(|all| {
            all.iter()
                .find(|m| text_of(m, "name") == text_of(template, "name"))
        });
    match inner.map(|m| text_of(m, "kind")) {
        Some("function") => SymbolKind::Function,
        Some("class" | "struct" | "union") => SymbolKind::Class,
        Some("interface") => SymbolKind::Interface,
        _ => SymbolKind::Macro,
    }
}

fn symbol(
    context: &Context<'_>,
    node: &Value,
    source: &Source,
    id: &str,
    kind: SymbolKind,
    name: String,
) -> Symbol {
    let line = node.get("line").and_then(Value::as_u64).unwrap_or(1).max(1);
    let text = context
        .sources
        .iter()
        .find(|s| s.exec_path == source.exec_path)
        .and_then(|s| context.texts.get(&s.exec_path))
        .map_or("", String::as_str);
    let comment = text_of(node, "comment");
    let docs = Sections::parse(comment);
    let signature = match kind {
        SymbolKind::Module => format!("module {name};"),
        SymbolKind::EnumVariant => match node.get("value").and_then(Value::as_str) {
            Some(value) => format!("{name} = {value}"),
            None => name.clone(),
        },
        _ => declaration(text, line as usize),
    };
    let params = parameter_names(node)
        .into_iter()
        .map(|param| Param {
            r#type: parameter_type(&signature, &param),
            doc: docs.params.get(&param).cloned().unwrap_or_default(),
            name: param,
        })
        .collect();
    let returns = docs.returns.clone().map(|doc| Returns {
        r#type: String::new(),
        doc,
    });
    Symbol {
        id: format!("{LANGUAGE}:{}:{id}", context.package),
        kind: kind as i32,
        signature_text: signature,
        doc_markdown: docs.summary,
        params,
        returns,
        examples: docs.examples,
        source: Some(SourceRef {
            file: source.rel.clone(),
            line,
            generated: source.generated,
        }),
        visibility: Visibility::Public as i32,
        relations: None,
        extensions: vec![Extension {
            key: KEY_CONFIG.to_owned(),
            value: context.config.as_bytes().to_vec(),
        }],
        ..Symbol::default()
    }
}

fn parameter_names(node: &Value) -> Vec<String> {
    if !matches!(text_of(node, "kind"), "function" | "constructor") {
        return Vec::new();
    }
    node.get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|param| text_of(param, "name").to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// The declaration text from `line` to the first `{`, `;` or `=>` outside parentheses.
pub fn declaration(text: &str, line: usize) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut previous = ' ';
    'lines: for source_line in text.lines().skip(line.saturating_sub(1)).take(16) {
        for ch in source_line.chars() {
            match ch {
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                '{' | ';' if depth == 0 => break 'lines,
                '>' if depth == 0 && previous == '=' => {
                    out.pop();
                    break 'lines;
                }
                _ => {}
            }
            out.push(ch);
            previous = ch;
        }
        out.push(' ');
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The type of one parameter, read from the first parenthesis group of the signature.
fn parameter_type(signature: &str, name: &str) -> String {
    let Some(start) = signature.find('(') else {
        return String::new();
    };
    let mut depth = 0usize;
    let mut current = String::new();
    let mut parts: Vec<String> = Vec::new();
    for ch in signature[start + 1..].chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' if depth == 0 => break,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    parts.push(current);
    for part in parts {
        let declared = part.split('=').next().unwrap_or_default().trim();
        if let Some((kind, last)) = declared.rsplit_once(char::is_whitespace)
            && last == name
        {
            return kind.trim().to_owned();
        }
    }
    String::new()
}

/// The parts of one ddoc comment.
#[derive(Debug, Default)]
pub struct Sections {
    pub summary: String,
    pub params: BTreeMap<String, String>,
    pub returns: Option<String>,
    pub examples: Vec<String>,
}

impl Sections {
    /// Splits a ddoc comment at its `Params:`, `Returns:` and `Examples:` headings.
    pub fn parse(comment: &str) -> Sections {
        let comment = comment.replace("\r\n", "\n");
        let mut sections = Sections::default();
        let mut mode = "";
        let mut summary: Vec<&str> = Vec::new();
        let mut returns: Vec<&str> = Vec::new();
        let mut example: Vec<&str> = Vec::new();
        let mut last_param: Option<String> = None;
        for line in comment.lines() {
            if let Some(rest) = line.trim().strip_prefix("Returns:")
                && !rest.trim().is_empty()
            {
                mode = "returns";
                returns.push(rest.trim());
                continue;
            }
            let heading = line.trim().strip_suffix(':').map(str::trim);
            match heading {
                Some("Params") => {
                    mode = "params";
                    continue;
                }
                Some("Returns") => {
                    mode = "returns";
                    continue;
                }
                Some("Examples" | "Example") => {
                    mode = "examples";
                    continue;
                }
                _ => {}
            }
            match mode {
                "params" => {
                    if let Some((name, doc)) = line.split_once(" = ")
                        && !name.trim().is_empty()
                        && !name.trim().contains(char::is_whitespace)
                    {
                        last_param = Some(name.trim().to_owned());
                        sections
                            .params
                            .insert(name.trim().to_owned(), doc.trim().to_owned());
                    } else if let Some(name) = &last_param
                        && let Some(doc) = sections.params.get_mut(name)
                        && !line.trim().is_empty()
                    {
                        doc.push(' ');
                        doc.push_str(line.trim());
                    }
                }
                "returns" => returns.push(line),
                "examples" => example.push(line),
                _ => summary.push(line),
            }
        }
        sections.summary = summary.join("\n").trim().to_owned();
        let returns = returns.join("\n").trim().to_owned();
        if !returns.is_empty() {
            sections.returns = Some(returns);
        }
        let example = example.join("\n").trim().to_owned();
        if !example.is_empty() {
            sections.examples.push(format!("```d\n{example}\n```"));
        }
        sections
    }
}

#[cfg(test)]
mod lib_tests;
