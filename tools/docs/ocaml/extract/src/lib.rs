//! Converts the JSON pages of `odoc html-generate --as-json` into one validated DocIr shard.

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
use std::path::{Path, PathBuf};

use documentation_ir::proto::{DocIr, Extension, Relations, Symbol, SymbolKind, Visibility};
use serde_json::Value;

mod html;

use html::{Node, markdown, text};

const LANGUAGE: &str = "ocaml";
const PAGE_FILE: &str = "index.html.json";
const KEY_INCOMPLETE: &str = "ocaml.incomplete";
const KEY_CONFIG: &str = "ocaml.config";
const KEY_ITEM: &str = "ocaml.item";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub html: Vec<String>,
    pub package: String,
    pub config: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: ocaml_extract --html DIR... --package NAME [--config TEXT] --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse_args(args: &[String]) -> Result<Options, String> {
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
        let slot = match flag {
            "--html" => {
                options.html.push(value.clone());
                index += 2;
                continue;
            }
            "--package" => &mut options.package,
            "--config" => &mut options.config,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        if !slot.is_empty() {
            return Err(format!("{flag} given more than once"));
        }
        *slot = value.clone();
        index += 2;
    }
    if options.html.is_empty() {
        return Err("--html is required".to_owned());
    }
    for (flag, value) in [("--package", &options.package), ("--out", &options.out)] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let mut pages = Vec::new();
    for html in &options.html {
        collect(Path::new(html), Path::new(""), &mut pages)?;
    }
    pages.sort_by(|a, b| a.0.cmp(&b.0));
    let shard = convert(&pages, &options.package, &options.config)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

/// Reads every page below `root`, keyed by its directory relative to `root`.
fn collect(root: &Path, relative: &Path, pages: &mut Vec<(String, String)>) -> Result<(), String> {
    let directory: PathBuf = root.join(relative);
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = relative.join(&name);
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot stat {}: {error}", entry.path().display()))?;
        if kind.is_dir() {
            collect(root, &path, pages)?;
        } else if name == PAGE_FILE {
            let json = fs::read_to_string(entry.path())
                .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
            let key = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            pages.push((key, json));
        }
    }
    Ok(())
}

/// One documented item before ids are made unique.
#[derive(Clone, Debug)]
struct Item {
    path: String,
    name: String,
    parent: Option<String>,
    kind: SymbolKind,
    label: &'static str,
    signature: String,
    doc: String,
}

/// Splits an anchor id such as `module-type-S` into its label and name.
fn split_anchor(id: &str) -> Option<(&'static str, &str)> {
    const LABELS: [&str; 9] = [
        "module-type",
        "class-type",
        "module",
        "class",
        "exception",
        "extension",
        "method",
        "type",
        "val",
    ];
    for label in LABELS {
        if let Some(name) = id.strip_prefix(label).and_then(|r| r.strip_prefix('-')) {
            if !name.is_empty() {
                return Some((label, name));
            }
        }
    }
    None
}

fn kind_of(label: &str) -> SymbolKind {
    match label {
        "module" => SymbolKind::Module,
        "module-type" | "class-type" => SymbolKind::Interface,
        "class" => SymbolKind::Class,
        "method" => SymbolKind::Method,
        "val" => SymbolKind::Function,
        "exception" | "extension" => SymbolKind::EnumVariant,
        _ => SymbolKind::TypeAlias,
    }
}

fn keyword(label: &str) -> &'static str {
    match label {
        "module" => "module",
        "module-type" => "module type",
        "class" => "class",
        "class-type" => "class type",
        _ => "",
    }
}

/// Returns the names the page directory stands for, such as `["Shapes", "Sub"]`, and the label
/// of its last segment.
fn page_names(key: &str) -> Result<(Vec<String>, &'static str), String> {
    let mut segments: Vec<&str> = key.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return Err(format!("odoc page '{key}' is not below a package directory"));
    }
    segments.remove(0);
    let mut names = Vec::new();
    let mut label = "module";
    for segment in segments {
        match split_anchor(segment) {
            Some((found, name)) => {
                label = found;
                names.push(name.to_owned());
            }
            None => {
                label = "module";
                names.push(segment.to_owned());
            }
        }
    }
    Ok((names, label))
}

fn walk<'a>(node: &'a Node, found: &mut Vec<&'a Node>) {
    if node.has_class("odoc-spec") {
        found.push(node);
    }
    for child in &node.children {
        walk(child, found);
    }
}

fn items_of(content: &Node, parent: &str) -> Vec<Item> {
    let mut specs = Vec::new();
    walk(content, &mut specs);
    let mut items = Vec::new();
    for spec in specs {
        let Some(head) = spec
            .children
            .iter()
            .find(|c| c.has_class("spec") && c.has_class("anchored"))
        else {
            continue;
        };
        let Some((label, name)) = head.attr("id").and_then(split_anchor) else {
            continue;
        };
        let doc = spec
            .children
            .iter()
            .find(|c| c.has_class("spec-doc"))
            .map(markdown)
            .unwrap_or_default();
        items.push(Item {
            path: format!("{parent}.{name}"),
            name: name.to_owned(),
            parent: Some(parent.to_owned()),
            kind: kind_of(label),
            label,
            signature: text(head, &["def-doc", "anchor"]),
            doc,
        });
    }
    items
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

fn qualified(path: &str) -> String {
    path.split('.')
        .filter(|segment| !segment.is_empty())
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

/// Converts the pages (directory key, JSON text) of one odoc package into a validated shard.
pub fn convert(
    pages: &[(String, String)],
    package: &str,
    config: &str,
) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let mut items: Vec<Item> = Vec::new();
    for (key, json) in pages {
        let root: Value = serde_json::from_str(json)
            .map_err(|error| format!("odoc page '{key}' is malformed: {error}"))?;
        let field = |name: &str| -> String {
            root.get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let (names, label) = page_names(key)?;
        let path = names.join(".");
        let parent = if names.len() > 1 {
            Some(names[..names.len() - 1].join("."))
        } else {
            None
        };
        let name = names.last().cloned().unwrap_or_default();
        let preamble = html::parse(&field("preamble"));
        items.push(Item {
            path: path.clone(),
            name: name.clone(),
            parent,
            kind: kind_of(label),
            label,
            signature: format!("{} {name}", keyword(label)),
            doc: markdown(&Node::root(preamble)),
        });
        let content = Node::root(html::parse(&field("content")));
        items.extend(items_of(&content, &path));
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for item in &items {
        *seen.entry(item.path.clone()).or_default() += 1;
    }
    items.sort_by(|a, b| (&a.path, a.label).cmp(&(&b.path, b.label)));
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let prefix = format!("{LANGUAGE}:{package}:");
    let mut symbols = Vec::new();
    for item in &items {
        let first = used.entry(item.path.clone()).or_default();
        let ambiguous = seen.get(&item.path).copied().unwrap_or(0) > 1;
        let path = if ambiguous && *first > 0 {
            format!("{}-{}", item.path, item.label)
        } else {
            item.path.clone()
        };
        *first += 1;
        let relations = item.parent.as_ref().map(|parent| Relations {
            member_of: vec![format!("{prefix}{}", qualified(parent))],
            implements: Vec::new(),
            reexported_as: Vec::new(),
        });
        let doc = item.doc.trim().to_owned();
        let mut extensions = vec![extension(KEY_ITEM, item.label)];
        if doc.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, "doc"));
        }
        if !config.is_empty() {
            extensions.push(extension(KEY_CONFIG, config));
        }
        extensions.sort_by(|a, b| a.key.cmp(&b.key));
        let signature = if item.signature.trim().is_empty() {
            item.name.clone()
        } else {
            item.signature.clone()
        };
        symbols.push(Symbol {
            id: format!("{prefix}{}", qualified(&path)),
            kind: item.kind as i32,
            signature_text: signature,
            doc_markdown: doc,
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: item.name.clone(),
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "odoc output lists no public OCaml API of package '{package}'"
        ));
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

#[cfg(test)]
mod lib_tests;
