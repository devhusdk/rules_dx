//! Renders DocIr shards into API pages, an index, a SUMMARY fragment and search records.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use documentation_ir::decode_shard;
use documentation_ir::proto::{DocIr, Symbol};

/// The inputs and outputs of one aggregation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub shards: Vec<String>,
    pub source_url: String,
    pub summary: String,
    pub api: String,
    pub api_tree: String,
    pub records: String,
}

/// One rendered output: a path relative to its output root and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub summary: String,
    pub api: String,
    pub pages: BTreeMap<String, String>,
    pub records: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: site_aggregate --shard FILE [--shard FILE] [--source-url URL] --summary FILE --api FILE --api-tree DIR --records FILE"
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
        match flag {
            "--shard" => options.shards.push(value.clone()),
            "--source-url" => options.source_url = value.clone(),
            "--summary" => options.summary = value.clone(),
            "--api" => options.api = value.clone(),
            "--api-tree" => options.api_tree = value.clone(),
            "--records" => options.records = value.clone(),
            _ => return Err(format!("unknown argument '{flag}'")),
        }
        index += 2;
    }
    if options.shards.is_empty() {
        return Err("--shard is required".to_string());
    }
    for (flag, value) in [
        ("--summary", &options.summary),
        ("--api", &options.api),
        ("--api-tree", &options.api_tree),
        ("--records", &options.records),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    Ok(options)
}

/// Returns the API page path of one symbol ID, relative to the book source root.
pub fn api_path(id: &str) -> String {
    format!("api/{}.md", id.replace(':', "/"))
}

/// Returns the reader-facing title of one symbol ID.
pub fn title(id: &str) -> String {
    let mut parts = id.splitn(3, ':');
    let language = parts.next().unwrap_or_default();
    let package = parts.next().unwrap_or_default();
    let qualified = parts.next().unwrap_or_default();
    if language == "rust" {
        let mut path = vec![package.to_string()];
        path.extend(qualified.split('.').map(str::to_string));
        return path.join("::");
    }
    format!("{package}.{qualified}")
}

/// Returns one inline code span of `text`, fenced with more backticks than the text holds.
pub fn code(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest + 1);
    if longest > 0 && (text.starts_with('`') || text.ends_with('`')) {
        format!("{fence} {text} {fence}")
    } else {
        format!("{fence}{text}{fence}")
    }
}

/// Escapes the characters that would end or nest a Markdown link text.
fn link_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '[' | ']' | '<' | '>' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Reader-facing titles: a symbol's display name when its shard sets one, else `title(id)`.
struct Titles<'a> {
    names: BTreeMap<&'a str, &'a str>,
}

impl Titles<'_> {
    fn of(&self, id: &str) -> String {
        match self.names.get(id) {
            Some(name) => (*name).to_string(),
            None => title(id),
        }
    }
}

/// Reads, renders and writes every output, returning "" on success.
pub fn run(options: &Options) -> String {
    let mut shards = Vec::new();
    for path in &options.shards {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => return format!("cannot read shard '{path}': {error}"),
        };
        match decode_shard(&bytes) {
            Ok(shard) => shards.push(shard),
            Err(error) => return format!("shard '{path}' is not a valid DocIr shard: {error:?}"),
        }
    }
    let rendered = match render(&shards, &options.source_url) {
        Ok(rendered) => rendered,
        Err(error) => return error,
    };
    let tree = Path::new(&options.api_tree);
    for (rel, text) in &rendered.pages {
        let path = tree.join(rel);
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return format!("cannot create '{}': {error}", parent.display());
            }
        }
        if let Err(error) = fs::write(&path, text) {
            return format!("cannot write '{}': {error}", path.display());
        }
    }
    for (path, text) in [
        (&options.summary, &rendered.summary),
        (&options.api, &rendered.api),
        (&options.records, &rendered.records),
    ] {
        if let Err(error) = fs::write(path, text) {
            return format!("cannot write '{path}': {error}");
        }
    }
    String::new()
}

struct Page<'a> {
    id: String,
    symbol: &'a Symbol,
    canonical: Option<String>,
}

/// Renders every output of one site's shards.
pub fn render(shards: &[DocIr], source_url: &str) -> Result<Rendered, String> {
    let mut pages: BTreeMap<String, Page> = BTreeMap::new();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for shard in shards {
        let group = format!("{}:{}", shard.language, shard.package);
        for symbol in &shard.symbols {
            let aliases = symbol
                .relations
                .as_ref()
                .map(|r| r.reexported_as.clone())
                .unwrap_or_default();
            let entries = std::iter::once((symbol.id.clone(), None))
                .chain(aliases.into_iter().map(|a| (a, Some(symbol.id.clone()))));
            for (id, canonical) in entries {
                let page = Page {
                    id: id.clone(),
                    symbol,
                    canonical,
                };
                if pages.insert(id.clone(), page).is_some() {
                    return Err(format!("two shards define the API page for '{id}'"));
                }
                groups.entry(group.clone()).or_default().push(id);
            }
        }
    }
    if pages.is_empty() {
        return Err("the shards name no symbols".to_string());
    }
    let known: BTreeSet<&str> = pages.keys().map(String::as_str).collect();
    let titles = Titles {
        names: shards
            .iter()
            .flat_map(|shard| &shard.symbols)
            .filter(|symbol| !symbol.display_name.is_empty())
            .map(|symbol| (symbol.id.as_str(), symbol.display_name.as_str()))
            .collect(),
    };
    let mut rendered = Rendered {
        summary: "- [API](api.md)\n".to_string(),
        api: "# API Reference\n".to_string(),
        pages: BTreeMap::new(),
        records: String::new(),
    };
    let mut records = Vec::new();
    for (group, ids) in &groups {
        rendered
            .api
            .push_str(&format!("\n## {}\n\n", group_title(group)));
        let mut ids = ids.clone();
        ids.sort();
        for id in ids {
            let Some(page) = pages.get(&id) else {
                continue;
            };
            let path = api_path(&id);
            let summary = first_line(&page.symbol.doc_markdown);
            let name = titles.of(&id);
            rendered
                .summary
                .push_str(&format!("    - [{}]({path})\n", link_text(&name)));
            match summary {
                Some(line) => rendered
                    .api
                    .push_str(&format!("- [{}]({path}): {line}\n", code(&name))),
                None => rendered
                    .api
                    .push_str(&format!("- [{}]({path})\n", code(&name))),
            }
            let rel = path.strip_prefix("api/").unwrap_or(&path).to_string();
            rendered
                .pages
                .insert(rel, page_text(page, &known, &titles, source_url));
            records.push(serde_json::json!({
                "body": summary.unwrap_or_default(),
                "title": name,
                "url": format!("{}.html", path.strip_suffix(".md").unwrap_or(&path)),
            }));
        }
    }
    records.sort_by_key(|record| record["url"].as_str().unwrap_or_default().to_string());
    rendered.records = serde_json::to_string_pretty(&records)
        .map_err(|error| format!("cannot encode search records: {error}"))?
        + "\n";
    Ok(rendered)
}

fn group_title(group: &str) -> String {
    match group.split_once(':') {
        Some(("rust", package)) => format!("Rust crate `{package}`"),
        Some(("go", package)) => format!("Go package `{package}`"),
        Some(("ruby", package)) => format!("Ruby library `{package}`"),
        Some((language, package)) => format!("{language} package `{package}`"),
        None => group.to_string(),
    }
}

fn first_line(doc: &str) -> Option<&str> {
    doc.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#') && !line.starts_with("```"))
}

fn page_text(page: &Page, known: &BTreeSet<&str>, titles: &Titles, source_url: &str) -> String {
    let symbol = page.symbol;
    let from = api_path(&page.id);
    let mut text = format!("# {}\n\n", code(&titles.of(&page.id)));
    if let Some(canonical) = &page.canonical {
        text.push_str(&format!(
            "Re-export of {}.\n\n",
            link(&from, canonical, known, titles)
        ));
    }
    if !symbol.signature_text.is_empty() {
        let fence = match page.id.split_once(':') {
            Some(("rust", _)) => "rust,noplayground",
            Some(("go", _)) => "go",
            Some(("ruby", _)) => "ruby",
            _ => "text",
        };
        text.push_str(&format!("```{fence}\n{}\n```\n\n", symbol.signature_text));
    }
    if !symbol.doc_markdown.trim().is_empty() {
        text.push_str(&demote_headings(symbol.doc_markdown.trim_end()));
        text.push_str("\n\n");
    } else {
        text.push_str("Undocumented.\n\n");
    }
    if let Some(source) = &symbol.source {
        let place = format!("{}:{}", source.file, source.line);
        if source.generated || source_url.is_empty() {
            let note = if source.generated { " (generated)" } else { "" };
            text.push_str(&format!("Source: `{place}`{note}\n\n"));
        } else {
            text.push_str(&format!(
                "Source: [{place}]({}{}#L{})\n\n",
                source_url, source.file, source.line
            ));
        }
    }
    if let Some(relations) = &symbol.relations {
        for (label, ids) in [
            ("Member of", &relations.member_of),
            ("Implements", &relations.implements),
            ("Also exported as", &relations.reexported_as),
        ] {
            if page.canonical.is_some() && label == "Also exported as" {
                continue;
            }
            if !ids.is_empty() {
                let links: Vec<String> = ids
                    .iter()
                    .map(|id| link(&from, id, known, titles))
                    .collect();
                text.push_str(&format!("{label}: {}\n\n", links.join(", ")));
            }
        }
    }
    let members: Vec<String> = known
        .iter()
        .filter(|id| {
            id.strip_prefix(symbol.id.as_str())
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|rest| !rest.contains('.'))
        })
        .map(|id| link(&from, id, known, titles))
        .collect();
    if page.canonical.is_none() && !members.is_empty() {
        text.push_str(&format!("Members: {}\n\n", members.join(", ")));
    }
    text.trim_end().to_string() + "\n"
}

fn link(from: &str, id: &str, known: &BTreeSet<&str>, titles: &Titles) -> String {
    let name = code(&titles.of(id));
    if !known.contains(id) {
        return name;
    }
    format!("[{name}]({})", relative(from, &api_path(id)))
}

/// Returns the link from one book page to another, relative to the first page's folder.
pub fn relative(from: &str, to: &str) -> String {
    let from_dirs: Vec<&str> = from.split('/').collect();
    let from_dirs = &from_dirs[..from_dirs.len().saturating_sub(1)];
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dirs
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count()
        .min(to_parts.len().saturating_sub(1));
    let mut parts: Vec<&str> = vec![".."; from_dirs.len() - common];
    parts.extend(&to_parts[common..]);
    parts.join("/")
}

fn demote_headings(doc: &str) -> String {
    let mut fenced = false;
    doc.lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                return line.to_string();
            }
            if !fenced && line.starts_with('#') {
                return format!("##{line}");
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
