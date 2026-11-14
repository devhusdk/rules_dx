//! Locks the packages that `*.opam.locked` files pin, with the archive each opam-repository entry names.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The opam-repository tree the package files are read from.
pub const REPOSITORY: &str = concat!(
    "https://raw.githubusercontent.com/ocaml/opam-repository/",
    env!("OPAM_REPOSITORY_COMMIT"),
    "/packages"
);

/// The `depends` filters of entries a build does not need.
pub const SKIPPED_FILTERS: [&str; 4] = ["dev", "with-dev-setup", "with-doc", "with-test"];

const TOOLCHAIN_PACKAGES: [&str; 7] = [
    "dune",
    "ocaml",
    "ocaml-base-compiler",
    "ocaml-config",
    "ocaml-options-vanilla",
    "ocaml-system",
    "ocaml-variants",
];

/// The `extra` key listing the dependencies of a package that no lock file pins.
pub const UNPINNED: &str = "opam_unpinned";

/// The suffix of the state files.
pub const LOCKED: &str = ".opam.locked";

/// Downloads opam files.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Https(HttpFetch);

impl Https {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Https {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Https {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// Whether the pinned toolchain provides the opam package.
#[must_use]
pub fn is_toolchain_package(name: &str) -> bool {
    TOOLCHAIN_PACKAGES.contains(&name) || name.starts_with("base-")
}

/// The library name dune gives a package by default.
#[must_use]
pub fn library(name: &str) -> String {
    name.replace(['-', '.'], "_")
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Str(String),
    Word(String),
    Open(char),
    Close(char),
}

fn tokens(text: &str) -> Result<Vec<Token>, Error> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c.is_whitespace() {
            i += 1;
        } else if c == '#' {
            while chars.get(i).is_some_and(|&next| next != '\n') {
                i += 1;
            }
        } else if c == '(' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while chars.get(i..i + 2).is_some_and(|pair| pair != ['*', ')']) {
                i += 1;
            }
            if chars.get(i..i + 2).is_none() {
                return Err(Error::new("has an unterminated comment"));
            }
            i += 2;
        } else if c == '"' {
            let triple = chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'"');
            let mut value = String::new();
            i += if triple { 3 } else { 1 };
            loop {
                let Some(&next) = chars.get(i) else {
                    return Err(Error::new("has an unterminated string"));
                };
                if next == '\\' {
                    if let Some(&escaped) = chars.get(i + 1) {
                        value.push(escaped);
                    }
                    i += 2;
                } else if triple
                    && chars
                        .get(i..)
                        .is_some_and(|rest| rest.starts_with(&['"', '"', '"']))
                {
                    i += 3;
                    break;
                } else if !triple && next == '"' {
                    i += 1;
                    break;
                } else {
                    value.push(next);
                    i += 1;
                }
            }
            out.push(Token::Str(value));
        } else if matches!(c, '[' | '{' | '(') {
            out.push(Token::Open(c));
            i += 1;
        } else if matches!(c, ']' | '}' | ')') {
            out.push(Token::Close(c));
            i += 1;
        } else {
            let start = i;
            while chars.get(i).is_some_and(|&next| {
                !next.is_whitespace()
                    && !matches!(next, '"' | '[' | ']' | '{' | '}' | '(' | ')' | '#')
            }) {
                i += 1;
            }
            let word = chars
                .get(start..i)
                .ok_or_else(|| Error::new("has a word out of range"))?;
            out.push(Token::Word(word.iter().collect()));
        }
    }
    Ok(out)
}

/// The tokens of the top-level field `key`, without the key.
fn field(all: &[Token], key: &str) -> Option<Vec<Token>> {
    let mut depth = 0usize;
    let wanted = format!("{key}:");
    for (i, token) in all.iter().enumerate() {
        match token {
            Token::Open(_) => depth += 1,
            Token::Close(_) => depth = depth.saturating_sub(1),
            Token::Word(word) if depth == 0 && (*word == wanted || *word == key) => {
                return all.get(i + 1..).map(value);
            }
            Token::Str(_) | Token::Word(_) => {}
        }
    }
    None
}

/// The tokens of one value: a single token, or one bracketed group with its brackets.
fn value(rest: &[Token]) -> Vec<Token> {
    let mut depth = 0usize;
    for (i, token) in rest.iter().enumerate() {
        match token {
            Token::Open(_) => depth += 1,
            Token::Close(_) => depth = depth.saturating_sub(1),
            Token::Str(_) | Token::Word(_) => {}
        }
        if depth == 0 {
            return rest.iter().take(i + 1).cloned().collect();
        }
    }
    rest.to_vec()
}

fn render(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|token| match token {
            Token::Str(text) => format!("\"{text}\""),
            Token::Word(text) => text.clone(),
            Token::Open(c) | Token::Close(c) => c.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `(name, filter)` entries of the `depends` field, the filter as text.
fn depends(text: &str) -> Result<Vec<(String, String)>, Error> {
    let all = tokens(text)?;
    let Some(list) = field(&all, "depends") else {
        return Ok(Vec::new());
    };
    let inner = match list.as_slice() {
        [Token::Open('['), inner @ .., _] => inner,
        [Token::Open('[')] => return Err(Error::new("has an unterminated depends list")),
        all => all,
    };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while let Some(rest @ [token, ..]) = inner.get(i..) {
        match token {
            Token::Str(name) => {
                out.push((name.clone(), String::new()));
                i += 1;
            }
            Token::Open('{') => {
                let group = value(rest);
                let [_, filter @ .., _] = group.as_slice() else {
                    return Err(Error::new("has an unterminated dependency filter"));
                };
                if let Some(last) = out.last_mut() {
                    last.1 = render(filter);
                }
                i += group.len();
            }
            Token::Word(_) | Token::Open(_) | Token::Close(_) => i += 1,
        }
    }
    Ok(out)
}

fn skipped(filter: &str) -> bool {
    filter
        .split('&')
        .any(|term| SKIPPED_FILTERS.contains(&term.trim()))
}

/// The `{package: version}` pins of an opam lock file; entries limited to tests, docs or development are left out.
///
/// # Errors
/// Fails when the text is not opam syntax.
pub fn parse_locked(text: &str) -> Result<BTreeMap<String, String>, Error> {
    let mut out = BTreeMap::new();
    for (name, filter) in depends(text)? {
        if skipped(&filter) || filter.contains('|') {
            continue;
        }
        let version = filter.split('&').find_map(|term| {
            let rest = term.trim().strip_prefix('=')?;
            if rest.starts_with('=') {
                return None;
            }
            Some(rest.trim().trim_matches('"').to_owned())
        });
        if let Some(version) = version {
            out.insert(name, version);
        }
    }
    Ok(out)
}

/// The `depends` names a build needs.
///
/// # Errors
/// Fails when the text is not opam syntax.
pub fn parse_depend_names(text: &str) -> Result<Vec<String>, Error> {
    Ok(depends(text)?
        .into_iter()
        .filter(|(_, filter)| !skipped(filter))
        .map(|(name, _)| name)
        .collect())
}

/// The source of a package archive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Source {
    /// The archive URL.
    pub src: String,
    /// The sha256 the opam file states, else empty.
    pub sha256: String,
}

/// The `url` section of an opam file.
///
/// # Errors
/// Fails when the file has no url section with an https `src`.
pub fn parse_url(text: &str) -> Result<Source, Error> {
    let all = tokens(text)?;
    let start = all
        .iter()
        .position(|token| *token == Token::Word("url".to_owned()))
        .ok_or_else(|| Error::new("has no url section"))?;
    let section = all.get(start + 1..).map(value).unwrap_or_default();
    let body = section
        .get(1..)
        .ok_or_else(|| Error::new("has an empty url section"))?;
    let src = field(body, "src")
        .and_then(|tokens| match tokens.first() {
            Some(Token::Str(src)) => Some(src.clone()),
            _ => None,
        })
        .ok_or_else(|| Error::new("has a url section without src"))?;
    if !src.starts_with("https://") {
        return Err(Error::new(format!(
            "has the source {src}, which is not https"
        )));
    }
    let sha256 = field(body, "checksum")
        .unwrap_or_default()
        .iter()
        .find_map(|token| match token {
            Token::Str(sum) => sum.strip_prefix("sha256=").map(str::to_owned),
            Token::Word(_) | Token::Open(_) | Token::Close(_) => None,
        })
        .unwrap_or_default();
    Ok(Source { src, sha256 })
}

/// The resolver of the opam ecosystem.
pub struct Opam<'a> {
    repository: &'a str,
    download: &'a dyn Download,
}

impl<'a> Opam<'a> {
    /// A resolver that reads opam files below `repository`.
    #[must_use]
    pub fn new(repository: &'a str, download: &'a dyn Download) -> Self {
        Self {
            repository,
            download,
        }
    }

    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }

    fn opam_file(&self, name: &str, version: &str) -> Result<String, Error> {
        let url = format!("{}/{name}/{name}.{version}/opam", self.repository);
        let bytes = self
            .download
            .bytes(&url)
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        String::from_utf8(bytes).map_err(|_| Error::new(format!("{url} is not UTF-8")))
    }

    /// The lock packages of the pins.
    ///
    /// # Errors
    /// Fails when an opam file cannot be read or has no usable url section.
    pub fn packages(&self, pins: &BTreeMap<String, String>) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();
        for (name, version) in pins {
            if is_toolchain_package(name) {
                continue;
            }
            let text = self.opam_file(name, version)?;
            let source = parse_url(&text)
                .map_err(|error| Error::new(format!("{name} {version} {error}")))?;
            let mut deps = BTreeSet::new();
            let mut unpinned = BTreeSet::new();
            for dep in parse_depend_names(&text)? {
                if is_toolchain_package(&dep) || dep == *name {
                    continue;
                }
                if pins.contains_key(&dep) {
                    deps.insert(dep);
                } else {
                    unpinned.insert(dep);
                }
            }
            let mut extra = BTreeMap::new();
            if !unpinned.is_empty() {
                extra.insert(UNPINNED.to_owned(), serde_json::json!(unpinned));
            }
            out.push(Package {
                name: name.clone(),
                version: version.clone(),
                archives: vec![Archive {
                    urls: vec![source.src],
                    sha256: source.sha256,
                    strip_prefix: format!("{name}-{version}"),
                    ..Archive::default()
                }],
                deps: deps.into_iter().collect(),
                provides: vec![library(name)],
                extra,
                ..Package::default()
            });
        }
        Ok(out)
    }
}

impl Resolver for Opam<'_> {
    fn resolve(&self, scratch: &Scratch, _old: Option<&Lock>) -> Result<Resolution, Error> {
        let mut names: Vec<String> = std::fs::read_dir(scratch.set())
            .map_err(|error| Error::new(error.to_string()))?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(LOCKED))
            .collect();
        names.sort();
        if names.is_empty() {
            return Err(Error::new(format!(
                "the set has no {LOCKED} file; run opam lock"
            )));
        }
        let mut pins = BTreeMap::new();
        for name in &names {
            let text = std::fs::read_to_string(scratch.set().join(name))
                .map_err(|error| Error::new(format!("{name}: {error}")))?;
            for (package, version) in
                parse_locked(&text).map_err(|error| Error::new(format!("{name} {error}")))?
            {
                if let Some(other) = pins.insert(package.clone(), version.clone())
                    && other != version
                {
                    return Err(Error::new(format!(
                        "{package} is pinned at {other} and {version}"
                    )));
                }
            }
        }
        Ok(Resolution {
            groups: std::collections::BTreeMap::new(),
            packages: self.packages(&pins)?,
        })
    }
}
