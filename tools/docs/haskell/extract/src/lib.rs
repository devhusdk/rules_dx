//! Converts the Hoogle text of `haddock --hoogle` into one validated DocIr shard.
//!
//! Hoogle text is haddock's stable, machine-readable output: one line per module, type,
//! constructor, field, class and function, each preceded by its `-- |` doc. It carries no
//! positions, so each declaration is located in the module source by matching its name.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use documentation_ir::proto::{DocIr, Extension, SourceRef, Symbol, SymbolKind, Visibility};
use std::collections::BTreeMap;
use std::fs;

const LANGUAGE: &str = "haskell";
const KEY_CONFIG: &str = "haskell.config";

/// One source file of the library: where haddock read it, and where it lives in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub rel: String,
    pub generated: bool,
}

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub hoogle: String,
    pub sources: String,
    pub out: String,
    pub package: String,
    pub config: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: haskell_extract --hoogle DIR --sources FILE --out FILE --package NAME [--config TEXT]"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !["--hoogle", "--sources", "--out", "--package", "--config"].contains(&flag) {
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
        hoogle: one("--hoogle")?,
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
    let mut files = Vec::new();
    let entries = fs::read_dir(&options.hoogle)
        .map_err(|error| format!("cannot read {}: {error}", options.hoogle))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt") {
            files.push(path.to_string_lossy().into_owned());
        }
    }
    let hoogle = match files.as_slice() {
        [one] => read(one)?,
        [] => return Err(format!("no *.txt Hoogle file in {}", options.hoogle)),
        _ => return Err(format!("several *.txt files in {}", options.hoogle)),
    };
    let sources = parse_sources(&read(&options.sources)?)?;
    let mut texts = BTreeMap::new();
    for source in &sources {
        texts.insert(source.exec_path.clone(), read(&source.exec_path)?);
    }
    let shard = convert(&hoogle, &sources, &texts, &options.package, &options.config)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

struct Module<'a> {
    name: String,
    source: &'a Source,
    text: &'a str,
}

struct Context<'a> {
    package: &'a str,
    config: String,
}

/// The type or class the following constructors, fields or methods belong to.
struct Parent {
    id: String,
    name: String,
    is_class: bool,
    line: usize,
}

/// Converts Hoogle text into a validated IR shard. `texts` maps each exec path to its text.
pub fn convert(
    hoogle: &str,
    sources: &[Source],
    texts: &BTreeMap<String, String>,
    package: &str,
    config: &str,
) -> Result<DocIr, String> {
    let mut config_text = format!("hoogle=1;package={package}");
    if !config.is_empty() {
        config_text.push_str(&format!(";config={config}"));
    }
    let context = Context {
        package,
        config: config_text,
    };
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut module: Option<Module<'_>> = None;
    let mut module_id = String::new();
    let mut parent: Option<Parent> = None;
    let mut doc: Vec<String> = Vec::new();
    for raw in hoogle.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix("--") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if let Some(first) = rest.strip_prefix('|').or_else(|| rest.strip_prefix('^')) {
                doc.clear();
                doc.push(first.trim().to_owned());
            } else if !doc.is_empty() {
                doc.push(rest.trim().to_owned());
            }
            continue;
        }
        let taken = std::mem::take(&mut doc);
        let docs = taken.join("\n").trim().to_owned();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        if let Some(name) = line.strip_prefix("module ") {
            let name = name.trim().to_owned();
            let found = find_source(sources, texts, &name)?;
            module_id = qualified("", &name);
            parent = None;
            let at = line_of(found.text, |l| l.trim_start().starts_with("module "));
            symbols.push(symbol(
                &context,
                &found,
                &module_id,
                SymbolKind::Module,
                format!("module {name}"),
                docs,
                at,
            ));
            module = Some(found);
            continue;
        }
        let Some(current) = module.as_ref() else {
            continue;
        };
        let first = line.split_whitespace().next().unwrap_or_default();
        match first {
            "instance" | "infixl" | "infixr" | "infix" | "default" | "deriving" => continue,
            "data" | "newtype" | "type" | "class" => {
                let words: Vec<&str> = line.split_whitespace().collect();
                let skip = words
                    .iter()
                    .skip(1)
                    .take_while(|w| matches!(**w, "family" | "role"))
                    .count();
                if words.get(1) == Some(&"instance") {
                    continue;
                }
                let Some(name) = declared_name(&words[1 + skip..], first == "class") else {
                    continue;
                };
                let kind = match first {
                    "class" => SymbolKind::Interface,
                    "type" => SymbolKind::TypeAlias,
                    _ => SymbolKind::Class,
                };
                let id = qualified(&module_id, &name);
                let at = line_of(current.text, |l| {
                    let t = l.trim_start();
                    t.starts_with(first) && tokens(t).any(|w| w == name)
                });
                symbols.push(symbol(&context, current, &id, kind, line.to_owned(), docs, at));
                parent = matches!(kind, SymbolKind::Class | SymbolKind::Interface).then(|| {
                    Parent {
                        id,
                        name,
                        is_class: first == "class",
                        line: at,
                    }
                });
            }
            _ => {
                let Some((names, sig)) = line.split_once(" :: ") else {
                    continue;
                };
                for name in names.trim_matches(['[', ']']).split(',') {
                    let name = name.trim().trim_matches(['(', ')']);
                    if name.is_empty() {
                        continue;
                    }
                    let is_field = names.starts_with('[');
                    let constructor = !is_field
                        && line.starts_with(|c: char| c.is_uppercase() || c == '(')
                        && name.starts_with(|c: char| c.is_uppercase() || c == ':');
                    let method = parent
                        .as_ref()
                        .filter(|p| p.is_class && !is_field && !constructor)
                        .filter(|p| sig.starts_with(&format!("{} ", p.name)))
                        .is_some();
                    let (kind, base, from) = match (&parent, is_field, constructor, method) {
                        (Some(p), true, _, _) => (SymbolKind::Field, p.id.as_str(), p.line),
                        (Some(p), _, true, _) => (SymbolKind::EnumVariant, p.id.as_str(), p.line),
                        (Some(p), _, _, true) => (SymbolKind::Method, p.id.as_str(), p.line),
                        _ => (SymbolKind::Function, module_id.as_str(), 1),
                    };
                    let at = locate(current.text, name, from, constructor);
                    let id = qualified(base, name);
                    symbols.push(symbol(
                        &context,
                        current,
                        &id,
                        kind,
                        line.to_owned(),
                        docs.clone(),
                        at,
                    ));
                }
            }
        }
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

fn find_source<'a>(
    sources: &'a [Source],
    texts: &'a BTreeMap<String, String>,
    module: &str,
) -> Result<Module<'a>, String> {
    let wanted = module.replace('.', "/");
    for source in sources {
        for ext in [".hs", ".lhs"] {
            let want = format!("{wanted}{ext}");
            for path in [&source.rel, &source.exec_path] {
                if path == &want || path.ends_with(&format!("/{want}")) {
                    let text = texts.get(&source.exec_path).map_or("", String::as_str);
                    return Ok(Module {
                        name: module.to_owned(),
                        source,
                        text,
                    });
                }
            }
        }
    }
    Err(format!("module '{module}' is not in the srcs manifest"))
}

fn tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\''))
        .filter(|w| !w.is_empty())
}

/// The name a `data`, `newtype`, `type` or `class` line declares.
fn declared_name(words: &[&str], is_class: bool) -> Option<String> {
    let mut rest = words;
    if is_class || words.contains(&"=>") {
        if let Some(at) = words.iter().position(|w| *w == "=>") {
            rest = &words[at + 1..];
        }
    }
    let word = rest.first()?;
    let word = word.trim_matches(['(', ')']);
    (!word.is_empty()).then(|| word.to_owned())
}

/// The 1-based line of the first source line that `matches`, or 1.
fn line_of(text: &str, matches: impl Fn(&str) -> bool) -> usize {
    text.lines().position(matches).map_or(1, |at| at + 1)
}

/// The line of a function, field or constructor `name`, searched from line `from`.
fn locate(text: &str, name: &str, from: usize, constructor: bool) -> usize {
    let start = from.saturating_sub(1);
    let found = text.lines().enumerate().skip(start).find(|(_, line)| {
        if constructor {
            return tokens(line).any(|w| w == name);
        }
        let t = line.trim_start().trim_start_matches(['{', ',']).trim_start();
        let t = t.strip_prefix('(').map_or(t, |inner| inner);
        t.strip_prefix(name).is_some_and(|rest| {
            let rest = rest.trim_start_matches(')').trim_start();
            rest.starts_with("::") || rest.starts_with(',')
        })
    });
    found.map_or(from.max(1), |(at, _)| at + 1)
}

fn qualified(parent: &str, name: &str) -> String {
    let escaped = if parent.is_empty() {
        name.split('.')
            .map(documentation_ir::escape_id_segment)
            .collect::<Vec<_>>()
            .join(".")
    } else {
        documentation_ir::escape_id_segment(name)
    };
    if parent.is_empty() {
        escaped
    } else {
        format!("{parent}.{escaped}")
    }
}

fn symbol(
    context: &Context<'_>,
    module: &Module<'_>,
    id: &str,
    kind: SymbolKind,
    signature: String,
    docs: String,
    line: usize,
) -> Symbol {
    let _ = &module.name;
    Symbol {
        id: format!("{LANGUAGE}:{}:{id}", context.package),
        kind: kind as i32,
        signature_text: signature,
        doc_markdown: docs,
        source: Some(SourceRef {
            file: module.source.rel.clone(),
            line: line.max(1) as u64,
            generated: module.source.generated,
        }),
        visibility: Visibility::Public as i32,
        extensions: vec![Extension {
            key: KEY_CONFIG.to_owned(),
            value: context.config.as_bytes().to_vec(),
        }],
        ..Symbol::default()
    }
}

#[cfg(test)]
mod lib_tests;
