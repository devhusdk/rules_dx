//! Converts the JSON of `zig_doc_dump` into one validated DocIr shard.
//!
//! The dump (schema 1) carries, per source file, the `//!` file doc and every declaration with
//! its `///` doc, `pub` flag, kind, line and exact signature text. Only what the main file (the
//! first dumped file) exposes is documented: its `pub` declarations, and the library's own
//! files it re-exports with `pub const X = @import("own.zig")`, which become modules.

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
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const LANGUAGE: &str = "zig";
const KEY_CONFIG: &str = "zig.config";
const SCHEMA: u64 = 1;

/// One source file of the library: where the dump read it, and where it lives in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub rel: String,
    pub generated: bool,
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub dump: String,
    pub sources: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: zig_extract --dump FILE --sources FILE --out FILE --package NAME [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !["--dump", "--sources", "--out", "--package", "--config"].contains(&flag) {
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
        dump: one("--dump")?,
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
    let json = read(&options.dump)?;
    let sources = parse_sources(&read(&options.sources)?)?;
    let shard = convert(&json, &sources, &options.package, &options.config)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

struct Context<'a> {
    package: &'a str,
    config: String,
    sources: &'a [Source],
    files: &'a [Value],
}

/// Converts the dump JSON into a validated IR shard.
pub fn convert(
    json: &str,
    sources: &[Source],
    package: &str,
    config: &str,
) -> Result<DocIr, String> {
    let root: Value = serde_json::from_str(json).map_err(|error| format!("bad JSON: {error}"))?;
    if root.get("schema").and_then(Value::as_u64) != Some(SCHEMA) {
        return Err(format!("dump schema is not {SCHEMA}"));
    }
    let files = root
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "dump has no files".to_owned())?;
    let Some(main) = files.first() else {
        return Err("dump has no files".to_owned());
    };
    let mut text = format!("zig-ast={SCHEMA};package={package}");
    if !config.is_empty() {
        text.push_str(&format!(";config={config}"));
    }
    let context = Context {
        package,
        config: text,
        sources,
        files,
    };
    let mut symbols = Vec::new();
    let mut visited = BTreeSet::new();
    module(
        &context,
        main,
        &documentation_ir::escape_id_segment(package),
        &mut visited,
        &mut symbols,
    )?;
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

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Resolves `target` against the directory of `from`, folding `.` and `..` segments.
pub fn resolve(from: &str, target: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn normalized(path: &str) -> String {
    resolve("x/", path)
}

/// Emits the module symbol of `file` under `id`, then its public members and re-exported modules.
fn module(
    context: &Context<'_>,
    file: &Value,
    id: &str,
    visited: &mut BTreeSet<String>,
    out: &mut Vec<Symbol>,
) -> Result<(), String> {
    let path = text_of(file, "path");
    if !visited.insert(path.to_owned()) {
        return Ok(());
    }
    let source = source_of(context, path)?;
    let name = id.rsplit('.').next().unwrap_or(id);
    out.push(symbol(
        context,
        source,
        id,
        SymbolKind::Module,
        &format!("@import(\"{name}\")"),
        text_of(file, "doc"),
        1,
    ));
    fields(context, file, source, id, false, out);
    for decl in list(file, "decls")
        .iter()
        .filter(|d| d.get("public") == Some(&Value::Bool(true)))
    {
        if text_of(decl, "kind") == "import" {
            let wanted = resolve(path, text_of(decl, "import"));
            let target = context
                .files
                .iter()
                .skip(1)
                .find(|f| normalized(text_of(f, "path")) == normalized(&wanted));
            if let Some(target) = target {
                let child = qualified(id, text_of(decl, "name"));
                module(context, target, &child, visited, out)?;
            }
            continue;
        }
        declaration(context, decl, source, id, false, out);
    }
    Ok(())
}

fn source_of<'a>(context: &'a Context<'_>, path: &str) -> Result<&'a Source, String> {
    context
        .sources
        .iter()
        .find(|s| s.exec_path == path)
        .ok_or_else(|| format!("dumped file '{path}' is not in the srcs manifest"))
}

fn qualified(parent: &str, name: &str) -> String {
    format!("{parent}.{}", documentation_ir::escape_id_segment(name))
}

fn fields(
    context: &Context<'_>,
    parent: &Value,
    source: &Source,
    parent_id: &str,
    variants: bool,
    out: &mut Vec<Symbol>,
) {
    for item in list(parent, "fields") {
        let name = text_of(item, "name");
        if name.is_empty() {
            continue;
        }
        let ty = text_of(item, "type");
        let signature = if ty.is_empty() {
            name.to_owned()
        } else {
            format!("{name}: {ty}")
        };
        let kind = if variants {
            SymbolKind::EnumVariant
        } else {
            SymbolKind::Field
        };
        out.push(symbol(
            context,
            source,
            &qualified(parent_id, name),
            kind,
            &signature,
            text_of(item, "doc"),
            line_of(item),
        ));
    }
}

fn line_of(value: &Value) -> u64 {
    value
        .get("line")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn declaration(
    context: &Context<'_>,
    decl: &Value,
    source: &Source,
    parent_id: &str,
    in_container: bool,
    out: &mut Vec<Symbol>,
) {
    let name = text_of(decl, "name");
    if name.is_empty() {
        return;
    }
    let id = qualified(parent_id, name);
    let kind = match text_of(decl, "kind") {
        "fn" if in_container && first_param_is_self(decl) => SymbolKind::Method,
        "fn" => SymbolKind::Function,
        "struct" | "union" | "opaque" => SymbolKind::Class,
        "enum" | "error" => SymbolKind::Enum,
        "const" => SymbolKind::Constant,
        "var" => SymbolKind::Variable,
        _ => return,
    };
    let mut item = symbol(
        context,
        source,
        &id,
        kind,
        text_of(decl, "signature"),
        text_of(decl, "doc"),
        line_of(decl),
    );
    item.params = list(decl, "params")
        .iter()
        .map(|param| Param {
            name: text_of(param, "name").to_owned(),
            r#type: text_of(param, "type").to_owned(),
            doc: String::new(),
        })
        .collect();
    let returns = text_of(decl, "returns");
    if !returns.is_empty() {
        item.returns = Some(Returns {
            r#type: returns.to_owned(),
            doc: String::new(),
        });
    }
    out.push(item);
    let container = matches!(
        text_of(decl, "kind"),
        "struct" | "union" | "opaque" | "enum"
    );
    if container {
        fields(
            context,
            decl,
            source,
            &id,
            text_of(decl, "kind") == "enum",
            out,
        );
        for member in list(decl, "members")
            .iter()
            .filter(|m| m.get("public") == Some(&Value::Bool(true)))
        {
            if text_of(member, "kind") != "import" {
                declaration(context, member, source, &id, true, out);
            }
        }
    }
}

/// True when the first parameter is the container itself, by the `self` naming convention.
fn first_param_is_self(decl: &Value) -> bool {
    list(decl, "params")
        .first()
        .is_some_and(|param| matches!(text_of(param, "name"), "self" | "this"))
}

fn symbol(
    context: &Context<'_>,
    source: &Source,
    id: &str,
    kind: SymbolKind,
    signature: &str,
    doc: &str,
    line: u64,
) -> Symbol {
    Symbol {
        id: format!("{LANGUAGE}:{}:{id}", context.package),
        kind: kind as i32,
        signature_text: signature.to_owned(),
        doc_markdown: doc.trim().to_owned(),
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

#[cfg(test)]
mod lib_tests;
