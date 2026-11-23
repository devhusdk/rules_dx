//! Checks that every link, anchor and API page a docs site names really exists.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod rendered;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use documentation_ir::tool::{PathIoError, path_io};

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

/// Runs `site_check` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return 0;
    }
    if args.iter().any(|arg| arg == "--rendered") {
        return match parse_rendered(args) {
            Ok((rendered, forbid)) => {
                let findings = rendered::check_rendered(&rendered, &forbid);
                let text: Vec<String> = findings.iter().map(ToString::to_string).collect();
                report(&text.join("\n"))
            }
            Err(detail) => reject(detail, rendered::usage()),
        };
    }
    match parse_args(args) {
        Ok(inputs) => match check(&inputs) {
            Ok(()) => 0,
            Err(error) => report(&error.to_string()),
        },
        Err(detail) => reject(detail, usage()),
    }
}

/// A rejected command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgsError {
    MissingValue {
        flag: String,
        operand: &'static str,
    },
    Unknown {
        flag: String,
    },
    Required {
        flag: &'static str,
        operand: &'static str,
    },
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { flag, operand } => write!(f, "{flag} needs a {operand}"),
            Self::Unknown { flag } => write!(f, "unknown argument '{flag}'"),
            Self::Required { flag, operand } => write!(f, "{flag} {operand} is required"),
        }
    }
}

impl std::error::Error for ArgsError {}

fn parse_rendered(args: &[String]) -> Result<(rendered::Rendered, Vec<String>), ArgsError> {
    let mut rendered = rendered::Rendered::default();
    let mut forbid: Vec<String> = Vec::new();
    let mut index = 0;
    while let Some(flag) = args.get(index).cloned() {
        if flag == "--landing" {
            rendered.landing = true;
            index += 1;
            continue;
        }
        let Some(value) = args.get(index + 1) else {
            return Err(ArgsError::MissingValue {
                flag,
                operand: "DIR",
            });
        };
        match flag.as_str() {
            "--rendered" => rendered.root = std::path::PathBuf::from(value),
            "--forbid" => forbid.push(value.clone()),
            _ => return Err(ArgsError::Unknown { flag }),
        }
        index += 2;
    }
    if rendered.root.as_os_str().is_empty() {
        return Err(ArgsError::Required {
            flag: "--rendered",
            operand: "DIR",
        });
    }
    Ok((rendered, forbid))
}

fn report(refusal: &str) -> i32 {
    if refusal.is_empty() {
        return 0;
    }
    eprintln!("docs_site: {refusal}");
    1
}

fn reject(detail: impl std::fmt::Display, usage: impl std::fmt::Display) -> i32 {
    eprintln!("site_check: {detail}\n{usage}");
    2
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> String {
    let mut line = String::from("usage: site_check --book FILE --api FILE --shard FILE");
    line.push_str(" [--shard FILE] [--prose FILE] [--data FILE] [--records FILE]");
    line
}

/// Parses argv into inputs.
///
/// # Errors
///
/// Returns an error for an unknown flag, a flag without a value or a missing required flag.
pub fn parse_args(args: &[String]) -> Result<Inputs, ArgsError> {
    let mut inputs = Inputs::default();
    let mut index = 0;
    while let Some(flag) = args.get(index).map(String::as_str) {
        match flag {
            "--book" | "--api" | "--records" | "--shard" | "--prose" | "--data" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(ArgsError::MissingValue {
                        flag: flag.to_owned(),
                        operand: "FILE",
                    });
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
                return Err(ArgsError::Unknown {
                    flag: flag.to_owned(),
                });
            }
        }
        index += 2;
    }
    for (flag, missing) in [
        ("--book", inputs.book.is_none()),
        ("--api", inputs.api.is_none()),
        ("--shard", inputs.shards.is_empty()),
        ("--prose", inputs.prose.is_empty()),
    ] {
        if missing {
            return Err(ArgsError::Required {
                flag,
                operand: "FILE",
            });
        }
    }
    Ok(inputs)
}

/// Why a docs site does not hold together.
#[derive(Debug)]
pub enum CheckError {
    Read(PathIoError),
    NoBookTitle,
    ProseTitle {
        path: String,
    },
    InvalidShard {
        path: String,
        error: documentation_ir::Error,
    },
    NoSymbols,
    MissingApiPage {
        id: String,
    },
    RecordUrl {
        url: String,
    },
    EmptyLinkTarget,
    DanglingAnchor {
        target: String,
    },
    DanglingLink {
        target: String,
    },
    DanglingFragment {
        target: String,
    },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::NoBookTitle => f.write_str("book has no title"),
            Self::ProseTitle { path } => write!(f, "prose missing title '{path}'"),
            Self::InvalidShard { path, error } => {
                write!(f, "shard '{path}' is not a valid DocIr shard: {error:?}")
            }
            Self::NoSymbols => f.write_str("shard names no symbols"),
            Self::MissingApiPage { id } => write!(f, "missing API page for {id}"),
            Self::RecordUrl { url } => write!(f, "search record url '{url}' has no rendered page"),
            Self::EmptyLinkTarget => f.write_str("empty link target"),
            Self::DanglingAnchor { target } => write!(f, "dangling anchor '{target}'"),
            Self::DanglingLink { target } => write!(f, "dangling link '{target}'"),
            Self::DanglingFragment { target } => write!(f, "dangling fragment '{target}'"),
        }
    }
}

impl std::error::Error for CheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::NoBookTitle
            | Self::ProseTitle { .. }
            | Self::InvalidShard { .. }
            | Self::NoSymbols
            | Self::MissingApiPage { .. }
            | Self::RecordUrl { .. }
            | Self::EmptyLinkTarget
            | Self::DanglingAnchor { .. }
            | Self::DanglingLink { .. }
            | Self::DanglingFragment { .. } => None,
        }
    }
}

/// Checks that the site holds together.
///
/// # Errors
///
/// Returns the first input that cannot be read or the first refusal found.
pub fn check(inputs: &Inputs) -> Result<(), CheckError> {
    let book_text = read(inputs.book.as_deref().unwrap_or_default())?;
    let prose_texts = inputs
        .prose
        .iter()
        .map(|p| read(p))
        .collect::<Result<Vec<_>, _>>()?;
    let api_text = read(inputs.api.as_deref().unwrap_or_default())?;
    let pages = || prose_texts.iter().map(String::as_str).collect::<Vec<_>>();

    if !any_line(&book_text, |line| line.starts_with("title")) {
        return Err(CheckError::NoBookTitle);
    }
    for (path, text) in inputs.prose.iter().zip(&prose_texts) {
        if !any_line(text, |line| line.starts_with("# ")) {
            return Err(CheckError::ProseTitle { path: path.clone() });
        }
    }

    let mut ids = BTreeSet::new();
    for shard in &inputs.shards {
        let bytes = path_io::read(Path::new(shard)).map_err(CheckError::Read)?;
        let decoded = match documentation_ir::decode_shard(&bytes) {
            Ok(decoded) => decoded,
            Err(error) => {
                return Err(CheckError::InvalidShard {
                    path: shard.clone(),
                    error,
                });
            }
        };
        for symbol in decoded.symbols {
            if let Some(relations) = &symbol.relations {
                ids.extend(relations.reexported_as.iter().cloned());
            }
            ids.insert(symbol.id);
        }
    }
    if ids.is_empty() {
        return Err(CheckError::NoSymbols);
    }
    let id_paths: Vec<(&String, String)> = ids
        .iter()
        .map(|id| (id, format!("api/{}.md", id.replace(':', "/"))))
        .collect();
    for (id, path) in &id_paths {
        if !api_text.contains(&format!("({path})")) {
            return Err(CheckError::MissingApiPage { id: (*id).clone() });
        }
    }
    let api_paths: BTreeSet<String> = id_paths.into_iter().map(|(_, path)| path).collect();

    if let Some(records) = &inputs.records {
        for url in record_urls(&read(records)?) {
            let page = url.strip_suffix(".html").map(|stem| format!("{stem}.md"));
            if !page.is_some_and(|page| api_paths.contains(&page)) {
                return Err(CheckError::RecordUrl { url });
            }
        }
    }

    if pages().iter().any(|text| has_empty_link_target(text)) {
        return Err(CheckError::EmptyLinkTarget);
    }

    let site = Site::new(inputs, &prose_texts, &api_text, &api_paths);
    for (source, target) in link_targets(&inputs.prose, &prose_texts, &site.root) {
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        if let Some(fragment) = target.strip_prefix('#') {
            if !site.has_anchor(&source, fragment) {
                return Err(CheckError::DanglingAnchor { target });
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
            return Err(CheckError::DanglingLink { target });
        };
        if !fragment.is_empty() && !site.has_anchor(&route, fragment) {
            return Err(CheckError::DanglingFragment { target });
        }
    }
    Ok(())
}

fn record_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("\"url\"") {
        rest = after;
        let Some(open) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        let Some(open) = open.trim_start().strip_prefix('"') else {
            continue;
        };
        let value = open.split_once('"').map_or(open, |(value, _)| value);
        urls.push(value.to_string());
    }
    urls
}

fn read(path: &str) -> Result<String, CheckError> {
    path_io::read_to_string(Path::new(path)).map_err(CheckError::Read)
}

fn any_line(text: &str, wanted: impl Fn(&str) -> bool) -> bool {
    text.lines().any(wanted)
}

fn has_empty_link_target(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, window) in bytes.windows(3).enumerate() {
        let [b']', b'(', b')'] = window else {
            continue;
        };
        for byte in bytes.iter().take(index).rev() {
            match byte {
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

fn common_root(paths: &[String]) -> String {
    let mut dirs = paths.iter().map(|p| {
        let mut parts: Vec<&str> = p.split('/').collect();
        parts.pop();
        parts
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

fn route_under(root: &str, path: &str) -> String {
    if root.is_empty() {
        return path.to_string();
    }
    path.strip_prefix(root)
        .map_or(path, |rest| rest.trim_start_matches('/'))
        .to_string()
}

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
        let mut fenced = false;
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
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
        let (Some(b']'), Some(b'(')) = (bytes.get(index), bytes.get(index + 1)) else {
            index += 1;
            continue;
        };
        let mut opened = false;
        for byte in bytes.iter().take(index).rev() {
            match byte {
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
        let Some((target, _)) = line.get(start..).and_then(|tail| tail.split_once(')')) else {
            break;
        };
        found.push(target.to_string());
        index = start + target.len();
    }
    found
}

fn split_reference(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let (head, tail) = trimmed.split_once(']')?;
    let value = tail.strip_prefix(':')?;
    Some((head.len() + 1, value))
}

fn tidy(value: &str) -> String {
    let trimmed = value.trim();
    let unbracketed = trimmed.strip_prefix('<').unwrap_or(trimmed);
    let unbracketed = unbracketed.strip_suffix('>').unwrap_or(unbracketed);
    if unbracketed.starts_with('"') {
        return String::new();
    }
    let cut = unbracketed
        .split_once('"')
        .map_or(unbracketed, |(head, _)| head);
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
