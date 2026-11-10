//! Converts the symbol graph of `swift-symbolgraph-extract` into one validated DocIr shard.
//!
//! A symbol graph carries each public declaration with its doc comment, declaration fragments
//! and parameter list, but no source locations, so each declaration is located in the library's
//! own sources by matching its keyword and name.

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
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const LANGUAGE: &str = "swift";
const KEY_CONFIG: &str = "swift.config";

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
    pub graphs: String,
    pub sources: String,
    pub module: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: swift_extract --graphs DIR --sources FILE --module NAME --out FILE --package NAME [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if ![
            "--graphs",
            "--sources",
            "--module",
            "--out",
            "--package",
            "--config",
        ]
        .contains(&flag)
        {
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
        graphs: one("--graphs")?,
        sources: one("--sources")?,
        module: one("--module")?,
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
    let graph = read(&format!(
        "{}/{}.symbols.json",
        options.graphs, options.module
    ))?;
    let sources = parse_sources(&read(&options.sources)?)?;
    let mut texts = BTreeMap::new();
    for source in &sources {
        texts.insert(source.exec_path.clone(), read(&source.exec_path)?);
    }
    let shard = convert(
        &graph,
        &options.module,
        &sources,
        &texts,
        &options.package,
        &options.config,
    )?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

/// The IR kind and the declaration keywords of a symbol graph kind, or None when it is not documented.
fn kind_of(identifier: &str) -> Option<(SymbolKind, &'static [&'static str])> {
    Some(match identifier.strip_prefix("swift.")? {
        "struct" => (SymbolKind::Class, &["struct"]),
        "class" => (SymbolKind::Class, &["class", "actor"]),
        "protocol" => (SymbolKind::Interface, &["protocol"]),
        "enum" => (SymbolKind::Enum, &["enum"]),
        "enum.case" => (SymbolKind::EnumVariant, &["case"]),
        "func" | "func.op" => (SymbolKind::Function, &["func"]),
        "method" | "type.method" => (SymbolKind::Method, &["func"]),
        "init" => (SymbolKind::Method, &["init"]),
        "subscript" | "type.subscript" => (SymbolKind::Method, &["subscript"]),
        "property" | "type.property" | "var" => (SymbolKind::Property, &["var", "let"]),
        "typealias" => (SymbolKind::TypeAlias, &["typealias"]),
        "macro" => (SymbolKind::Macro, &["macro"]),
        _ => return None,
    })
}

struct Entry<'a> {
    precise: String,
    path: Vec<String>,
    kind: SymbolKind,
    keywords: &'static [&'static str],
    node: &'a Value,
}

/// A declaration's place in the sources: index into `sources` and 1-based line.
type Place = (usize, usize);

/// Converts one module's symbol graph into a validated IR shard. `texts` maps each exec path to its text.
pub fn convert(
    graph: &str,
    module: &str,
    sources: &[Source],
    texts: &BTreeMap<String, String>,
    package: &str,
    config: &str,
) -> Result<DocIr, String> {
    let root: Value =
        serde_json::from_str(graph).map_err(|error| format!("bad symbol graph: {error}"))?;
    let graph_module = root
        .pointer("/module/name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if graph_module != module {
        return Err(format!(
            "symbol graph is for module '{graph_module}', not '{module}'"
        ));
    }
    let mut config_text = format!("symbolgraph=1;package={package};module={module}");
    if !config.is_empty() {
        config_text.push_str(&format!(";config={config}"));
    }
    let mut order: Vec<usize> = (0..sources.len()).collect();
    order.sort_by(|a, b| sources[*a].rel.cmp(&sources[*b].rel));
    let lines: Vec<Vec<&str>> = sources
        .iter()
        .map(|source| {
            texts
                .get(&source.exec_path)
                .map_or("", String::as_str)
                .lines()
                .collect()
        })
        .collect();

    let mut entries: Vec<Entry<'_>> = Vec::new();
    for node in root
        .get("symbols")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
    {
        let access = node.get("accessLevel").and_then(Value::as_str);
        if !matches!(access, Some("public" | "open")) {
            continue;
        }
        let Some((kind, keywords)) = node
            .pointer("/kind/identifier")
            .and_then(Value::as_str)
            .and_then(kind_of)
        else {
            continue;
        };
        let path: Vec<String> = node
            .get("pathComponents")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        let precise = node
            .pointer("/identifier/precise")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        entries.push(Entry {
            precise,
            path,
            kind,
            keywords,
            node,
        });
    }
    entries.sort_by(|a, b| {
        a.path
            .len()
            .cmp(&b.path.len())
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.precise.cmp(&b.precise))
    });

    let module_id = documentation_ir::escape_id_segment(module);
    let mut taken_ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut ids_by_path: BTreeMap<Vec<String>, String> = BTreeMap::new();
    let mut used: BTreeSet<Place> = BTreeSet::new();
    let mut places: BTreeMap<String, Place> = BTreeMap::new();
    let mut symbols: Vec<Symbol> = Vec::new();

    let module_place = order.first().map(|first| (*first, 1));
    symbols.push(build(
        &format!("{LANGUAGE}:{package}:{module_id}"),
        SymbolKind::Module,
        format!("import {module}"),
        module,
        None,
        String::new(),
        module_place.map(|place| source_ref(sources, place)),
        &config_text,
    ));

    for entry in &entries {
        let mut id = module_id.clone();
        for segment in &entry.path {
            id.push('.');
            id.push_str(&documentation_ir::escape_id_segment(segment));
        }
        let count = taken_ids.entry(id.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            id = format!("{id}@{count}");
        }
        let parent_path = &entry.path[..entry.path.len() - 1];
        let parent_id = ids_by_path.get(parent_path).cloned();
        let member_of = parent_id.clone().unwrap_or_else(|| module_id.clone());
        ids_by_path
            .entry(entry.path.clone())
            .or_insert_with(|| id.clone());

        let name = entry.path[entry.path.len() - 1].as_str();
        let base = name.split('(').next().unwrap_or(name);
        let parent_place = parent_id.as_ref().and_then(|id| places.get(id)).copied();
        let place = locate(&lines, &order, entry.keywords, base, parent_place, &used);
        if let Some(place) = place {
            used.insert(place);
            places.insert(id.clone(), place);
        }

        let signature = fragments(entry.node.get("declarationFragments"));
        let (doc, mut params, returns) = split_doc(&doc_lines(entry.node));
        let mut symbol = build(
            &format!("{LANGUAGE}:{package}:{id}"),
            entry.kind,
            signature,
            name,
            Some(&format!("{LANGUAGE}:{package}:{member_of}")),
            doc,
            place.map(|place| source_ref(sources, place)),
            &config_text,
        );
        symbol.params = signature_params(entry.node, &mut params);
        let return_type = signature_returns(entry.node);
        if return_type.is_some() || returns.is_some() {
            symbol.returns = Some(Returns {
                r#type: return_type.unwrap_or_default(),
                doc: returns.unwrap_or_default(),
            });
        }
        symbols.push(symbol);
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

#[allow(clippy::too_many_arguments)]
fn build(
    id: &str,
    kind: SymbolKind,
    signature: String,
    display: &str,
    member_of: Option<&str>,
    doc: String,
    source: Option<SourceRef>,
    config: &str,
) -> Symbol {
    Symbol {
        id: id.to_owned(),
        kind: kind as i32,
        signature_text: signature,
        doc_markdown: doc,
        source,
        visibility: Visibility::Public as i32,
        relations: member_of.map(|parent| Relations {
            member_of: vec![parent.to_owned()],
            ..Relations::default()
        }),
        extensions: vec![Extension {
            key: KEY_CONFIG.to_owned(),
            value: config.as_bytes().to_vec(),
        }],
        display_name: display.to_owned(),
        ..Symbol::default()
    }
}

fn source_ref(sources: &[Source], place: Place) -> SourceRef {
    SourceRef {
        file: sources[place.0].rel.clone(),
        line: place.1 as u64,
        generated: sources[place.0].generated,
    }
}

/// The spellings of declaration fragments, joined.
fn fragments(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("spelling").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn doc_lines(node: &Value) -> Vec<String> {
    node.pointer("/docComment/lines")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .map(|line| {
                    line.get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim_end()
                        .to_owned()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Splits a doc comment into its markdown, its `Parameter` entries and its `Returns` entry.
fn split_doc(lines: &[String]) -> (String, Vec<(String, String)>, Option<String>) {
    let mut kept: Vec<&str> = Vec::new();
    let mut params: Vec<(String, String)> = Vec::new();
    let mut returns: Option<String> = None;
    #[derive(PartialEq)]
    enum Open {
        None,
        Param,
        Returns,
        Block,
    }
    let mut open = Open::None;
    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if let Some(rest) = lower
            .strip_prefix("- parameter ")
            .map(|_| &trimmed["- parameter ".len()..])
            && let Some((name, text)) = rest.split_once(':')
        {
            params.push((name.trim().to_owned(), text.trim().to_owned()));
            open = Open::Param;
            continue;
        }
        if lower == "- parameters:" {
            open = Open::Block;
            continue;
        }
        if lower.starts_with("- returns:") {
            returns = Some(trimmed["- returns:".len()..].trim().to_owned());
            open = Open::Returns;
            continue;
        }
        if open == Open::Block
            && indented
            && let Some(item) = trimmed.strip_prefix("- ")
            && let Some((name, text)) = item.split_once(':')
        {
            params.push((name.trim().to_owned(), text.trim().to_owned()));
            continue;
        }
        if indented && !trimmed.is_empty() {
            match open {
                Open::Param | Open::Block => {
                    if let Some(last) = params.last_mut() {
                        last.1 = format!("{} {}", last.1, trimmed).trim().to_owned();
                        continue;
                    }
                }
                Open::Returns => {
                    if let Some(text) = returns.as_mut() {
                        *text = format!("{text} {trimmed}").trim().to_owned();
                        continue;
                    }
                }
                Open::None => {}
            }
        }
        open = Open::None;
        kept.push(line.as_str());
    }
    let doc = kept.join("\n").trim().to_owned();
    (doc, params, returns)
}

/// The parameters a function signature lists, with the doc text matched from `documented`.
fn signature_params(node: &Value, documented: &mut Vec<(String, String)>) -> Vec<Param> {
    let listed = node
        .pointer("/functionSignature/parameters")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut out = Vec::new();
    for item in listed {
        let label = item.get("name").and_then(Value::as_str).unwrap_or_default();
        let internal = item
            .get("internalName")
            .and_then(Value::as_str)
            .unwrap_or(label);
        let text = fragments(item.get("declarationFragments"));
        let r#type = text
            .split_once(": ")
            .map_or(String::new(), |(_, ty)| ty.to_owned());
        let doc = documented
            .iter()
            .position(|(name, _)| name == internal || name == label)
            .map(|at| documented.remove(at).1)
            .unwrap_or_default();
        out.push(Param {
            name: internal.to_owned(),
            r#type,
            doc,
        });
    }
    if listed.is_empty() {
        for (name, doc) in documented.drain(..) {
            out.push(Param {
                name,
                r#type: String::new(),
                doc,
            });
        }
    }
    out
}

fn signature_returns(node: &Value) -> Option<String> {
    let text = fragments(node.pointer("/functionSignature/returns"));
    (!text.is_empty() && text != "()" && text != "Void").then_some(text)
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `line` declares `name` with one of `keywords`.
fn declares(line: &str, keywords: &[&str], name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return false;
    }
    for keyword in keywords {
        let mut from = 0;
        while let Some(found) = line[from..].find(keyword) {
            let start = from + found;
            let end = start + keyword.len();
            from = end;
            if line[..start].ends_with(is_ident) || line[end..].starts_with(is_ident) {
                continue;
            }
            if matches!(*keyword, "init" | "subscript") {
                return true;
            }
            let after = &line[end..];
            let rest = after.trim_start();
            if rest.len() == after.len() {
                continue;
            }
            let hit = if *keyword == "case" {
                rest.split(',').any(|part| {
                    let part = part.trim_start();
                    part.strip_prefix(name)
                        .is_some_and(|tail| !tail.starts_with(is_ident))
                })
            } else {
                rest.strip_prefix(name)
                    .is_some_and(|tail| !(name.starts_with(is_ident) && tail.starts_with(is_ident)))
            };
            if hit {
                return true;
            }
        }
    }
    false
}

/// The first unused declaration of `name`, searched from the parent's line when it is known.
fn locate(
    lines: &[Vec<&str>],
    order: &[usize],
    keywords: &[&str],
    name: &str,
    parent: Option<Place>,
    used: &BTreeSet<Place>,
) -> Option<Place> {
    let search = |file: usize, from: usize| {
        lines[file]
            .iter()
            .enumerate()
            .skip(from.saturating_sub(1))
            .map(|(at, line)| (at + 1, *line))
            .find(|(at, line)| !used.contains(&(file, *at)) && declares(line, keywords, name))
            .map(|(at, _)| (file, at))
    };
    if let Some((file, line)) = parent
        && let Some(found) = search(file, line)
    {
        return Some(found);
    }
    order.iter().find_map(|file| search(*file, 1))
}

#[cfg(test)]
mod lib_tests;
