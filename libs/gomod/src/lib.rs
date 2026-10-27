//! One reader for `go.mod` and `go.sum`, shared by every Go dependency tool.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;

use gomod_parser::{GoMod, ModuleReplacement, Replacement};

const DIRECTIVES: [&str; 10] = [
    "exclude",
    "godebug",
    "go",
    "ignore",
    "module",
    "replace",
    "require",
    "retract",
    "tool",
    "toolchain",
];

/// One `require` entry with the trailing comment its line carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoRequirement {
    pub module: String,
    pub version: String,
    pub indirect: bool,
    pub marker: Option<String>,
}

/// What a `replace` directive redirects a module to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoReplacementTarget {
    FilePath(String),
    Module { module: String, version: String },
}

/// One `replace` directive, keyed by the module path it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoReplacement {
    pub module: String,
    pub version: Option<String>,
    pub target: GoReplacementTarget,
}

/// The `go.mod` directives a dependency reader needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoModFile {
    pub module: String,
    pub go: Option<String>,
    pub require: Vec<GoRequirement>,
    pub replace: Vec<GoReplacement>,
}

impl GoModFile {
    /// Finds the replacement that applies to one required module version.
    pub fn replacement_for(&self, module: &str, version: &str) -> Option<&GoReplacement> {
        let mut wildcard = None;
        for entry in self.replace.iter().filter(|entry| entry.module == module) {
            match entry.version.as_deref() {
                Some(old) if old == version => return Some(entry),
                Some(_) => {}
                None => wildcard = Some(entry),
            }
        }
        wildcard
    }
}

/// Reads a `go.mod`, failing on syntax the Go toolchain rejects.
pub fn parse_mod(text: &str) -> Result<GoModFile, String> {
    let padded = pad_glued_comments(text);
    let parsed = GoMod::from_str(&padded).map_err(|detail| format!("invalid go.mod: {detail}"))?;
    let mut markers = requirement_markers(text);
    let require = parsed
        .require
        .into_iter()
        .map(|entry| {
            let module = unquote(&entry.module.module_path);
            let marker = markers.get_mut(&module).and_then(VecDeque::pop_front);
            GoRequirement {
                marker,
                module,
                version: unquote(&entry.module.version),
                indirect: entry.indirect,
            }
        })
        .collect();
    let replace: Vec<GoReplacement> = parsed.replace.into_iter().map(replacement).collect();
    for (index, entry) in replace.iter().enumerate() {
        if replace[..index]
            .iter()
            .any(|earlier| earlier.module == entry.module && earlier.version == entry.version)
        {
            return Err(match &entry.version {
                Some(version) => format!(
                    "invalid go.mod: multiple replacements for {} {version}",
                    entry.module
                ),
                None => format!("invalid go.mod: multiple replacements for {}", entry.module),
            });
        }
    }
    Ok(GoModFile {
        module: parsed.module,
        go: parsed.go,
        require,
        replace,
    })
}

/// Reads a `go.sum` into module paths and their highest hashed version.
// go.sum is a line grammar (`module version[/go.mod] hash`), so it is split by line, not parsed.
pub fn parse_sum(text: &str) -> BTreeMap<String, String> {
    let mut hashed: BTreeMap<String, String> = BTreeMap::new();
    let mut manifest_only: BTreeMap<String, String> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let (Some(module), Some(version)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        let (version, map) = match version.strip_suffix("/go.mod") {
            Some(version) => (version, &mut manifest_only),
            None => (version, &mut hashed),
        };
        match map.get_mut(module) {
            Some(best) => {
                if compare_go_versions(version, best) == Ordering::Greater {
                    version.clone_into(best);
                }
            }
            None => {
                map.insert(module.to_owned(), version.to_owned());
            }
        }
    }
    for (module, version) in manifest_only {
        hashed.entry(module).or_insert(version);
    }
    hashed
}

/// Parsed `vMAJOR.MINOR.PATCH[-pre][+build]`.
struct GoVersion<'a> {
    numbers: [u64; 3],
    pre: Vec<&'a str>,
}

fn parse_go_version(text: &str) -> Option<GoVersion<'_>> {
    let text = text.strip_prefix('v')?;
    let text = text.split('+').next().unwrap_or(text);
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, pre.split('.').collect()),
        None => (text, Vec::new()),
    };
    let mut numbers = [0u64; 3];
    let mut parts = core.split('.');
    for (index, slot) in numbers.iter_mut().enumerate() {
        match parts.next() {
            Some(part) => *slot = part.parse().ok()?,
            None if index > 0 => break,
            None => return None,
        }
    }
    if parts.next().is_some() {
        return None;
    }
    Some(GoVersion { numbers, pre })
}

fn compare_pre_part(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// Orders Go module versions by semver; a version that does not parse sorts lowest.
fn compare_go_versions(a: &str, b: &str) -> Ordering {
    match (parse_go_version(a), parse_go_version(b)) {
        (None, None) => a.cmp(b),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => {
            a.numbers
                .cmp(&b.numbers)
                .then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => {
                        for (x, y) in a.pre.iter().zip(&b.pre) {
                            let order = compare_pre_part(x, y);
                            if order != Ordering::Equal {
                                return order;
                            }
                        }
                        a.pre.len().cmp(&b.pre.len())
                    }
                })
        }
    }
}

/// Why a `go.mod` requirement could not be edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoEditError {
    Invalid(String),
    NotFound,
    Ambiguous(usize),
}

/// Sets the version of the one `require` entry for `module`, leaving every other byte alone.
pub fn set_require_version(text: &str, module: &str, version: &str) -> Result<String, GoEditError> {
    let before = parse_mod(text).map_err(GoEditError::Invalid)?;
    let count = before
        .require
        .iter()
        .filter(|requirement| requirement.module == module)
        .count();
    match count {
        0 => return Err(GoEditError::NotFound),
        1 => {}
        count => return Err(GoEditError::Ambiguous(count)),
    }
    let span = locate_require_version(text, module).ok_or_else(|| {
        GoEditError::Invalid(format!("cannot locate the require entry for {module:?}"))
    })?;
    let mut out = String::with_capacity(text.len() + version.len());
    out.push_str(&text[..span.start]);
    out.push_str(&span.quote.render(version));
    out.push_str(&text[span.end..]);
    let after = parse_mod(&out).map_err(GoEditError::Invalid)?;
    let mut expected = before;
    for requirement in &mut expected.require {
        if requirement.module == module {
            requirement.version = version.to_owned();
        }
    }
    if after != expected {
        return Err(GoEditError::Invalid(format!(
            "version {version:?} does not produce a valid requirement"
        )));
    }
    Ok(out)
}

/// Adds a direct `require` entry for `module`, leaving every other byte alone.
/// A module that is already required fails with `Ambiguous(count)`.
///
/// The entry goes into the first `require ( ... )` block that holds a direct
/// requirement, in sorted position; without one, a new block is appended.
pub fn insert_require(text: &str, module: &str, version: &str) -> Result<String, GoEditError> {
    let before = parse_mod(text).map_err(GoEditError::Invalid)?;
    let count = before
        .require
        .iter()
        .filter(|requirement| requirement.module == module)
        .count();
    if count > 0 {
        return Err(GoEditError::Ambiguous(count));
    }
    if module.is_empty()
        || version.is_empty()
        || [module, version].iter().any(|part| {
            part.chars()
                .any(|c| c.is_whitespace() || matches!(c, '"' | '`' | '(' | ')'))
                || part.contains("//")
        })
    {
        return Err(GoEditError::Invalid(format!(
            "{module:?} {version:?} is not a plain requirement"
        )));
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let out = match direct_require_block(text) {
        Some(block) => {
            let at = block
                .entries
                .iter()
                .find(|(_, name)| name.as_str() > module)
                .map_or(block.close, |(start, _)| *start);
            let entry = format!("{}{module} {version}{eol}", block.indent);
            format!("{}{entry}{}", &text[..at], &text[at..])
        }
        None => {
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(eol);
            }
            if !out.trim().is_empty() {
                out.push_str(eol);
            }
            out.push_str(&format!("require ({eol}\t{module} {version}{eol}){eol}"));
            out
        }
    };
    let after = parse_mod(&out).map_err(GoEditError::Invalid)?;
    let mut expected = before.require.clone();
    expected.push(GoRequirement {
        module: module.to_owned(),
        version: version.to_owned(),
        indirect: false,
        marker: None,
    });
    let mut got = after.require;
    let key = |r: &GoRequirement| (r.module.clone(), r.version.clone(), r.indirect);
    expected.sort_by_key(key);
    got.sort_by_key(key);
    if got != expected || after.replace != before.replace || after.module != before.module {
        return Err(GoEditError::Invalid(format!(
            "version {version:?} does not produce a valid requirement"
        )));
    }
    Ok(out)
}

struct RequireBlock {
    /// Start offset and module path of every entry line.
    entries: Vec<(usize, String)>,
    /// Start offset of the closing `)` line.
    close: usize,
    indent: String,
}

/// The first `require ( ... )` block holding at least one direct requirement.
fn direct_require_block(text: &str) -> Option<RequireBlock> {
    let mut base = 0;
    let mut current: Option<(RequireBlock, bool)> = None;
    let mut other_block = false;
    for line in text.split_inclusive('\n') {
        let start = base;
        base += line.len();
        let tokens = tokenize(line, start);
        let Some(first) = tokens.first() else {
            continue;
        };
        if other_block {
            if first.value == ")" {
                other_block = false;
            }
            continue;
        }
        if let Some((block, direct)) = current.as_mut() {
            if first.value == ")" {
                block.close = start;
                if *direct {
                    return current.map(|(block, _)| block);
                }
                current = None;
                continue;
            }
            let indirect = line
                .split_once("//")
                .is_some_and(|(_, comment)| comment.trim().starts_with("indirect"));
            if !indirect {
                *direct = true;
            }
            if block.entries.is_empty() {
                block.indent = line[..line.len() - line.trim_start().len()].to_owned();
            }
            block.entries.push((start, first.value.clone()));
            continue;
        }
        if tokens.get(1).map(|token| token.value.as_str()) == Some("(") {
            if first.value == "require" && tokens.len() == 2 {
                current = Some((
                    RequireBlock {
                        entries: Vec::new(),
                        close: 0,
                        indent: "\t".to_owned(),
                    },
                    false,
                ));
            } else {
                other_block = true;
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
enum Quote {
    Bare,
    Double,
    Raw,
}

impl Quote {
    fn render(self, version: &str) -> String {
        match self {
            Quote::Bare => version.to_owned(),
            Quote::Double => format!("\"{version}\""),
            Quote::Raw => format!("`{version}`"),
        }
    }
}

struct Token {
    start: usize,
    end: usize,
    value: String,
    quote: Quote,
}

struct VersionSpan {
    start: usize,
    end: usize,
    quote: Quote,
}

// Framing only: locates byte spans in lines `gomod-parser` already read; it does not decide entries.
fn tokenize(line: &str, base: usize) -> Vec<Token> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
        } else if byte == b'/' && bytes.get(at + 1) == Some(&b'/') {
            break;
        } else if byte == b'(' || byte == b')' {
            tokens.push(Token {
                start: base + at,
                end: base + at + 1,
                value: (byte as char).to_string(),
                quote: Quote::Bare,
            });
            at += 1;
        } else if byte == b'"' || byte == b'`' {
            let mut end = at + 1;
            while end < bytes.len() && bytes[end] != byte {
                if byte == b'"' && bytes[end] == b'\\' {
                    end += 1;
                }
                end += 1;
            }
            let end = (end + 1).min(bytes.len());
            let inner_end = if end > at + 1 && bytes[end - 1] == byte {
                end - 1
            } else {
                end
            };
            tokens.push(Token {
                start: base + at,
                end: base + end,
                value: line[at + 1..inner_end].to_owned(),
                quote: if byte == b'"' {
                    Quote::Double
                } else {
                    Quote::Raw
                },
            });
            at = end;
        } else {
            let mut end = at;
            while end < bytes.len()
                && !bytes[end].is_ascii_whitespace()
                && !matches!(bytes[end], b'(' | b')' | b',')
                && !(bytes[end] == b'/' && bytes.get(end + 1) == Some(&b'/'))
            {
                end += 1;
            }
            let end = end.max(at + 1);
            tokens.push(Token {
                start: base + at,
                end: base + end,
                value: line[at..end].to_owned(),
                quote: Quote::Bare,
            });
            at = end;
        }
    }
    tokens
}

/// Name and version tokens of every `require` entry.
// Framing only: spans for editing; `gomod-parser` is the reader of record.
fn require_entries(text: &str) -> Vec<(Token, Token)> {
    let mut block: Option<String> = None;
    let mut found = Vec::new();
    let mut base = 0;
    for line in text.split_inclusive('\n') {
        let tokens = tokenize(line, base);
        base += line.len();
        let Some(first) = tokens.first() else {
            continue;
        };
        let is_require = if let Some(keyword) = &block {
            if first.value == ")" {
                block = None;
                continue;
            }
            keyword == "require"
        } else {
            if tokens.get(1).map(|token| token.value.as_str()) == Some("(") {
                block = Some(first.value.clone());
                continue;
            }
            first.value == "require"
        };
        if !is_require {
            continue;
        }
        let skip = usize::from(block.is_none());
        let mut entry = tokens.into_iter().skip(skip);
        if let (Some(name), Some(version)) = (entry.next(), entry.next()) {
            found.push((name, version));
        }
    }
    found
}

/// Inserts a space where a require version runs straight into `//`, which
/// `gomod-parser` would otherwise read as part of the version.
// Framing only: shim owned by #4008; delete it once gomod-parser handles glued comments.
fn pad_glued_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut copied = 0;
    for (_, version) in require_entries(text) {
        if text.get(version.end..version.end + 2) == Some("//") {
            out.push_str(&text[copied..version.end]);
            out.push(' ');
            copied = version.end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

fn locate_require_version(text: &str, module: &str) -> Option<VersionSpan> {
    let mut found = None;
    for (name, version) in require_entries(text) {
        if name.value == module {
            if found.is_some() {
                return None;
            }
            found = Some(VersionSpan {
                start: version.start,
                end: version.end,
                quote: version.quote,
            });
        }
    }
    found
}

fn unquote(token: &str) -> String {
    for quote in ['"', '`'] {
        if let Some(inner) = token
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.to_owned();
        }
    }
    token.to_owned()
}

fn replacement(entry: ModuleReplacement) -> GoReplacement {
    let target = match entry.replacement {
        Replacement::FilePath(path) => GoReplacementTarget::FilePath(path),
        Replacement::Module(module) => GoReplacementTarget::Module {
            module: module.module_path,
            version: module.version,
        },
    };
    GoReplacement {
        module: entry.module_path,
        version: entry.version,
        target,
    }
}

/// Pairs every `require` line with the comment that line carries.
// Framing only: `gomod-parser` is the reader of record for the require list; see #4008.
fn requirement_markers(text: &str) -> BTreeMap<String, VecDeque<String>> {
    let mut markers: BTreeMap<String, VecDeque<String>> = BTreeMap::new();
    let mut block: Option<&str> = None;
    for line in text.lines() {
        let (code, comment) = line.split_once("//").unwrap_or((line, ""));
        let comment = comment.trim();
        let tokens = code.split_whitespace().collect::<Vec<_>>();
        let Some(keyword) = tokens.first().copied() else {
            continue;
        };
        if keyword == ")" {
            block = None;
            continue;
        }
        if DIRECTIVES.contains(&keyword) {
            block = if tokens.get(1) == Some(&"(") {
                Some(keyword)
            } else {
                None
            };
            if keyword == "require" && tokens.len() > 2 {
                record_marker(&mut markers, tokens[1], comment);
            }
            continue;
        }
        if block == Some("require") && tokens.len() == 2 {
            record_marker(&mut markers, tokens[0], comment);
        }
    }
    markers
}

fn record_marker(markers: &mut BTreeMap<String, VecDeque<String>>, module: &str, comment: &str) {
    if comment.is_empty() {
        return;
    }
    markers
        .entry(module.to_owned())
        .or_default()
        .push_back(comment.to_owned());
}

#[cfg(test)]
mod tests;
