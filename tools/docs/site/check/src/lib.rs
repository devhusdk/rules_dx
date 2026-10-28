//! Checks that every link, anchor and API page a docs site names really exists.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod rendered;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The required and repeated inputs for one aggregate.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub book: Option<String>,
    pub api: Option<String>,
    pub records: Option<String>,
    pub shards: Vec<String>,
    pub prose: Vec<String>,
    pub data: Vec<String>,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> String {
    let mut line = String::from("usage: site_check --book FILE --api FILE --shard FILE");
    line.push_str(" [--shard FILE] [--prose FILE] [--data FILE] [--records FILE]");
    line
}

/// Parses argv into inputs, or returns the usage rejection text.
pub fn parse_args(args: &[String]) -> Result<Inputs, (String, String)> {
    let mut inputs = Inputs::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--book" | "--api" | "--records" | "--shard" | "--prose" | "--data" => {
                let Some(value) = args.get(index + 1) else {
                    return Err((format!("{flag} needs a FILE"), usage()));
                };
                match flag {
                    "--book" => inputs.book = Some(value.clone()),
                    "--api" => inputs.api = Some(value.clone()),
                    "--records" => inputs.records = Some(value.clone()),
                    "--shard" => inputs.shards.push(value.clone()),
                    "--prose" => inputs.prose.push(value.clone()),
                    _ => inputs.data.push(value.clone()),
                }
            }
            _ => {
                return Err((format!("unknown argument '{flag}'"), usage()));
            }
        }
        index += 2;
    }
    if inputs.book.is_none() {
        return Err(("--book FILE is required".to_string(), usage()));
    }
    if inputs.api.is_none() {
        return Err(("--api FILE is required".to_string(), usage()));
    }
    if inputs.shards.is_empty() {
        return Err(("--shard FILE is required".to_string(), usage()));
    }
    if inputs.prose.is_empty() {
        return Err(("--prose FILE is required".to_string(), usage()));
    }
    Ok(inputs)
}

/// Returns the check refusal text, or an empty string when the site holds together.
pub fn check(inputs: &Inputs) -> String {
    let book_text = read(inputs.book.as_deref().unwrap_or_default());
    let api = inputs.api.as_deref().unwrap_or_default();
    let prose_texts: Vec<String> = inputs.prose.iter().map(|p| read(p)).collect();
    let api_text = read(api);
    let pages = || prose_texts.iter().map(String::as_str).collect::<Vec<_>>();

    if !any_line(&book_text, |line| line.starts_with("title")) {
        return "book has no title".to_string();
    }
    for (path, text) in inputs.prose.iter().zip(&prose_texts) {
        if !any_line(text, |line| line.starts_with("# ")) {
            return format!("prose missing title '{path}'");
        }
    }

    let mut ids = BTreeSet::new();
    for shard in &inputs.shards {
        let bytes = std::fs::read(Path::new(shard)).unwrap_or_default();
        let shard = match documentation_ir::decode_shard(&bytes) {
            Ok(shard) => shard,
            Err(error) => return format!("shard '{shard}' is not a valid DocIr shard: {error:?}"),
        };
        for symbol in shard.symbols {
            if let Some(relations) = &symbol.relations {
                ids.extend(relations.reexported_as.iter().cloned());
            }
            ids.insert(symbol.id);
        }
    }
    if ids.is_empty() {
        return "shard names no symbols".to_string();
    }
    let id_paths: Vec<(&String, String)> = ids
        .iter()
        .map(|id| (id, format!("api/{}.md", id.replace(':', "/"))))
        .collect();
    for (id, path) in &id_paths {
        if !api_text.contains(&format!("({path})")) {
            return format!("missing API page for {id}");
        }
    }
    let api_paths: BTreeSet<String> = id_paths.into_iter().map(|(_, path)| path).collect();

    if let Some(records) = &inputs.records {
        for url in record_urls(&read(records)) {
            let page = url.strip_suffix(".html").map(|stem| format!("{stem}.md"));
            if !page.is_some_and(|page| api_paths.contains(&page)) {
                return format!("search record url '{url}' has no rendered page");
            }
        }
    }

    if pages().iter().any(|text| has_empty_link_target(text)) {
        return "empty link target".to_string();
    }

    let site = Site::new(inputs, &prose_texts, &api_text, &api_paths);
    for (source, target) in link_targets(&inputs.prose, &prose_texts, &site.root) {
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        if let Some(fragment) = target.strip_prefix('#') {
            if !site.has_anchor(&source, fragment) {
                return format!("dangling anchor '{target}'");
            }
            continue;
        }
        let (base, fragment) = split_fragment(&target);
        let (base, directory) = match base.strip_suffix('/') {
            Some(trimmed) => (trimmed, true),
            None => (base, false),
        };
        let resolved = resolve_route(&source, base).and_then(|route| {
            let route = if directory {
                if route.is_empty() {
                    "README.md".to_string()
                } else {
                    format!("{route}/README.md")
                }
            } else {
                route
            };
            let known = if directory {
                site.prose.contains(&route)
            } else {
                site.pages.contains_key(&route)
            };
            known.then_some(route)
        });
        let Some(route) = resolved else {
            return format!("dangling link '{target}'");
        };
        if !fragment.is_empty() && !site.has_anchor(&route, fragment) {
            return format!("dangling fragment '{target}'");
        }
    }
    String::new()
}

/// The `url` string values of a search records JSON array.
fn record_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"url\"") {
        rest = &rest[at + 5..];
        let Some(open) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        let Some(open) = open.trim_start().strip_prefix('"') else {
            continue;
        };
        let end = open.find('"').unwrap_or(open.len());
        urls.push(open[..end].to_string());
    }
    urls
}

fn read(path: &str) -> String {
    std::fs::read_to_string(Path::new(path)).unwrap_or_default()
}

fn any_line(text: &str, wanted: impl Fn(&str) -> bool) -> bool {
    text.lines().any(wanted)
}

fn has_empty_link_target(text: &str) -> bool {
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b']' || index + 2 >= bytes.len() {
            continue;
        }
        if bytes[index + 1] != b'(' || bytes[index + 2] != b')' {
            continue;
        }
        let mut back = index;
        while back > 0 {
            back -= 1;
            match bytes[back] {
                b']' => break,
                b'[' => return true,
                _ => {}
            }
        }
    }
    false
}

fn text_anchors(text: &str, prefix: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|l| l.starts_with(prefix))
        .map(anchor_name)
        .collect()
}

/// The pages of one site, keyed by their route relative to the book root.
struct Site {
    root: String,
    prose: BTreeSet<String>,
    pages: BTreeMap<String, BTreeSet<String>>,
}

impl Site {
    fn new(
        inputs: &Inputs,
        prose_texts: &[String],
        api_text: &str,
        api_paths: &BTreeSet<String>,
    ) -> Self {
        let root = common_root(&inputs.prose);
        let mut prose = BTreeSet::new();
        let mut pages = BTreeMap::new();
        for (path, text) in inputs.prose.iter().zip(prose_texts) {
            let route = route_under(&root, path);
            prose.insert(route.clone());
            pages.insert(route, text_anchors(text, "#"));
        }
        for path in &inputs.data {
            pages.insert(route_under(&root, path), BTreeSet::new());
        }
        for path in api_paths {
            pages.insert(path.clone(), BTreeSet::new());
        }
        pages.insert("api.md".to_string(), text_anchors(api_text, "## "));
        pages.insert("SUMMARY.md".to_string(), BTreeSet::new());
        Site { root, prose, pages }
    }

    fn has_anchor(&self, route: &str, fragment: &str) -> bool {
        self.pages.get(route).is_some_and(|anchors| {
            anchors.contains(fragment)
                || rendered::decode_fragment(fragment).is_some_and(|d| anchors.contains(&d))
        })
    }
}

/// Returns the longest directory that holds every path, "" when there is none.
fn common_root(paths: &[String]) -> String {
    let mut dirs = paths.iter().map(|p| {
        let parts: Vec<&str> = p.split('/').collect();
        parts[..parts.len() - 1].to_vec()
    });
    let Some(mut common) = dirs.next() else {
        return String::new();
    };
    for dir in dirs {
        let shared = common.iter().zip(&dir).take_while(|(a, b)| a == b).count();
        common.truncate(shared);
    }
    common.join("/")
}

/// Returns a path relative to the root directory.
fn route_under(root: &str, path: &str) -> String {
    if root.is_empty() {
        return path.to_string();
    }
    path.strip_prefix(root)
        .map(|rest| rest.trim_start_matches('/'))
        .unwrap_or(path)
        .to_string()
}

/// Joins a link target to the directory of its page, or None above the root.
fn resolve_route(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = match from.rsplit_once('/') {
        Some((dir, _)) => dir.split('/').collect(),
        None => Vec::new(),
    };
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// Ports mdBook 0.4.52 `id_from_content` and `normalize_id` for a heading line.
fn anchor_name(line: &str) -> String {
    let heading = line.trim().trim_start_matches('#').trim().replace('`', "");
    heading
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn link_targets(paths: &[String], prose: &[String], root: &str) -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for (path, text) in paths.iter().zip(prose) {
        let source = route_under(root, path);
        for line in text.lines() {
            for inner in inline_targets(line) {
                let tidied = tidy(&inner);
                found.insert((source.clone(), first_field(tidied.as_str())));
            }
            if let Some((_, rest)) = split_reference(line) {
                found.insert((
                    source.clone(),
                    first_field(&trim_wrapping(rest.trim_start())),
                ));
            }
        }
    }
    found
}

fn inline_targets(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b']' || index + 1 >= bytes.len() || bytes[index + 1] != b'(' {
            index += 1;
            continue;
        }
        let mut back = index;
        let mut opened = false;
        while back > 0 {
            back -= 1;
            match bytes[back] {
                b']' => break,
                b'[' => {
                    opened = true;
                    break;
                }
                _ => {}
            }
        }
        if !opened {
            index += 1;
            continue;
        }
        let start = index + 2;
        match line[start..].find(')') {
            Some(offset) => {
                found.push(line[start..start + offset].to_string());
                index = start + offset;
            }
            None => break,
        }
    }
    found
}

fn split_reference(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let close = trimmed.find(']')?;
    if !trimmed[close + 1..].starts_with(':') {
        return None;
    }
    Some((close + 1, &trimmed[close + 2..]))
}

fn tidy(value: &str) -> String {
    let trimmed = value.trim();
    let unbracketed = trimmed.strip_prefix('<').unwrap_or(trimmed);
    let unbracketed = unbracketed.strip_suffix('>').unwrap_or(unbracketed);
    if unbracketed.starts_with('"') {
        return String::new();
    }
    let cut = match unbracketed.find('"') {
        Some(at) => &unbracketed[..at],
        None => unbracketed,
    };
    cut.trim().to_string()
}

fn trim_wrapping(value: &str) -> String {
    let unbracketed = value.strip_prefix('<').unwrap_or(value);
    let unbracketed = unbracketed.strip_suffix('>').unwrap_or(unbracketed);
    let unquoted = unbracketed.strip_prefix('"').unwrap_or(unbracketed);
    unquoted.strip_suffix('"').unwrap_or(unquoted).to_string()
}

fn first_field(value: &str) -> String {
    value.split(' ').next().unwrap_or("").to_string()
}

fn split_fragment(target: &str) -> (&str, &str) {
    match target.split_once('#') {
        Some((base, fragment)) => (base, fragment),
        None => (target, ""),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
