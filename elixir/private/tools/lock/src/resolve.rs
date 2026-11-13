//! Resolves a Hex package set from the Mix or rebar3 lock next to its manifest.

use dx_lock::{Archive, Error, Lock, Package, Resolution, Resolver, Scratch, Spec};
use std::collections::BTreeMap;

/// The tarball URL prefix of repo.hex.pm.
pub const HEX_REPO: &str = "https://repo.hex.pm/tarballs/";

/// The `rebar.config` state file.
pub const REBAR_LOCK: &str = "rebar.lock";

/// One Erlang or Elixir term.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Term {
    /// A string or binary.
    Text(String),
    /// An atom, number or other bare word.
    Atom(String),
    /// A tuple.
    Tuple(Vec<Term>),
    /// A list; a keyword entry is a two element tuple.
    List(Vec<Term>),
    /// A map, in source order.
    Map(Vec<(Term, Term)>),
}

impl Term {
    /// The text of a string, binary or atom.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) | Self::Atom(text) => Some(text),
            _ => None,
        }
    }

    fn items(&self) -> Option<&[Term]> {
        match self {
            Self::Tuple(items) | Self::List(items) => Some(items),
            _ => None,
        }
    }
}

struct Parser<'a> {
    text: &'a [u8],
    pos: usize,
    comment: u8,
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'@' | b'-' | b'+')
}

impl Parser<'_> {
    fn fail<T>(&self, message: &str) -> Result<T, Error> {
        Err(Error::new(format!("{message} at byte {}", self.pos)))
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn rest(&self) -> &[u8] {
        &self.text[self.pos.min(self.text.len())..]
    }

    fn skip(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == self.comment {
                while self.peek().is_some_and(|byte| byte != b'\n') {
                    self.pos += 1;
                }
            } else if byte.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip();
        if self.rest().starts_with(token.as_bytes()) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    fn quoted(&mut self, quote: u8) -> Result<String, Error> {
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return self.fail("unterminated string"),
                Some(byte) if byte == quote => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    let Some(&next) = self.text.get(self.pos + 1) else {
                        return self.fail("unterminated string");
                    };
                    out.push(next);
                    self.pos += 2;
                }
                Some(byte) => {
                    out.push(byte);
                    self.pos += 1;
                }
            }
        }
        String::from_utf8(out).map_err(|error| Error::new(error.to_string()))
    }

    fn word(&mut self) -> String {
        let start = self.pos;
        while self.peek().is_some_and(is_word) {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.text[start..self.pos]).into_owned()
    }

    fn keyword(&mut self) -> Option<String> {
        self.skip();
        let start = self.pos;
        let name = match self.peek() {
            Some(b'"') => self.quoted(b'"').ok(),
            Some(byte) if is_word(byte) => Some(self.word()),
            _ => None,
        };
        if let Some(name) = name
            && self.rest().starts_with(b":")
            && !self.rest().starts_with(b"::")
            && self
                .text
                .get(self.pos + 1)
                .is_none_or(u8::is_ascii_whitespace)
        {
            self.pos += 1;
            return Some(name);
        }
        self.pos = start;
        None
    }

    fn sequence(&mut self, close: &str, keywords: bool) -> Result<Vec<Term>, Error> {
        let mut items = Vec::new();
        if self.eat(close) {
            return Ok(items);
        }
        loop {
            let item = match keywords.then(|| self.keyword()).flatten() {
                Some(key) => Term::Tuple(vec![Term::Atom(key), self.term()?]),
                None => self.term()?,
            };
            items.push(item);
            if self.eat(close) {
                return Ok(items);
            }
            if !self.eat(",") {
                return self.fail(&format!("expected , or {close}"));
            }
            if self.eat(close) {
                return Ok(items);
            }
        }
    }

    fn map(&mut self) -> Result<Vec<(Term, Term)>, Error> {
        let mut entries = Vec::new();
        loop {
            if self.eat("}") {
                return Ok(entries);
            }
            let entry = match self.keyword() {
                Some(key) => (Term::Atom(key), self.term()?),
                None => {
                    let key = self.term()?;
                    if !self.eat("=>") {
                        return self.fail("expected =>");
                    }
                    (key, self.term()?)
                }
            };
            entries.push(entry);
            if !self.eat(",") && !self.rest().starts_with(b"}") {
                self.skip();
                if !self.rest().starts_with(b"}") {
                    return self.fail("expected , or }");
                }
            }
        }
    }

    fn term(&mut self) -> Result<Term, Error> {
        self.skip();
        if self.eat("%{") {
            return Ok(Term::Map(self.map()?));
        }
        if self.eat("<<") {
            self.skip();
            let text = if self.peek() == Some(b'"') {
                self.quoted(b'"')?
            } else {
                String::new()
            };
            if !self.eat(">>") {
                return self.fail("expected >>");
            }
            return Ok(Term::Text(text));
        }
        match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                Ok(Term::Tuple(self.sequence("}", false)?))
            }
            Some(b'[') => {
                self.pos += 1;
                Ok(Term::List(self.sequence("]", true)?))
            }
            Some(b'"') => Ok(Term::Text(self.quoted(b'"')?)),
            Some(b'\'') => Ok(Term::Atom(self.quoted(b'\'')?)),
            Some(b':') => {
                self.pos += 1;
                if self.peek() == Some(b'"') {
                    Ok(Term::Atom(self.quoted(b'"')?))
                } else {
                    Ok(Term::Atom(self.word()))
                }
            }
            Some(byte) if is_word(byte) => Ok(Term::Atom(self.word())),
            Some(_) => self.fail("unexpected character"),
            None => self.fail("unexpected end"),
        }
    }
}

/// The one term of an Elixir file such as a Mix lock.
///
/// # Errors
/// Fails when the text is not one term.
pub fn parse_elixir(text: &str) -> Result<Term, Error> {
    let mut parser = Parser {
        text: text.as_bytes(),
        pos: 0,
        comment: b'#',
    };
    let term = parser.term()?;
    parser.skip();
    if parser.peek().is_some() {
        return parser.fail("trailing text");
    }
    Ok(term)
}

/// The dot terminated terms of an Erlang file such as `rebar.lock`.
///
/// # Errors
/// Fails when the text is not a list of terms each ended by `.`.
pub fn parse_erlang(text: &str) -> Result<Vec<Term>, Error> {
    let mut parser = Parser {
        text: text.as_bytes(),
        pos: 0,
        comment: b'%',
    };
    let mut terms = Vec::new();
    loop {
        parser.skip();
        if parser.peek().is_none() {
            return Ok(terms);
        }
        terms.push(parser.term()?);
        if !parser.eat(".") {
            return parser.fail("expected .");
        }
    }
}

fn package(name: &str, version: &str, sha256: &str, deps: Vec<String>) -> Result<Package, Error> {
    let sha256 = sha256.to_ascii_lowercase();
    if !dx_lock::is_sha256(&sha256) {
        return Err(Error::new(format!(
            "{name} {version} has no sha256 checksum"
        )));
    }
    Ok(Package {
        name: name.to_owned(),
        version: version.to_owned(),
        archives: vec![Archive {
            urls: vec![format!("{HEX_REPO}{name}-{version}.tar")],
            sha256,
            strip_prefix: String::new(),
            platform: String::new(),
            kind: String::new(),
        }],
        deps,
        provides: vec![name.to_owned()],
        dev: false,
        extra: BTreeMap::new(),
    })
}

fn option<'t>(options: &'t [Term], key: &str) -> Option<&'t Term> {
    options.iter().find_map(|option| match option.items() {
        Some([name, value]) if name.text() == Some(key) => Some(value),
        _ => None,
    })
}

fn mix_dependency(dep: &Term) -> Option<String> {
    let items = dep.items()?;
    let options = items.get(2).and_then(Term::items).unwrap_or_default();
    if option(options, "optional").and_then(Term::text) == Some("true") {
        return None;
    }
    option(options, "hex")
        .or_else(|| items.first())
        .and_then(Term::text)
        .map(str::to_owned)
}

/// The packages of a Mix lock.
///
/// # Errors
/// Fails when the text is not a map of hexpm entries.
pub fn mix_packages(text: &str) -> Result<Vec<Package>, Error> {
    let Term::Map(entries) = parse_elixir(text)? else {
        return Err(Error::new("the lock must hold one map"));
    };
    let mut packages = Vec::new();
    for (key, entry) in &entries {
        let key = key.text().unwrap_or_default();
        let items = entry.items().unwrap_or_default();
        let kind = items.first().and_then(Term::text).unwrap_or_default();
        if kind != "hex" {
            return Err(Error::new(format!(
                "entry {key} is a {kind} dependency; only hex entries are supported"
            )));
        }
        let field = |index: usize| items.get(index).and_then(Term::text);
        if items.len() < 8 || field(6) != Some("hexpm") {
            return Err(Error::new(format!(
                "entry {key} is not from the hexpm repository"
            )));
        }
        let deps = items[5]
            .items()
            .unwrap_or_default()
            .iter()
            .filter_map(mix_dependency)
            .collect();
        packages.push(package(
            field(1).unwrap_or_default(),
            field(2).unwrap_or_default(),
            field(7).unwrap_or_default(),
            deps,
        )?);
    }
    Ok(packages)
}

/// The packages of a `rebar.lock` in lock format 1.2.0.
///
/// # Errors
/// Fails when the text is not a 1.2.0 lock of pkg entries with `pkg_hash_ext` checksums.
pub fn rebar_packages(text: &str) -> Result<Vec<Package>, Error> {
    let terms = parse_erlang(text)?;
    let shape = || {
        Error::new(
            "rebar.lock must be lock format 1.2.0: a versioned entry list followed by the hash tables",
        )
    };
    let [head, tables] = terms.as_slice() else {
        return Err(shape());
    };
    let Some([version, entries]) = head.items() else {
        return Err(shape());
    };
    if version.text() != Some("1.2.0") {
        return Err(shape());
    }
    let hashes: BTreeMap<&str, &str> = tables
        .items()
        .unwrap_or_default()
        .iter()
        .find_map(|table| match table.items() {
            Some([name, list]) if name.text() == Some("pkg_hash_ext") => list.items(),
            _ => None,
        })
        .ok_or_else(|| Error::new("rebar.lock has no pkg_hash_ext table"))?
        .iter()
        .filter_map(|pair| match pair.items() {
            Some([name, hash]) => Some((name.text()?, hash.text()?)),
            _ => None,
        })
        .collect();
    let mut packages = Vec::new();
    for entry in entries.items().unwrap_or_default() {
        let items = entry.items().unwrap_or_default();
        let key = items.first().and_then(Term::text).unwrap_or_default();
        let source = items.get(1).and_then(Term::items).unwrap_or_default();
        let kind = source.first().and_then(Term::text).unwrap_or_default();
        if kind != "pkg" {
            return Err(Error::new(format!(
                "rebar.lock entry {key} is a {kind} dependency; only pkg entries are supported"
            )));
        }
        let hash = hashes
            .get(key)
            .ok_or_else(|| Error::new(format!("rebar.lock has no pkg_hash_ext for {key}")))?;
        let field = |index: usize| source.get(index).and_then(Term::text).unwrap_or_default();
        packages.push(package(field(1), field(2), hash, Vec::new())?);
    }
    Ok(packages)
}

/// The resolver of the hex ecosystem.
pub struct Hex {
    mix: String,
}

impl Hex {
    /// The resolver that reads the Mix lock named first in `spec.state`.
    #[must_use]
    pub fn new(spec: &Spec) -> Self {
        Self {
            mix: spec.state.first().cloned().unwrap_or_default(),
        }
    }

    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone(), "rebar.config".to_owned()],
            state: hub
                .lock
                .iter()
                .cloned()
                .chain([REBAR_LOCK.to_owned()])
                .collect(),
        }
    }
}

impl Resolver for Hex {
    fn resolve(&self, scratch: &Scratch, _old: Option<&Lock>) -> Result<Resolution, Error> {
        let read = |name: &str| std::fs::read_to_string(scratch.set().join(name)).ok();
        let mix = &self.mix;
        let packages = if let Some(text) = read(mix) {
            mix_packages(&text).map_err(|error| Error::new(format!("{mix}: {error}")))?
        } else if let Some(text) = read(REBAR_LOCK) {
            rebar_packages(&text).map_err(|error| Error::new(format!("{REBAR_LOCK}: {error}")))?
        } else {
            return Err(Error::new(format!(
                "the set has no {mix} or {REBAR_LOCK}; run mix deps.get or rebar3 lock first"
            )));
        };
        Ok(Resolution { packages })
    }
}
