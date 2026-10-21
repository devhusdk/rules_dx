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

/// Reads a `go.mod`, failing on syntax the Go toolchain rejects.
pub fn parse_mod(text: &str) -> Result<GoModFile, String> {
    let parsed = GoMod::from_str(text).map_err(|detail| format!("invalid go.mod: {detail}"))?;
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
    let replace = parsed.replace.into_iter().map(replacement).collect();
    Ok(GoModFile {
        module: parsed.module,
        go: parsed.go,
        require,
        replace,
    })
}

/// Reads a `go.sum` into module paths and versions, keeping the first of each pair.
pub fn parse_sum(text: &str) -> BTreeMap<String, String> {
    let mut packages = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let (Some(module), Some(version)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        let version = version.strip_suffix("/go.mod").unwrap_or(version);
        packages
            .entry(module.to_owned())
            .or_insert_with(|| version.to_owned());
    }
    packages
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

fn locate_require_version(text: &str, module: &str) -> Option<VersionSpan> {
    let mut block: Option<String> = None;
    let mut found = None;
    let mut base = 0;
    for line in text.split_inclusive('\n') {
        let tokens = tokenize(line, base);
        base += line.len();
        let Some(first) = tokens.first() else {
            continue;
        };
        let entry = if let Some(keyword) = &block {
            if first.value == ")" {
                block = None;
                continue;
            }
            (keyword == "require").then_some(&tokens[..])
        } else {
            if tokens.get(1).map(|token| token.value.as_str()) == Some("(") {
                block = Some(first.value.clone());
                continue;
            }
            (first.value == "require").then(|| &tokens[1..])
        };
        let Some([name, version, ..]) = entry else {
            continue;
        };
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
