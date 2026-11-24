#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use documentation_ir::proto::{DocIr, Extension, Relations, Symbol, SymbolKind, Visibility};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
use serde_json::Value;

mod html;

use html::{Node, markdown, text};

const LANGUAGE: &str = "ocaml";
const PAGE_FILE: &str = "index.html.json";
const KEY_INCOMPLETE: &str = "ocaml.incomplete";
const KEY_CONFIG: &str = "ocaml.config";
const KEY_ITEM: &str = "ocaml.item";

#[derive(Debug)]
pub enum OcamlExtractError {
    Io(PathIoError),
    Stat {
        path: PathBuf,
        source: io::Error,
    },
    InvalidPackage {
        package: String,
    },
    Malformed {
        key: String,
        source: serde_json::Error,
    },
    NotBelowPackage {
        key: String,
    },
    NoPublicApi {
        package: String,
    },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for OcamlExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Stat { path, source } => write!(f, "cannot stat {}: {source}", path.display()),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed { key, source } => {
                write!(f, "odoc page '{key}' is malformed: {source}")
            }
            Self::NotBelowPackage { key } => {
                write!(f, "odoc page '{key}' is not below a package directory")
            }
            Self::NoPublicApi { package } => write!(
                f,
                "odoc output lists no public OCaml API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OcamlExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Stat { source, .. } => Some(source),
            Self::Malformed { source, .. } => Some(source),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NotBelowPackage { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub html: Vec<String>,
    pub package: String,
    pub config: String,
    pub out: String,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("ocaml_extract", usage(), parse_args, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: ocaml_extract --html DIR... --package NAME [--config TEXT] --out FILE"
}

pub fn parse_args(args: &[String]) -> Result<Options, FlagError> {
    let mut flags =
        documentation_ir::tool::flags(args, &["--html", "--package", "--config", "--out"])?;
    let package = flags.one("--package")?;
    let config = flags.optional("--config")?.unwrap_or_default();
    let out = flags.one("--out")?;
    let html = flags.many("--html");
    if html.is_empty() {
        return Err(FlagError::Required {
            flag: "--html".to_owned(),
        });
    }
    Ok(Options {
        html,
        package,
        config,
        out,
    })
}

pub fn run(options: &Options) -> Result<(), OcamlExtractError> {
    let mut pages = Vec::new();
    for html in &options.html {
        collect(Path::new(html), Path::new(""), &mut pages)?;
    }
    pages.sort_by(|a, b| a.0.cmp(&b.0));
    let shard = convert(&pages, &options.package, &options.config)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(OcamlExtractError::Write)
}

fn collect(
    root: &Path,
    relative: &Path,
    pages: &mut Vec<(String, String)>,
) -> Result<(), OcamlExtractError> {
    let directory: PathBuf = root.join(relative);
    for entry in path_io::read_dir(&directory).map_err(OcamlExtractError::Io)? {
        let Some(name) = entry.file_name() else {
            continue;
        };
        let name = name.to_string_lossy().into_owned();
        let path = relative.join(&name);
        let kind = fs::symlink_metadata(&entry)
            .map_err(|source| OcamlExtractError::Stat {
                path: entry.clone(),
                source,
            })?
            .file_type();
        if kind.is_dir() {
            collect(root, &path, pages)?;
        } else if name == PAGE_FILE {
            let json = path_io::read_to_string(&entry).map_err(OcamlExtractError::Io)?;
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
        if let Some(name) = id.strip_prefix(label).and_then(|r| r.strip_prefix('-'))
            && !name.is_empty()
        {
            return Some((label, name));
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

fn page_names(key: &str) -> Result<(Vec<String>, &'static str), OcamlExtractError> {
    let mut segments: Vec<&str> = key.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return Err(OcamlExtractError::NotBelowPackage {
            key: key.to_owned(),
        });
    }
    segments.remove(0);
    let mut names = Vec::new();
    let mut label = "module";
    for segment in segments {
        if let Some((found, name)) = split_anchor(segment) {
            label = found;
            names.push(name.to_owned());
        } else {
            label = "module";
            names.push(segment.to_owned());
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

pub fn convert(
    pages: &[(String, String)],
    package: &str,
    config: &str,
) -> Result<DocIr, OcamlExtractError> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(OcamlExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let mut items: Vec<Item> = Vec::new();
    for (key, json) in pages {
        let root: Value =
            serde_json::from_str(json).map_err(|source| OcamlExtractError::Malformed {
                key: key.clone(),
                source,
            })?;
        let field = |name: &str| -> String {
            root.get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let (names, label) = page_names(key)?;
        let path = names.join(".");
        let parent = match names.split_last() {
            Some((_, init)) if !init.is_empty() => Some(init.join(".")),
            _ => None,
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
        return Err(OcamlExtractError::NoPublicApi {
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(OcamlExtractError::InvalidIr)?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
