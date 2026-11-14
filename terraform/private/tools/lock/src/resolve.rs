//! Resolves the `required_providers` of the `*.tf` files of a set to registry provider archives, one per platform.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The only registry host the resolver reads.
pub const HOST: &str = "registry.terraform.io";

/// The registry operating systems and architectures, and the lock platform of each.
const PLATFORMS: [(&str, &str, &str); 6] = [
    ("linux", "amd64", "linux_x86_64"),
    ("linux", "arm64", "linux_arm64"),
    ("darwin", "amd64", "macos_x86_64"),
    ("darwin", "arm64", "macos_arm64"),
    ("windows", "amd64", "windows_x86_64"),
    ("windows", "arm64", "windows_arm64"),
];

/// Reads JSON documents from a registry.
pub trait Registry {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn get(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Reads documents over HTTPS.
pub struct Site(HttpFetch);

impl Site {
    /// A registry client that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Site {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry for Site {
    fn get(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// The source text with comments blanked, strings kept.
fn uncommented(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' => skip_line(&mut chars, &mut out),
            '/' if chars.peek() == Some(&'/') => skip_line(&mut chars, &mut out),
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut star = false;
                for next in chars.by_ref() {
                    if star && next == '/' {
                        break;
                    }
                    star = next == '*';
                }
                out.push(' ');
            }
            '"' => {
                out.push(c);
                while let Some(next) = chars.next() {
                    out.push(next);
                    match next {
                        '\\' => out.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn skip_line(chars: &mut impl Iterator<Item = char>, out: &mut String) {
    for next in chars.by_ref() {
        if next == '\n' {
            out.push('\n');
            break;
        }
    }
}

/// One token of the block grammar the resolver reads.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    Text(String),
    Open,
    Close,
    Equals,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '=' => out.push(Token::Equals),
            '"' => {
                let mut value = String::new();
                while let Some(next) = chars.next() {
                    match next {
                        '\\' => value.extend(chars.next()),
                        '"' => break,
                        other => value.push(other),
                    }
                }
                out.push(Token::Text(value));
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {
                let mut word = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' || next == '-' {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Token::Word(word));
            }
            _ => {}
        }
    }
    out
}

/// One `required_providers` entry: the provider source address and the version constraint.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Requirement {
    /// `HOST/NAMESPACE/TYPE`.
    pub source: String,
    /// The constraint, empty for any version.
    pub constraint: String,
}

fn source_address(name: &str, source: Option<&str>) -> Result<String, Error> {
    let source = source.map_or_else(|| format!("hashicorp/{name}"), str::to_owned);
    let parts: Vec<&str> = source.split('/').collect();
    let full = match parts.as_slice() {
        [namespace, kind] => format!("{HOST}/{namespace}/{kind}"),
        [host, _, _] if *host == HOST => source.clone(),
        _ => {
            return Err(Error::new(format!(
                "provider {name}: source {source:?} is not NAMESPACE/TYPE or {HOST}/NAMESPACE/TYPE"
            )));
        }
    };
    let valid = full.split('/').skip(1).all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    });
    if valid {
        Ok(full)
    } else {
        Err(Error::new(format!("provider {name}: source {source:?} has an invalid segment")))
    }
}

/// The provider requirements of Terraform source, in source order.
///
/// # Errors
/// Fails on a provider source that is not on the public registry.
pub fn requirements(text: &str) -> Result<Vec<Requirement>, Error> {
    let tokens = tokens(&uncommented(text));
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(index) = tokens
        .iter()
        .skip(at)
        .position(|token| *token == Token::Word("required_providers".to_owned()))
    {
        let start = at + index + 1;
        at = start;
        if tokens.get(start) != Some(&Token::Open) {
            continue;
        }
        let mut depth = 1;
        let mut cursor = start + 1;
        let mut block = Vec::new();
        while let Some(token) = tokens.get(cursor) {
            cursor += 1;
            match token {
                Token::Open => depth += 1,
                Token::Close => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            block.push(token.clone());
        }
        found.extend(entries(&block)?);
        at = cursor;
    }
    Ok(found)
}

fn entries(block: &[Token]) -> Result<Vec<Requirement>, Error> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(Token::Word(name)) = block.get(cursor) {
        if block.get(cursor + 1) != Some(&Token::Equals) {
            cursor += 1;
            continue;
        }
        match block.get(cursor + 2) {
            Some(Token::Text(constraint)) => {
                out.push(Requirement {
                    source: source_address(name, None)?,
                    constraint: constraint.clone(),
                });
                cursor += 3;
            }
            Some(Token::Open) => {
                let mut source = None;
                let mut constraint = String::new();
                let mut depth = 1;
                cursor += 3;
                while let Some(token) = block.get(cursor) {
                    match token {
                        Token::Open => depth += 1,
                        Token::Close => depth -= 1,
                        Token::Word(key) if depth == 1 => {
                            if let (Some(Token::Equals), Some(Token::Text(value))) =
                                (block.get(cursor + 1), block.get(cursor + 2))
                            {
                                match key.as_str() {
                                    "source" => source = Some(value.clone()),
                                    "version" => constraint.clone_from(value),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    cursor += 1;
                    if depth == 0 {
                        break;
                    }
                }
                out.push(Requirement {
                    source: source_address(name, source.as_deref())?,
                    constraint,
                });
            }
            _ => cursor += 1,
        }
    }
    Ok(out)
}

/// A release version: its numeric parts.
fn version_parts(text: &str) -> Option<Vec<u64>> {
    text.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().ok())
        .collect()
}

fn compare(left: &[u64], right: &[u64]) -> std::cmp::Ordering {
    let width = left.len().max(right.len());
    let pad = |parts: &[u64]| {
        let mut padded = parts.to_vec();
        padded.resize(width, 0);
        padded
    };
    pad(left).cmp(&pad(right))
}

/// Whether the release `version` satisfies every comma separated clause of `constraint`.
///
/// # Errors
/// Fails on a clause the resolver does not read.
pub fn satisfies(version: &str, constraint: &str) -> Result<bool, Error> {
    let Some(have) = version_parts(version) else {
        return Ok(false);
    };
    for clause in constraint.split(',').map(str::trim).filter(|clause| !clause.is_empty()) {
        let (operator, number) = ["~>", ">=", "<=", "!=", "=", ">", "<"]
            .iter()
            .find_map(|operator| clause.strip_prefix(operator).map(|rest| (*operator, rest)))
            .unwrap_or(("=", clause));
        let Some(want) = version_parts(number) else {
            return Err(Error::new(format!("version constraint {clause:?} is not understood")));
        };
        let order = compare(&have, &want);
        let holds = match operator {
            "=" => order.is_eq(),
            "!=" => order.is_ne(),
            ">" => order.is_gt(),
            ">=" => order.is_ge(),
            "<" => order.is_lt(),
            "<=" => order.is_le(),
            _ => {
                let keep = want.len().saturating_sub(1).max(1);
                let mut ceiling: Vec<u64> = want.iter().take(keep).copied().collect();
                if let Some(last) = ceiling.last_mut() {
                    *last += 1;
                }
                order.is_ge() && compare(&have, &ceiling).is_lt()
            }
        };
        if !holds {
            return Ok(false);
        }
    }
    Ok(true)
}

fn json(bytes: &[u8], what: &str) -> Result<serde_json::Value, Error> {
    serde_json::from_slice(bytes).map_err(|error| Error::new(format!("{what}: not JSON: {error}")))
}

fn text<'a>(value: &'a serde_json::Value, key: &str, what: &str) -> Result<&'a str, Error> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new(format!("{what}: no {key}")))
}

/// The resolver of the terraform ecosystem.
pub struct Terraform<'a> {
    registry: &'a dyn Registry,
    state: String,
}

impl<'a> Terraform<'a> {
    /// A resolver that reads documents from `registry` and keeps the state the registry entry names.
    #[must_use]
    pub fn new(registry: &'a dyn Registry, hub: &dx_families::Hub) -> Self {
        Self {
            registry,
            state: hub.state.clone().unwrap_or_default(),
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

    fn api(source: &str) -> String {
        let path = source.strip_prefix(&format!("{HOST}/")).unwrap_or(source);
        format!("https://{HOST}/v1/providers/{path}")
    }

    fn version(&self, source: &str, constraint: &str) -> Result<(String, Vec<(String, String)>), Error> {
        let url = format!("{}/versions", Self::api(source));
        let listing = json(&self.registry.get(&url)?, &url)?;
        let mut best: Option<(Vec<u64>, String, Vec<(String, String)>)> = None;
        for entry in listing
            .get("versions")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            let version = text(entry, "version", &url)?;
            let Some(parts) = version_parts(version) else {
                continue;
            };
            if !satisfies(version, constraint)? {
                continue;
            }
            let platforms: Vec<(String, String)> = entry
                .get("platforms")
                .and_then(serde_json::Value::as_array)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter_map(|platform| {
                    let os = platform.get("os")?.as_str()?;
                    let arch = platform.get("arch")?.as_str()?;
                    PLATFORMS
                        .iter()
                        .any(|(o, a, _)| *o == os && *a == arch)
                        .then(|| (os.to_owned(), arch.to_owned()))
                })
                .collect();
            if !platforms.is_empty() && best.as_ref().is_none_or(|(have, _, _)| compare(&parts, have).is_gt()) {
                best = Some((parts, version.to_owned(), platforms));
            }
        }
        best.map(|(_, version, platforms)| (version, platforms)).ok_or_else(|| {
            Error::new(format!("{source}: no release satisfies {constraint:?} for a supported platform"))
        })
    }

    fn package(&self, source: &str, constraint: &str) -> Result<Package, Error> {
        let (version, platforms) = self
            .version(source, constraint)
            .map_err(|error| Error::new(format!("{source}: {error}")))?;
        let mut archives = Vec::new();
        for (os, arch) in platforms {
            let url = format!("{}/{version}/download/{os}/{arch}", Self::api(source));
            let download = json(&self.registry.get(&url)?, &url)?;
            let link = text(&download, "download_url", &url)?;
            if !link.starts_with("https://") {
                return Err(Error::new(format!("{url}: download_url {link:?} is not https")));
            }
            let platform = PLATFORMS
                .iter()
                .find_map(|(o, a, platform)| (*o == os && *a == arch).then_some(*platform))
                .unwrap_or_default();
            archives.push(Archive {
                urls: vec![link.to_owned()],
                sha256: text(&download, "shasum", &url)?.to_owned(),
                platform: platform.to_owned(),
                ..Archive::default()
            });
        }
        archives.sort_by(|a, b| a.platform.cmp(&b.platform));
        Ok(Package {
            archives,
            provides: vec![source.to_owned()],
            name: source.to_owned(),
            version,
            ..Package::default()
        })
    }
}

fn read_requirements(scratch: &Scratch) -> Result<BTreeMap<String, BTreeSet<String>>, Error> {
    let entries = std::fs::read_dir(scratch.set())
        .map_err(|error| Error::new(format!("{}: {error}", scratch.set().display())))?;
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "tf"))
        .collect();
    files.sort();
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files {
        let shown = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(&file)
            .map_err(|error| Error::new(format!("{shown}: {error}")))?;
        for requirement in requirements(&source).map_err(|error| Error::new(format!("{shown}: {error}")))? {
            let constraints = found.entry(requirement.source).or_default();
            if !requirement.constraint.is_empty() {
                constraints.insert(requirement.constraint);
            }
        }
    }
    Ok(found)
}

/// The text of the state file for the resolved providers.
#[must_use]
pub fn state(packages: &[Package], constraints: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut out = String::from("# This file is maintained automatically by the terraform lock step.\n");
    for package in packages {
        out.push_str(&format!(
            "\nprovider \"{}\" {{\n  version = \"{}\"\n",
            package.name, package.version
        ));
        if let Some(set) = constraints.get(&package.name).filter(|set| !set.is_empty()) {
            let joined: Vec<&str> = set.iter().map(String::as_str).collect();
            out.push_str(&format!("  constraints = \"{}\"\n", joined.join(", ")));
        }
        let hashes: BTreeSet<String> = package
            .archives
            .iter()
            .map(|archive| format!("zh:{}", archive.sha256))
            .collect();
        out.push_str("  hashes = [\n");
        for hash in hashes {
            out.push_str(&format!("    \"{hash}\",\n"));
        }
        out.push_str("  ]\n}\n");
    }
    out
}

fn pinned(scratch: &Scratch, state: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(scratch.set().join(state)) else {
        return found;
    };
    let tokens = tokens(&uncommented(&text));
    let mut cursor = 0;
    while let Some(token) = tokens.get(cursor) {
        cursor += 1;
        if *token != Token::Word("provider".to_owned()) {
            continue;
        }
        if let (Some(Token::Text(source)), Some(Token::Open)) = (tokens.get(cursor), tokens.get(cursor + 1)) {
            let mut probe = cursor + 2;
            while let Some(next) = tokens.get(probe) {
                if *next == Token::Close {
                    break;
                }
                if let (Token::Word(key), Some(Token::Equals), Some(Token::Text(value))) =
                    (next, tokens.get(probe + 1), tokens.get(probe + 2))
                    && key == "version"
                {
                    found.insert(source.clone(), value.clone());
                }
                probe += 1;
            }
        }
    }
    found
}

impl Resolver for Terraform<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let wanted = read_requirements(scratch)?;
        let state_versions = pinned(scratch, &self.state);
        let mut packages = Vec::new();
        for (source, constraints) in &wanted {
            let constraint = constraints.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
            let kept = old.and_then(|lock| {
                lock.packages.iter().find(|package| {
                    package.name == *source && satisfies(&package.version, &constraint).unwrap_or(false)
                })
            });
            let pin = state_versions
                .get(source)
                .filter(|version| satisfies(version, &constraint).unwrap_or(false));
            packages.push(match (kept, pin) {
                (Some(package), _) => package.clone(),
                (None, Some(version)) => self.package(source, &format!("={version}"))?,
                (None, None) => self.package(source, &constraint)?,
            });
        }
        std::fs::write(
            scratch.set().join(&self.state),
            state(&packages, &wanted),
        )
        .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        Ok(Resolution { packages })
    }
}
