#![deny(clippy::indexing_slicing, clippy::string_slice)]

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{
    FlagError, PathIoError, Source, SourcesError, WriteShardError, parse_sources, path_io,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::path::Path;

const LANGUAGE: &str = "swift";
const KEY_CONFIG: &str = "swift.config";
const KEY_EXTENDS: &str = "swift.extends";

#[derive(Debug)]
pub enum SwiftExtractError {
    Io(PathIoError),
    Sources(SourcesError),
    BadGraph(serde_json::Error),
    BadExtensionGraph {
        module: String,
        other: String,
        source: serde_json::Error,
    },
    WrongModule {
        found: String,
        module: String,
    },
    EmptyPath,
    SourceOutOfRange {
        index: usize,
    },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for SwiftExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Sources(error) => error.fmt(f),
            Self::BadGraph(error) => write!(f, "bad symbol graph: {error}"),
            Self::BadExtensionGraph {
                module,
                other,
                source,
            } => write!(f, "bad symbol graph for {module}@{other}: {source}"),
            Self::WrongModule { found, module } => {
                write!(f, "symbol graph is for module '{found}', not '{module}'")
            }
            Self::EmptyPath => f.write_str("a symbol has an empty path"),
            Self::SourceOutOfRange { index } => write!(f, "source index {index} is out of range"),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SwiftExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sources(error) => Some(error),
            Self::BadGraph(error) => Some(error),
            Self::BadExtensionGraph { source, .. } => Some(source),
            Self::Write(error) => Some(error),
            Self::WrongModule { .. }
            | Self::EmptyPath
            | Self::SourceOutOfRange { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub graphs: String,
    pub sources: String,
    pub module: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("swift_extract", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: swift_extract --graphs DIR --sources FILE --module NAME --out FILE --package NAME [--config TEXT]"
}

pub fn parse(args: &[String]) -> Result<Options, FlagError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--graphs",
            "--sources",
            "--module",
            "--out",
            "--package",
            "--config",
        ],
    )?;
    Ok(Options {
        graphs: flags.one("--graphs")?,
        sources: flags.one("--sources")?,
        module: flags.one("--module")?,
        out: flags.one("--out")?,
        package: flags.one("--package")?,
        config: flags.optional("--config")?.unwrap_or_default(),
    })
}

pub fn run(options: &Options) -> Result<(), SwiftExtractError> {
    let read = |path: &str| path_io::read_to_string(Path::new(path)).map_err(SwiftExtractError::Io);
    let graph = read(&format!(
        "{}/{}.symbols.json",
        options.graphs, options.module
    ))?;
    let extensions = read_extension_graphs(&options.graphs, &options.module)?;
    let sources = parse_sources(&read(&options.sources)?).map_err(SwiftExtractError::Sources)?;
    let mut texts = BTreeMap::new();
    for source in &sources {
        texts.insert(source.exec_path.clone(), read(&source.exec_path)?);
    }
    let shard = convert(
        &graph,
        &extensions,
        &options.module,
        &sources,
        &texts,
        &options.package,
        &options.config,
    )?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(SwiftExtractError::Write)
}

fn read_extension_graphs(
    dir: &str,
    module: &str,
) -> Result<Vec<(String, String)>, SwiftExtractError> {
    let prefix = format!("{module}@");
    let mut found: Vec<(String, String)> = Vec::new();
    for path in path_io::read_dir(Path::new(dir)).map_err(SwiftExtractError::Io)? {
        let Some(name) = path.file_name() else {
            continue;
        };
        let name = name.to_string_lossy();
        let Some(other) = name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".symbols.json"))
        else {
            continue;
        };
        let text = path_io::read_to_string(&path).map_err(SwiftExtractError::Io)?;
        found.push((other.to_owned(), text));
    }
    found.sort();
    Ok(found)
}

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
    other: Option<String>,
    extends: Option<String>,
    precise: String,
    path: Vec<String>,
    kind: SymbolKind,
    keywords: &'static [&'static str],
    node: &'a Value,
}

type Place = (usize, usize);

fn collect<'a>(root: &'a Value, other: Option<&str>, entries: &mut Vec<Entry<'a>>) {
    let extended: BTreeMap<&str, &str> = root
        .get("relationships")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter(|relation| relation.get("kind").and_then(Value::as_str) == Some("memberOf"))
        .filter_map(|relation| {
            Some((
                relation.get("source")?.as_str()?,
                relation.get("target")?.as_str()?,
            ))
        })
        .collect();
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
        let extends = other
            .and_then(|_| extended.get(precise.as_str()))
            .map(|target| (*target).to_owned());
        entries.push(Entry {
            other: other.map(str::to_owned),
            extends,
            precise,
            path,
            kind,
            keywords,
            node,
        });
    }
}

pub fn convert(
    graph: &str,
    extension_graphs: &[(String, String)],

    module: &str,
    sources: &[Source],
    texts: &BTreeMap<String, String>,
    package: &str,
    config: &str,
) -> Result<DocIr, SwiftExtractError> {
    let root: Value = serde_json::from_str(graph).map_err(SwiftExtractError::BadGraph)?;
    let graph_module = root
        .pointer("/module/name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if graph_module != module {
        return Err(SwiftExtractError::WrongModule {
            found: graph_module.to_owned(),
            module: module.to_owned(),
        });
    }
    let mut config_text = format!("symbolgraph=1;package={package};module={module}");
    if !config.is_empty() {
        write!(config_text, ";config={config}").unwrap_or_default();
    }
    let mut order: Vec<usize> = (0..sources.len()).collect();
    order.sort_by_key(|index| sources.get(*index).map(|source| &source.rel));
    let source_lines: Vec<Vec<&str>> = sources
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
    collect(&root, None, &mut entries);
    let extension_roots: Vec<(&str, Value)> = extension_graphs
        .iter()
        .map(|(other, text)| {
            serde_json::from_str(text)
                .map(|value| (other.as_str(), value))
                .map_err(|source| SwiftExtractError::BadExtensionGraph {
                    module: module.to_owned(),
                    other: other.clone(),
                    source,
                })
        })
        .collect::<Result<_, _>>()?;
    for (other, value) in &extension_roots {
        collect(value, Some(other), &mut entries);
    }
    entries.sort_by(|a, b| {
        a.path
            .len()
            .cmp(&b.path.len())
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.precise.cmp(&b.precise))
    });

    let module_id = documentation_ir::escape_id_segment(module);
    let mut assembly = Assembly {
        sources,
        source_lines: &source_lines,
        order: &order,
        module_id: &module_id,
        package,
        config_text: &config_text,
        taken_ids: BTreeMap::new(),
        ids_by_path: BTreeMap::new(),
        used: BTreeSet::new(),
        places: BTreeMap::new(),
    };
    let mut symbols: Vec<Symbol> = Vec::new();

    let module_place = order.first().map(|first| (*first, 1));
    symbols.push(build(
        &format!("{LANGUAGE}:{package}:{module_id}"),
        SymbolKind::Module,
        format!("import {module}"),
        module,
        None,
        String::new(),
        module_place
            .map(|place| source_ref(sources, place))
            .transpose()?,
        &config_text,
    ));

    for entry in &entries {
        symbols.push(assembly.symbol(entry)?);
    }
    let mut links: BTreeMap<String, String> = BTreeMap::new();
    for (path, id) in &assembly.ids_by_path {
        links.insert(path.join("/"), format!("{LANGUAGE}:{package}:{id}"));
    }
    for symbol in &mut symbols {
        symbol.doc_markdown = link_symbols(&symbol.doc_markdown, module, &links);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(SwiftExtractError::InvalidIr)?;
    Ok(shard)
}

struct Assembly<'a> {
    sources: &'a [Source],
    source_lines: &'a [Vec<&'a str>],
    order: &'a [usize],
    module_id: &'a str,
    package: &'a str,
    config_text: &'a str,
    taken_ids: BTreeMap<String, usize>,
    ids_by_path: BTreeMap<Vec<String>, String>,
    used: BTreeSet<Place>,
    places: BTreeMap<String, Place>,
}

impl Assembly<'_> {
    fn symbol(&mut self, entry: &Entry<'_>) -> Result<Symbol, SwiftExtractError> {
        let (sources, source_lines, order) = (self.sources, self.source_lines, self.order);
        let (module_id, package, config_text) = (self.module_id, self.package, self.config_text);
        let mut id = entry.other.as_ref().map_or_else(
            || module_id.to_owned(),
            |other| documentation_ir::escape_id_segment(other),
        );
        for segment in &entry.path {
            id.push('.');
            id.push_str(&documentation_ir::escape_id_segment(segment));
        }
        let count = self.taken_ids.entry(id.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            id = format!("{id}@{count}");
        }
        let Some((name, parent_path)) = entry.path.split_last() else {
            return Err(SwiftExtractError::EmptyPath);
        };
        let name = name.as_str();
        let parent_id = match entry.other {
            Some(_) => None,
            None => self.ids_by_path.get(parent_path).cloned(),
        };
        let member_of = parent_id.clone().unwrap_or_else(|| module_id.to_owned());
        if entry.other.is_none() {
            self.ids_by_path
                .entry(entry.path.clone())
                .or_insert_with(|| id.clone());
        }

        let base = name.split('(').next().unwrap_or(name);
        let parent_place = parent_id
            .as_ref()
            .and_then(|id| self.places.get(id))
            .copied();
        let place = locate(
            source_lines,
            order,
            entry.keywords,
            base,
            parent_place,
            &self.used,
        );
        if let Some(place) = place {
            self.used.insert(place);
            self.places.insert(id.clone(), place);
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
            place.map(|place| source_ref(sources, place)).transpose()?,
            config_text,
        );
        if let Some(target) = &entry.extends {
            symbol.extensions.push(Extension {
                key: KEY_EXTENDS.to_owned(),
                value: target.as_bytes().to_vec(),
            });
        }
        symbol.params = signature_params(entry.node, &mut params);
        let return_type = signature_returns(entry.node);
        if return_type.is_some() || returns.is_some() {
            symbol.returns = Some(Returns {
                r#type: return_type.unwrap_or_default(),
                doc: returns.unwrap_or_default(),
            });
        }
        Ok(symbol)
    }
}

fn link_symbols(text: &str, module: &str, links: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("``") {
        let Some((before, tail)) = rest.split_at_checked(open) else {
            break;
        };
        let run = tail.bytes().take_while(|b| *b == b'`').count();
        let Some((ticks, remainder)) = tail.split_at_checked(run) else {
            break;
        };
        out.push_str(before);
        out.push_str(ticks);
        rest = remainder;
        if run != 2 {
            continue;
        }
        let Some(close) = rest.find('`') else {
            break;
        };
        let Some((name, from_close)) = rest.split_at_checked(close) else {
            break;
        };
        let closing = from_close.bytes().take_while(|b| *b == b'`').count();
        let key = name
            .strip_prefix(module)
            .and_then(|tail| tail.strip_prefix('/'))
            .filter(|tail| !links.contains_key(name) && !tail.is_empty())
            .unwrap_or(name);
        let target = links.get(key);
        match (closing, target, key.rsplit('/').next()) {
            (2, Some(id), Some(last))
                if !name.is_empty() && !name.contains('\n') && !last.is_empty() =>
            {
                out.truncate(out.len() - 2);
                write!(out, "[{last}](dx:{id})").unwrap_or_default();
                let Some(remainder) = from_close.get(2..) else {
                    break;
                };
                rest = remainder;
            }
            _ => {
                let Some((ticks, remainder)) = from_close.split_at_checked(closing) else {
                    break;
                };
                out.push_str(name);
                out.push_str(ticks);
                rest = remainder;
            }
        }
    }
    out.push_str(rest);
    out
}

#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct input of the call"
)]
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

fn source_ref(sources: &[Source], place: Place) -> Result<SourceRef, SwiftExtractError> {
    let source = sources
        .get(place.0)
        .ok_or(SwiftExtractError::SourceOutOfRange { index: place.0 })?;
    Ok(SourceRef {
        file: source.rel.clone(),
        line: place.1 as u64,
        generated: source.generated,
    })
}

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

fn split_doc(lines: &[String]) -> (String, Vec<(String, String)>, Option<String>) {
    #[derive(PartialEq)]
    enum Open {
        None,
        Param,
        Returns,
        Block,
    }
    let mut kept: Vec<&str> = Vec::new();
    let mut params: Vec<(String, String)> = Vec::new();
    let mut returns: Option<String> = None;
    let mut open = Open::None;
    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if let Some(rest) = lower
            .strip_prefix("- parameter ")
            .and_then(|_| trimmed.get("- parameter ".len()..))
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
        if lower.starts_with("- returns:")
            && let Some(tail) = trimmed.get("- returns:".len()..)
        {
            returns = Some(tail.trim().to_owned());
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
                        format!("{} {}", last.1, trimmed)
                            .trim()
                            .clone_into(&mut last.1);
                        continue;
                    }
                }
                Open::Returns => {
                    if let Some(text) = returns.as_mut() {
                        format!("{text} {trimmed}").trim().clone_into(text);
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

fn declares(line: &str, keywords: &[&str], name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return false;
    }
    for keyword in keywords {
        let mut from = 0;
        while let Some(found) = line.get(from..).and_then(|tail| tail.find(keyword)) {
            let start = from + found;
            let end = start + keyword.len();
            from = end;
            if line
                .get(..start)
                .is_some_and(|head| head.ends_with(is_ident))
                || line
                    .get(end..)
                    .is_some_and(|tail| tail.starts_with(is_ident))
            {
                continue;
            }
            if matches!(*keyword, "init" | "subscript") {
                return true;
            }
            let Some(after) = line.get(end..) else {
                continue;
            };
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

fn locate(
    lines: &[Vec<&str>],
    order: &[usize],
    keywords: &[&str],
    name: &str,
    parent: Option<Place>,
    used: &BTreeSet<Place>,
) -> Option<Place> {
    let search = |file: usize, from: usize| {
        lines
            .get(file)?
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
