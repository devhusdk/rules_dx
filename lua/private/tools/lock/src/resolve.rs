//! Resolves the rocks a rockspec depends on from the luarocks.org manifest and the rockspecs it serves.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod rockspec;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use rockspec::Value;
use serde_json::{Value as Json, json};

/// The site rocks are resolved from.
pub const BASE: &str = "https://luarocks.org";

/// The manifest of the Lua version the rules build for.
pub const MANIFEST: &str = "manifest-5.5.json";

/// Downloads files.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Site(HttpFetch);

impl Site {
    /// A downloader that refuses plain HTTP.
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

impl Download for Site {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

const WIDTH: usize = 16;

/// A rock version: numeric and named parts, then the rock revision.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Version {
    parts: Vec<(i64, u64)>,
    count: usize,
    revision: Option<u64>,
}

impl Version {
    fn parse(text: &str) -> Self {
        let (core, revision) = match text.rsplit_once('-') {
            Some((head, tail))
                if !head.is_empty()
                    && tail.bytes().all(|b| b.is_ascii_digit())
                    && !tail.is_empty() =>
            {
                (head, tail.parse().ok())
            }
            _ => (text, None),
        };
        let mut parts = Vec::new();
        let mut token = String::new();
        let flush = |token: &mut String, parts: &mut Vec<(i64, u64)>| {
            if token.is_empty() {
                return;
            }
            if let Ok(number) = token.parse::<u64>() {
                parts.push((0, number));
            } else {
                let rank = match token.to_ascii_lowercase().as_str() {
                    "alpha" => -1_000_000,
                    "beta" => -100_000,
                    "pre" => -10_000,
                    "rc" => -1_000,
                    "dev" | "scm" | "cvs" | "git" | "svn" => -100,
                    _ => -50,
                };
                parts.push((rank, 0));
            }
            token.clear();
        };
        let mut digits = None;
        for c in core.chars() {
            if !c.is_ascii_alphanumeric() {
                flush(&mut token, &mut parts);
                digits = None;
                continue;
            }
            if digits.is_some_and(|was| was != c.is_ascii_digit()) {
                flush(&mut token, &mut parts);
            }
            digits = Some(c.is_ascii_digit());
            token.push(c);
        }
        flush(&mut token, &mut parts);
        let count = parts.len();
        parts.resize(WIDTH.max(count), (0, 0));
        Self {
            parts,
            count,
            revision,
        }
    }

    fn release(&self) -> bool {
        self.parts.iter().all(|(rank, _)| *rank >= 0)
    }

    fn core_cmp(&self, other: &Self) -> Ordering {
        let width = self.parts.len().max(other.parts.len());
        for at in 0..width {
            let left = self.parts.get(at).copied().unwrap_or((0, 0));
            let right = other.parts.get(at).copied().unwrap_or((0, 0));
            match left.cmp(&right) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        Ordering::Equal
    }

    fn full_cmp(&self, other: &Self) -> Ordering {
        self.core_cmp(other)
            .then(self.revision.unwrap_or(0).cmp(&other.revision.unwrap_or(0)))
    }

    fn against(&self, wanted: &Self) -> Ordering {
        match wanted.revision {
            Some(_) => self.full_cmp(wanted),
            None => self.core_cmp(wanted),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Eq,
    Ne,
    Ge,
    Gt,
    Le,
    Lt,
    Pessimistic,
}

/// One version bound of a dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Bound {
    op: Op,
    version: Version,
}

impl Bound {
    fn allows(&self, candidate: &Version) -> bool {
        let order = candidate.against(&self.version);
        match self.op {
            Op::Eq => order == Ordering::Equal,
            Op::Ne => order != Ordering::Equal,
            Op::Ge => order != Ordering::Less,
            Op::Gt => order == Ordering::Greater,
            Op::Le => order != Ordering::Greater,
            Op::Lt => order == Ordering::Less,
            Op::Pessimistic => {
                let keep = self.version.count.saturating_sub(1).max(1);
                order != Ordering::Less
                    && candidate.parts.get(..keep) == self.version.parts.get(..keep)
            }
        }
    }
}

/// A dependency: a rock and the bounds on its version.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Need {
    name: String,
    bounds: Vec<Bound>,
}

fn parse_need(entry: &str) -> Result<Need, String> {
    let entry = entry.trim();
    let end = entry
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .unwrap_or(entry.len());
    let (name, rest) = entry.split_at(end);
    if name.is_empty() {
        return Err(format!("{entry:?} names no rock"));
    }
    let mut bounds = Vec::new();
    for piece in rest
        .split(',')
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
    {
        let (op, version) = [
            ("~>", Op::Pessimistic),
            ("==", Op::Eq),
            ("~=", Op::Ne),
            (">=", Op::Ge),
            ("<=", Op::Le),
            (">", Op::Gt),
            ("<", Op::Lt),
            ("=", Op::Eq),
        ]
        .iter()
        .find_map(|(text, op)| {
            piece
                .strip_prefix(text)
                .map(|version| (*op, version.trim()))
        })
        .unwrap_or((Op::Eq, piece));
        if version.is_empty() {
            return Err(format!("{entry:?} has a bound without a version"));
        }
        bounds.push(Bound {
            op,
            version: Version::parse(version),
        });
    }
    Ok(Need {
        name: name.to_owned(),
        bounds,
    })
}

fn needs_of(table: Option<&Value>) -> Result<Vec<Need>, String> {
    let mut needs = Vec::new();
    for entry in table
        .and_then(Value::table)
        .map_or(&[][..], |table| &table.list[..])
    {
        let text = entry.text().ok_or("a dependency is not a string")?;
        let need = parse_need(text)?;
        if need.name != "lua" {
            needs.push(need);
        }
    }
    Ok(needs)
}

/// What a rockspec says about one rock.
struct Described {
    urls: Vec<String>,
    strip_prefix: String,
    modules: BTreeMap<String, String>,
    deps: Vec<Need>,
}

fn archive_dir(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    for suffix in [".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".zip"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    name.to_owned()
}

fn github_repo(url: &str) -> Option<(String, String)> {
    let rest = [
        "git+https://github.com/",
        "git://github.com/",
        "git+ssh://git@github.com/",
    ]
    .iter()
    .find_map(|prefix| url.strip_prefix(prefix))?;
    let (owner, repo) = rest.split_once('/')?;
    if repo.contains('/') {
        return None;
    }
    Some((
        owner.to_owned(),
        repo.strip_suffix(".git").unwrap_or(repo).to_owned(),
    ))
}

fn source_of(rock: &str, source: Option<&Value>) -> Result<(Vec<String>, String), String> {
    let table = source.and_then(Value::table);
    let field = |name: &str| {
        table
            .and_then(|table| table.map.get(name))
            .and_then(Value::text)
    };
    let url = field("url").ok_or_else(|| format!("{rock}: source.url is missing"))?;
    if let Some((owner, repo)) = github_repo(url) {
        let tag = field("tag").ok_or_else(|| {
            format!("{rock}: source {url} has no tag; only GitHub tags are supported")
        })?;
        let dir = field("dir").map_or_else(
            || format!("{repo}-{}", tag.strip_prefix('v').unwrap_or(tag)),
            str::to_owned,
        );
        return Ok((
            vec![format!(
                "https://github.com/{owner}/{repo}/archive/refs/tags/{tag}.tar.gz"
            )],
            dir,
        ));
    }
    if !url.starts_with("https://") || url.ends_with(".git") {
        return Err(format!(
            "{rock}: source {url} is not an https archive URL or a GitHub tag"
        ));
    }
    Ok((
        vec![url.to_owned()],
        field("dir").map_or_else(|| archive_dir(url), str::to_owned),
    ))
}

fn module_path(module: &str) -> String {
    format!("{}.lua", module.replace('.', "/"))
}

fn describe(rock: &str, version: &str, text: &str) -> Result<Described, String> {
    let globals = rockspec::parse(text)?;
    let package = globals.get("package").and_then(Value::text);
    let found = globals.get("version").and_then(Value::text);
    if package != Some(rock) || found != Some(version) {
        return Err(format!(
            "{rock} {version}: the rockspec describes {} {}",
            package.unwrap_or("?"),
            found.unwrap_or("?")
        ));
    }
    let (urls, strip_prefix) = source_of(rock, globals.get("source"))?;
    let build = globals.get("build").and_then(Value::table);
    let kind = build
        .and_then(|build| build.map.get("type"))
        .and_then(Value::text)
        .unwrap_or("builtin");
    if kind != "builtin" {
        return Err(format!(
            "{rock}: build.type {kind} is not supported; only builtin"
        ));
    }
    let mut modules = BTreeMap::new();
    let listed = build
        .and_then(|build| build.map.get("modules"))
        .and_then(Value::table);
    for (module, path) in listed.map_or_else(BTreeMap::new, |table| table.map.clone()) {
        let path = path.text().ok_or_else(|| {
            format!("{rock}: module {module} is a C module; only Lua sources are supported")
        })?;
        if path.starts_with('/') || path.split('/').any(|part| part == "..") {
            return Err(format!(
                "{rock}: module {module} has the path {path:?}; it must be a relative path inside the source"
            ));
        }
        modules.insert(module, path.to_owned());
    }
    if modules.is_empty() {
        return Err(format!("{rock}: build.modules is empty"));
    }
    root_of(rock, &modules)?;
    Ok(Described {
        urls,
        strip_prefix,
        modules,
        deps: needs_of(globals.get("dependencies"))?,
    })
}

/// The directory of the source the module names start from.
fn root_of(rock: &str, modules: &BTreeMap<String, String>) -> Result<String, String> {
    let mut root: Option<String> = None;
    for (module, path) in modules {
        let canonical = module_path(module);
        let prefix = path
            .strip_suffix(&canonical)
            .filter(|prefix| prefix.is_empty() || prefix.ends_with('/'))
            .map(|prefix| prefix.trim_end_matches('/').to_owned())
            .ok_or_else(|| {
                format!("{rock}: module {module} is {path}; it must end in {canonical}")
            })?;
        if root.as_ref().is_some_and(|known| *known != prefix) {
            return Err(format!("{rock}: modules do not share one source directory"));
        }
        root = Some(prefix);
    }
    Ok(root.unwrap_or_default())
}

/// The resolver of the luarocks ecosystem.
pub struct Lua<'a> {
    manifest: String,
    base: String,
    download: &'a dyn Download,
}

impl<'a> Lua<'a> {
    /// A resolver that downloads from `base` with `download`.
    #[must_use]
    pub fn new(spec: &Spec, base: &str, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            base: base.to_owned(),
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

    fn text(&self, name: &str) -> Result<String, Error> {
        let url = format!("{}/{name}", self.base);
        let bytes = self
            .download
            .bytes(&url)
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        String::from_utf8(bytes).map_err(|_| Error::new(format!("{url} is not UTF-8")))
    }

    fn releases(&self, index: &Json, rock: &str) -> Result<Vec<(Version, String)>, Error> {
        let versions = index
            .get("repository")
            .and_then(|repository| repository.get(rock))
            .and_then(Json::as_object)
            .ok_or_else(|| Error::new(format!("{rock} is not in {MANIFEST}")))?;
        Ok(versions
            .keys()
            .map(|text| (Version::parse(text), text.clone()))
            .filter(|(version, _)| version.release())
            .collect())
    }
}

fn locked(old: Option<&Lock>, need: &Need) -> Option<Package> {
    let package = old?
        .packages
        .iter()
        .find(|package| package.name == need.name)?;
    let version = Version::parse(&package.version);
    (package.extra.contains_key("lua_modules")
        && need.bounds.iter().all(|bound| bound.allows(&version)))
    .then(|| package.clone())
}

impl Resolver for Lua<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let (head, tail) = self
            .manifest
            .split_once('*')
            .unwrap_or((&self.manifest, ""));
        let mut names: Vec<String> = std::fs::read_dir(scratch.set())
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| {
                name.len() >= head.len() + tail.len()
                    && name.starts_with(head)
                    && name.ends_with(tail)
            })
            .collect();
        names.sort();
        if names.is_empty() {
            return Err(Error::new(format!("no {} in the set", self.manifest)));
        }
        let mut queue: Vec<Need> = Vec::new();
        for name in &names {
            let text = std::fs::read_to_string(scratch.set().join(name))
                .map_err(|error| Error::new(format!("{name}: {error}")))?;
            let globals =
                rockspec::parse(&text).map_err(|error| Error::new(format!("{name}: {error}")))?;
            queue.extend(
                needs_of(globals.get("dependencies"))
                    .map_err(|error| Error::new(format!("{name}: {error}")))?,
            );
        }
        let mut wanted: BTreeMap<String, Vec<Bound>> = BTreeMap::new();
        let mut chosen: BTreeMap<String, Package> = BTreeMap::new();
        let mut index: Option<Json> = None;
        while let Some(need) = queue.pop() {
            let bounds = wanted.entry(need.name.clone()).or_default();
            bounds.extend(need.bounds.iter().cloned());
            let bounds = bounds.clone();
            let need = Need {
                name: need.name,
                bounds,
            };
            if let Some(package) = chosen.get(&need.name) {
                let version = Version::parse(&package.version);
                if need.bounds.iter().all(|bound| bound.allows(&version)) {
                    continue;
                }
                return Err(Error::new(format!(
                    "{} {} does not satisfy every dependency on {}",
                    need.name, package.version, need.name
                )));
            }
            if let Some(package) = locked(old, &need) {
                queue.extend(package.deps.iter().map(|name| Need {
                    name: name.clone(),
                    bounds: Vec::new(),
                }));
                chosen.insert(need.name, package);
                continue;
            }
            if index.is_none() {
                index = Some(
                    serde_json::from_str(&self.text(MANIFEST)?)
                        .map_err(|error| Error::new(format!("{MANIFEST}: {error}")))?,
                );
            }
            let mut releases = self.releases(index.as_ref().unwrap_or(&Json::Null), &need.name)?;
            releases.retain(|(version, _)| need.bounds.iter().all(|bound| bound.allows(version)));
            releases.sort_by(|left, right| left.0.full_cmp(&right.0));
            let (_, version) = releases.pop().ok_or_else(|| {
                Error::new(format!(
                    "{} has no release that satisfies its dependencies",
                    need.name
                ))
            })?;
            let spec = self.text(&format!("{}-{version}.rockspec", need.name))?;
            let described = describe(&need.name, &version, &spec).map_err(Error::new)?;
            let mut deps: Vec<String> = described.deps.iter().map(|dep| dep.name.clone()).collect();
            deps.sort();
            deps.dedup();
            queue.extend(described.deps);
            let provides: Vec<String> = described.modules.keys().cloned().collect();
            let mut extra = BTreeMap::new();
            extra.insert("lua_modules".to_owned(), json!(described.modules));
            chosen.insert(
                need.name.clone(),
                Package {
                    name: need.name,
                    version,
                    archives: vec![Archive {
                        urls: described.urls,
                        sha256: String::new(),
                        strip_prefix: described.strip_prefix,
                        platform: String::new(),
                        kind: String::new(),
                    }],
                    deps,
                    provides,
                    dev: false,
                    extra,
                },
            );
        }
        let known: BTreeSet<&String> = chosen.keys().collect();
        for package in chosen.values() {
            if let Some(missing) = package.deps.iter().find(|dep| !known.contains(dep)) {
                return Err(Error::new(format!(
                    "{} depends on {missing}, which is not locked",
                    package.name
                )));
            }
        }
        Ok(Resolution {
            groups: std::collections::BTreeMap::new(),
            packages: chosen.into_values().collect(),
        })
    }
}
