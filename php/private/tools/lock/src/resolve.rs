//! Resolves a Composer package set through the Packagist v2 API and writes its lock and state.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

/// The metadata endpoint of Packagist.
pub const METADATA: &str = "https://repo.packagist.org/p2";

/// Downloads metadata and archives.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Packagist(HttpFetch);

impl Packagist {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Packagist {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Packagist {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

type Version = [u64; 4];

/// Whether a requirement names the platform rather than a package.
#[must_use]
pub fn is_platform(name: &str) -> bool {
    matches!(
        name,
        "php" | "php-64bit" | "php-ipv6" | "php-zts" | "php-debug" | "hhvm"
    ) || ["ext-", "lib-", "composer-"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The parts of a stable release version, `None` for branches and pre-releases.
#[must_use]
pub fn parse_version(text: &str) -> Option<Version> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut out = [0; 4];
    let mut count = 0;
    for part in text.split('.') {
        let slot = out.get_mut(count)?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
        count += 1;
    }
    (count > 0).then_some(out)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
    Ne,
}

type Comparator = (Op, Version);

fn partial(text: &str) -> Result<(Version, usize), String> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut out = [0; 4];
    let mut count = 0;
    for part in text.split('.') {
        let Some(slot) = out.get_mut(count) else {
            return Err(format!("{text:?} has more than four parts"));
        };
        if matches!(part, "*" | "x" | "X") {
            break;
        }
        *slot = part
            .parse()
            .map_err(|_| format!("{text:?} is not a release version"))?;
        count += 1;
    }
    if count == 0 && !text.starts_with(['*', 'x', 'X']) {
        return Err(format!("{text:?} is not a release version"));
    }
    Ok((out, count))
}

fn bump(version: Version, index: usize) -> Result<Version, String> {
    let next = version
        .get(index)
        .ok_or_else(|| format!("version part {index} is out of range"))?
        + 1;
    let mut out = [0; 4];
    for (at, (slot, part)) in out.iter_mut().zip(version).enumerate() {
        *slot = if at < index {
            part
        } else if at == index {
            next
        } else {
            0
        };
    }
    Ok(out)
}

fn atom(text: &str) -> Result<Vec<Comparator>, String> {
    if matches!(text, "*" | "") {
        return Ok(Vec::new());
    }
    if text.starts_with("dev-") || text.contains("@dev") || text.contains(" as ") {
        return Err(format!("{text:?} is a branch constraint"));
    }
    let text = text.split('@').next().unwrap_or(text);
    for (prefix, op) in [
        (">=", Op::Ge),
        ("<=", Op::Le),
        ("!=", Op::Ne),
        ("<>", Op::Ne),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("==", Op::Eq),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return Ok(vec![(op, partial(rest.trim())?.0)]);
        }
    }
    if let Some(rest) = text.strip_prefix('^') {
        let (low, count) = partial(rest)?;
        let at = (0..count)
            .find(|&at| low.get(at).is_some_and(|&part| part != 0))
            .unwrap_or_else(|| count.saturating_sub(1));
        return Ok(vec![(Op::Ge, low), (Op::Lt, bump(low, at)?)]);
    }
    if let Some(rest) = text.strip_prefix('~') {
        let (low, count) = partial(rest)?;
        return Ok(vec![
            (Op::Ge, low),
            (Op::Lt, bump(low, count.saturating_sub(2))?),
        ]);
    }
    let (low, count) = partial(text)?;
    if text.contains(['*', 'x', 'X']) {
        return Ok(vec![(Op::Ge, low), (Op::Lt, bump(low, count - 1)?)]);
    }
    Ok(vec![(Op::Eq, low)])
}

fn conjunction(text: &str) -> Result<Vec<Comparator>, String> {
    let words: Vec<&str> = text
        .split([' ', ','])
        .filter(|word| !word.is_empty())
        .collect();
    let mut out = Vec::new();
    let mut rest = words.as_slice();
    loop {
        match rest {
            [low, "-", high, tail @ ..] => {
                let (high, count) = partial(high)?;
                out.push((Op::Ge, partial(low)?.0));
                out.push(if count < 3 {
                    (Op::Lt, bump(high, count - 1)?)
                } else {
                    (Op::Le, high)
                });
                rest = tail;
            }
            [word, tail @ ..] => {
                out.extend(atom(word)?);
                rest = tail;
            }
            [] => break,
        }
    }
    Ok(out)
}

/// A Composer version constraint: alternatives of comparator lists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Constraint(Vec<Vec<Comparator>>);

impl Constraint {
    /// Parses the constraint grammar of release versions.
    ///
    /// # Errors
    /// Fails on branch constraints and on text that is not a version.
    pub fn parse(text: &str) -> Result<Self, String> {
        let alternatives = text
            .split("||")
            .flat_map(|part| part.split(" | "))
            .map(|part| conjunction(part.trim()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(alternatives))
    }

    /// Whether `version` satisfies the constraint.
    #[must_use]
    pub fn allows(&self, version: Version) -> bool {
        self.0.iter().any(|all| {
            all.iter().all(|(op, bound)| match op {
                Op::Ge => version >= *bound,
                Op::Gt => version > *bound,
                Op::Le => version <= *bound,
                Op::Lt => version < *bound,
                Op::Eq => version == *bound,
                Op::Ne => version != *bound,
            })
        })
    }
}

/// The versions of a Packagist `p2` document with the minified fields expanded, newest first.
///
/// # Errors
/// Fails when the document is not a Packagist `p2` document for `name`.
pub fn expand(name: &str, bytes: &[u8]) -> Result<Vec<Map<String, Value>>, Error> {
    let document: Value = serde_json::from_slice(bytes)
        .map_err(|error| Error::new(format!("{name}: metadata: {error}")))?;
    let list = document
        .get("packages")
        .and_then(|packages| packages.get(name))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(format!("{name}: metadata lists no versions")))?;
    let minified = document.get("minified").is_some();
    let mut out: Vec<Map<String, Value>> = Vec::new();
    for entry in list {
        let Some(entry) = entry.as_object() else {
            return Err(Error::new(format!("{name}: metadata has a bad version")));
        };
        let mut full = if minified {
            out.last().cloned().unwrap_or_default()
        } else {
            Map::new()
        };
        for (key, value) in entry {
            if value.as_str() == Some("__unset") {
                full.remove(key);
            } else {
                full.insert(key.clone(), value.clone());
            }
        }
        out.push(full);
    }
    Ok(out)
}

fn string_map(entry: &Map<String, Value>, key: &str) -> BTreeMap<String, String> {
    entry
        .get(key)
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// The PSR-4 namespaces an entry autoloads, without the trailing separator.
#[must_use]
pub fn namespaces(entry: &Map<String, Value>) -> Vec<String> {
    let mut out: Vec<String> = entry
        .get("autoload")
        .and_then(|autoload| autoload.get("psr-4"))
        .and_then(Value::as_object)
        .map(|map| {
            map.keys()
                .map(|key| key.trim_end_matches('\\').to_owned())
                .filter(|key| !key.is_empty())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

const MANIFEST_FIELDS: [&str; 8] = [
    "name",
    "version",
    "version_normalized",
    "type",
    "autoload",
    "require",
    "replace",
    "provide",
];

/// The fields of an entry staged into `vendor/composer/installed.json`.
#[must_use]
pub fn manifest(entry: &Map<String, Value>) -> Value {
    let mut out = Map::new();
    for key in MANIFEST_FIELDS {
        if let Some(value) = entry.get(key).filter(|value| !is_empty(value)) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    out.entry("type").or_insert_with(|| json!("library"));
    Value::Object(out)
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// The resolver of the composer ecosystem.
pub struct Php<'a> {
    manifest: String,
    state: String,
    download: &'a dyn Download,
}

struct Chosen {
    entry: Map<String, Value>,
    version: Version,
}

impl<'a> Php<'a> {
    /// A resolver that downloads with `download`.
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            state: spec.state.first().cloned().unwrap_or_default(),
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

    fn versions(
        &self,
        name: &str,
        cache: &mut BTreeMap<String, Vec<(Version, Map<String, Value>)>>,
    ) -> Result<(), Error> {
        if cache.contains_key(name) {
            return Ok(());
        }
        let bytes = self
            .download
            .bytes(&format!("{METADATA}/{name}.json"))
            .map_err(|error| Error::new(format!("{name}: {error}")))?;
        let mut found: Vec<(Version, Map<String, Value>)> = expand(name, &bytes)?
            .into_iter()
            .filter_map(|entry| {
                let version = parse_version(entry.get("version")?.as_str()?)?;
                Some((version, entry))
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0));
        cache.insert(name.to_owned(), found);
        Ok(())
    }

    /// The packages the roots need, each at the newest release every requirement allows.
    fn select(
        &self,
        roots: &BTreeMap<String, String>,
        old: Option<&Lock>,
    ) -> Result<BTreeMap<String, Chosen>, Error> {
        let kept: BTreeMap<&str, Version> = old
            .map(|lock| {
                lock.packages
                    .iter()
                    .filter_map(|package| Some((package.name.as_str(), parse_version(&package.version)?)))
                    .collect()
            })
            .unwrap_or_default();
        let mut cache = BTreeMap::new();
        let mut chosen: BTreeMap<String, Chosen> = BTreeMap::new();
        for _ in 0..64 {
            let mut wanted: BTreeMap<String, Vec<(String, Constraint)>> = BTreeMap::new();
            let mut queue: Vec<(String, String, String)> = roots
                .iter()
                .map(|(name, constraint)| ("the root".to_owned(), name.clone(), constraint.clone()))
                .collect();
            let mut visited = BTreeSet::new();
            while let Some((from, name, text)) = queue.pop() {
                if is_platform(&name) {
                    continue;
                }
                let constraint = Constraint::parse(&text)
                    .map_err(|error| Error::new(format!("{from} requires {name}: {error}")))?;
                wanted.entry(name.clone()).or_default().push((from, constraint));
                if visited.insert(name.clone())
                    && let Some(package) = chosen.get(&name)
                {
                    for (dep, text) in string_map(&package.entry, "require") {
                        queue.push((name.clone(), dep, text));
                    }
                }
            }
            let mut next: BTreeMap<String, Chosen> = BTreeMap::new();
            for (name, constraints) in &wanted {
                self.versions(name, &mut cache)?;
                let allowed =
                    |version: &Version| constraints.iter().all(|(_, c)| c.allows(*version));
                let releases = cache
                    .get(name)
                    .ok_or_else(|| Error::new(format!("{name}: has no fetched releases")))?;
                let (version, entry) = releases
                    .iter()
                    .find(|(version, _)| {
                        kept.get(name.as_str()) == Some(version) && allowed(version)
                    })
                    .or_else(|| releases.iter().find(|(version, _)| allowed(version)))
                    .ok_or_else(|| {
                        Error::new(format!(
                            "{name}: no stable release satisfies {}",
                            constraints
                                .iter()
                                .map(|(from, _)| from.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;
                next.insert(
                    name.clone(),
                    Chosen {
                        entry: entry.clone(),
                        version: *version,
                    },
                );
            }
            let settled = next.len() == chosen.len()
                && next.iter().all(|(name, package)| {
                    chosen
                        .get(name)
                        .is_some_and(|old| old.version == package.version)
                });
            chosen = next;
            if settled {
                return Ok(chosen);
            }
        }
        Err(Error::new("the requirements do not settle on one version each"))
    }

    fn archive(&self, name: &str, entry: &Map<String, Value>) -> Result<Archive, Error> {
        let dist = entry
            .get("dist")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new(format!("{name} has no dist")))?;
        let url = dist.get("url").and_then(Value::as_str).unwrap_or_default();
        if dist.get("type").and_then(Value::as_str) != Some("zip") || !url.starts_with("https://") {
            return Err(Error::new(format!(
                "{name} has no https zip dist; source-only packages are not supported"
            )));
        }
        let bytes = self
            .download
            .bytes(url)
            .map_err(|error| Error::new(format!("{name}: {url}: {error}")))?;
        let archive = zip::ZipArchive::new(Cursor::new(&bytes))
            .map_err(|error| Error::new(format!("{name}: {url}: {error}")))?;
        let names = archive
            .file_names()
            .map(|member| member.map(std::borrow::Cow::into_owned))
            .collect::<Result<Vec<String>, _>>()
            .map_err(|error| Error::new(format!("{name}: {url}: {error}")))?;
        let tops: BTreeSet<&str> = names
            .iter()
            .filter_map(|member| member.split_once('/').map(|(top, _)| top))
            .collect();
        let strip_prefix = match tops.iter().collect::<Vec<_>>().as_slice() {
            [top] if names.iter().all(|member| member.contains('/')) => (**top).to_owned(),
            _ => String::new(),
        };
        Ok(Archive {
            urls: vec![url.to_owned()],
            sha256: dx_digest::sha256_hex(&bytes),
            strip_prefix,
            platform: String::new(),
            kind: "zip".to_owned(),
        })
    }
}

/// The package of `old` that is `name` at `version` with the archive URL `url`.
fn unchanged<'l>(old: Option<&'l Lock>, name: &str, version: &str, url: &str) -> Option<&'l Package> {
    old?.packages.iter().find(|package| {
        package.name == name
            && package.version == version
            && matches!(package.archives.as_slice(), [archive] if archive.urls == [url])
            && package.extra.contains_key("composer_manifest")
    })
}

fn read_requires(json: &Value, key: &str) -> BTreeMap<String, String> {
    json.get(key)
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

const RELEVANT: [&str; 10] = [
    "name",
    "version",
    "require",
    "require-dev",
    "conflict",
    "replace",
    "provide",
    "minimum-stability",
    "prefer-stable",
    "repositories",
];

/// The `content-hash` Composer records for a manifest.
#[must_use]
pub fn content_hash(json: &Value) -> String {
    let mut content = Map::new();
    for key in RELEVANT.iter().chain(&["extra"]) {
        if let Some(value) = json.get(*key) {
            content.insert((*key).to_owned(), value.clone());
        }
    }
    if let Some(platform) = json.get("config").and_then(|config| config.get("platform")) {
        content.insert("config".to_owned(), json!({ "platform": platform }));
    }
    let text = serde_json::to_string(&Value::Object(content)).unwrap_or_default();
    let digest = Md5::digest(text.replace('/', "\\/").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn state_entry(entry: &Map<String, Value>) -> Value {
    let mut out = entry.clone();
    out.remove("uid");
    out.remove("version_normalized");
    Value::Object(out)
}

fn to_php_json(value: &Value) -> String {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_default();
    let mut text = pretty
        .lines()
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            format!("{}{}", " ".repeat(indent * 2), line.trim_start())
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace('/', "\\/");
    text.push('\n');
    text
}

impl Resolver for Php<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let text = std::fs::read_to_string(scratch.set().join(&self.manifest))
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let production = read_requires(&document, "require");
        let mut roots = production.clone();
        for (name, constraint) in read_requires(&document, "require-dev") {
            roots.entry(name).or_insert(constraint);
        }
        let chosen = self.select(&roots, old)?;
        for (name, package) in &chosen {
            if package.entry.get("type").and_then(Value::as_str) == Some("composer-plugin") {
                return Err(Error::new(format!("{name} is a composer-plugin package")));
            }
        }
        let stability = document
            .get("minimum-stability")
            .and_then(Value::as_str)
            .unwrap_or("stable");
        if stability != "stable" {
            return Err(Error::new(format!(
                "minimum-stability {stability:?} is not supported; only stable releases lock"
            )));
        }
        let platform = |key: &str| -> Value {
            let required: Map<String, Value> = read_requires(&document, key)
                .into_iter()
                .filter(|(name, _)| is_platform(name))
                .map(|(name, constraint)| (name, Value::String(constraint)))
                .collect();
            if required.is_empty() { json!([]) } else { Value::Object(required) }
        };        let mut provided: BTreeMap<String, String> = BTreeMap::new();
        for (name, package) in &chosen {
            for key in ["replace", "provide"] {
                for virtual_name in string_map(&package.entry, key).into_keys() {
                    provided.entry(virtual_name).or_insert_with(|| name.clone());
                }
            }
        }
        let edges = |name: &str| -> Vec<String> {
            chosen
                .get(name)
                .map_or_else(BTreeMap::new, |package| {
                    string_map(&package.entry, "require")
                })
                .into_keys()
                .filter(|dep| !is_platform(dep))
                .filter_map(|dep| {
                    chosen
                        .contains_key(&dep)
                        .then_some(dep.clone())
                        .or_else(|| provided.get(&dep).cloned())
                })
                .filter(|dep| dep != name)
                .collect()
        };
        let mut production_set = BTreeSet::new();
        let mut queue: Vec<String> = production
            .keys()
            .filter(|name| chosen.contains_key(*name))
            .cloned()
            .collect();
        while let Some(name) = queue.pop() {
            if production_set.insert(name.clone()) {
                queue.extend(edges(&name));
            }
        }
        let mut packages = Vec::new();
        let mut locked: Vec<Value> = Vec::new();
        let mut locked_dev: Vec<Value> = Vec::new();
        for (name, package) in &chosen {
            let version = package
                .entry
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let url = package
                .entry
                .get("dist")
                .and_then(|dist| dist.get("url"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let dev = !production_set.contains(name);
            let kept = unchanged(old, name, &version, url)
                .and_then(|kept| Some((kept.archives.first()?.clone(), kept.extra.clone())));
            let (archive, extra) = match kept {
                Some(found) => found,
                None => {
                    let mut extra = BTreeMap::new();
                    extra.insert("composer_manifest".to_owned(), manifest(&package.entry));
                    (self.archive(name, &package.entry)?, extra)
                }
            };
            packages.push(Package {
                name: name.clone(),
                version,
                archives: vec![archive],
                deps: edges(name),
                provides: namespaces(&package.entry),
                dev,
                extra,
            });
            if dev {
                locked_dev.push(state_entry(&package.entry));
            } else {
                locked.push(state_entry(&package.entry));
            }
        }
        let state = json!({
            "_readme": [
                "This file locks the dependencies of your project to a known state",
                "Read more about it at https://getcomposer.org/doc/01-basic-usage.md#installing-dependencies",
                "This file is @generated automatically",
            ],
            "content-hash": content_hash(&document),
            "packages": locked,
            "packages-dev": locked_dev,
            "aliases": [],
            "minimum-stability": stability,
            "stability-flags": [],
            "prefer-stable": document.get("prefer-stable").and_then(Value::as_bool).unwrap_or(false),
            "prefer-lowest": false,
            "platform": platform("require"),
            "platform-dev": platform("require-dev"),
            "plugin-api-version": "2.6.0",
        });
        std::fs::write(scratch.set().join(&self.state), to_php_json(&state))
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        Ok(Resolution { packages })
    }
}
