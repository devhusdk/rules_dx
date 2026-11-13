//! Resolves a DUB package set from the `dependencies` of its manifest and the code.dlang.org registry.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

/// The registry API and archive host.
pub const BASE: &str = "https://code.dlang.org";

/// Downloads registry answers and archives.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Registry(HttpFetch);

impl Registry {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Registry {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// The URL of the version list of a package.
#[must_use]
pub fn versions_url(name: &str) -> String {
    format!("{BASE}/api/packages/{name}/versions")
}

/// The URL of the archive of a package version.
#[must_use]
pub fn archive_url(name: &str, version: &str) -> String {
    format!("{BASE}/packages/{name}/{version}.zip")
}

/// A version split into its parts.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Version {
    numbers: [u64; 3],
    pre: Option<String>,
}

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre.to_owned())),
            None => (text, None),
        };
        let mut numbers = [0; 3];
        let mut parts = core.split('.');
        for slot in &mut numbers {
            match parts.next() {
                Some(part) => *slot = part.parse().ok()?,
                None => break,
            }
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Self { numbers, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numbers
            .cmp(&other.numbers)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One bound of a version range.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Bound {
    Any,
    Cmp(&'static str, Version),
    Compatible(Version, usize),
}

/// A version range: every bound must hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Range {
    text: String,
    bounds: Vec<Bound>,
}

impl Range {
    /// Parses a DUB version specification.
    ///
    /// # Errors
    /// Fails on a branch, a path or any text that is not a version range.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut bounds = Vec::new();
        let text = text.trim();
        let bad = || Error::new(format!("{text:?} is not a supported version range"));
        if text == "*" {
            bounds.push(Bound::Any);
        }
        let mut tokens = text.split_whitespace().peekable();
        while let Some(token) = tokens.next() {
            if token == "*" {
                continue;
            }
            let (op, rest) = ["~>", ">=", "<=", "==", ">", "<"]
                .iter()
                .find_map(|op| token.strip_prefix(op).map(|rest| (*op, rest)))
                .unwrap_or(("==", token));
            let rest = if rest.is_empty() {
                tokens.next().ok_or_else(bad)?
            } else {
                rest
            };
            let version = Version::parse(rest).ok_or_else(bad)?;
            bounds.push(match op {
                "~>" => Bound::Compatible(version, rest.split('.').count()),
                ">=" => Bound::Cmp(">=", version),
                "<=" => Bound::Cmp("<=", version),
                ">" => Bound::Cmp(">", version),
                "<" => Bound::Cmp("<", version),
                _ => Bound::Cmp("==", version),
            });
        }
        if bounds.is_empty() {
            return Err(bad());
        }
        Ok(Self {
            text: text.to_owned(),
            bounds,
        })
    }

    fn allows(&self, version: &Version) -> bool {
        self.bounds.iter().all(|bound| match bound {
            Bound::Any => true,
            Bound::Cmp(op, wanted) => match *op {
                ">=" => version >= wanted,
                "<=" => version <= wanted,
                ">" => version > wanted,
                "<" => version < wanted,
                _ => version == wanted,
            },
            Bound::Compatible(wanted, parts) => {
                let keep = if *parts >= 3 { 2 } else { 1 };
                version >= wanted && version.numbers.get(..keep) == wanted.numbers.get(..keep)
            }
        })
    }

    fn names_prerelease(&self) -> bool {
        self.bounds.iter().any(|bound| match bound {
            Bound::Any => false,
            Bound::Cmp(_, version) | Bound::Compatible(version, _) => version.pre.is_some(),
        })
    }
}

/// The dependencies of a manifest, by base package name; optional ones are skipped.
///
/// # Errors
/// Fails on a dependency selected by path or repository, or with an unsupported range.
pub fn parse_dependencies(manifest: &Value) -> Result<BTreeMap<String, Vec<Range>>, Error> {
    let mut deps: BTreeMap<String, Vec<Range>> = BTreeMap::new();
    let Some(entries) = manifest.get("dependencies").and_then(Value::as_object) else {
        return Ok(deps);
    };
    for (key, value) in entries {
        let name = key.split(':').next().unwrap_or(key);
        let text = match value {
            Value::String(text) => text.as_str(),
            Value::Object(fields) => {
                if fields.contains_key("path") || fields.contains_key("repository") {
                    return Err(Error::new(format!(
                        "dependency {key} is selected by path or repository; only registry versions are supported"
                    )));
                }
                if fields.get("optional").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                fields.get("version").and_then(Value::as_str).unwrap_or("*")
            }
            _ => {
                return Err(Error::new(format!("dependency {key} has no version")));
            }
        };
        let range = Range::parse(text).map_err(|error| Error::new(format!("dependency {key}: {error}")))?;
        let ranges = deps.entry(name.to_owned()).or_default();
        if !ranges.contains(&range) {
            ranges.push(range);
        }
    }
    Ok(deps)
}

/// The resolver of the dub ecosystem.
pub struct Dub<'a> {
    manifest: String,
    state: String,
    download: &'a dyn Download,
}

/// What one downloaded archive holds.
#[derive(Clone)]
struct Described {
    package: Package,
    nested: BTreeMap<String, Vec<Range>>,
}

impl<'a> Dub<'a> {
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

    fn versions(&self, name: &str) -> Result<Vec<(Version, String)>, Error> {
        let url = versions_url(name);
        let body = self
            .download
            .bytes(&url)
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        let list: Vec<String> = serde_json::from_slice(&body)
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        Ok(list
            .into_iter()
            .filter_map(|text| Some((Version::parse(&text)?, text)))
            .collect())
    }

    fn pick(
        &self,
        name: &str,
        ranges: &[Range],
        selected: Option<&str>,
        listed: &mut BTreeMap<String, Vec<(Version, String)>>,
    ) -> Result<String, Error> {
        let prerelease = ranges.iter().any(Range::names_prerelease);
        let allowed = |version: &Version| {
            (prerelease || version.pre.is_none()) && ranges.iter().all(|range| range.allows(version))
        };
        if let Some(text) = selected
            && Version::parse(text).is_some_and(|version| allowed(&version))
        {
            return Ok(text.to_owned());
        }
        if !listed.contains_key(name) {
            listed.insert(name.to_owned(), self.versions(name)?);
        }
        listed[name]
            .iter()
            .filter(|(version, _)| allowed(version))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, text)| text.clone())
            .ok_or_else(|| {
                let wanted: Vec<&str> = ranges.iter().map(|range| range.text.as_str()).collect();
                Error::new(format!("no version of {name} satisfies {}", wanted.join(" and ")))
            })
    }

    fn describe(&self, name: &str, version: &str) -> Result<Described, Error> {
        let failed = |message: String| Error::new(format!("package {name} {version}: {message}"));
        let url = archive_url(name, version);
        let bytes = self
            .download
            .bytes(&url)
            .map_err(|error| failed(format!("{url}: {error}")))?;
        let members = read_members(&bytes, &self.manifest).map_err(|error| failed(format!("{url}: {error}")))?;
        let tops: BTreeSet<&str> = members
            .names
            .iter()
            .filter_map(|member| member.split_once('/').map(|(top, _)| top))
            .collect();
        let strip_prefix = match tops.iter().collect::<Vec<_>>().as_slice() {
            [top] if members.names.iter().all(|member| member.contains('/')) => (**top).to_owned(),
            _ => String::new(),
        };
        let path = |file: &str| {
            if strip_prefix.is_empty() {
                file.to_owned()
            } else {
                format!("{strip_prefix}/{file}")
            }
        };
        let relative: Vec<String> = members
            .names
            .iter()
            .filter_map(|member| {
                if strip_prefix.is_empty() {
                    Some(member.clone())
                } else {
                    member.strip_prefix(&format!("{strip_prefix}/")).map(str::to_owned)
                }
            })
            .collect();
        let manifest: Value = if let Some(bytes) = members.files.get(&path(&self.manifest)) {
            parse_manifest(&self.manifest, bytes).map_err(failed)?
        } else if relative.iter().any(|member| member == "dub.sdl") {
            return Err(failed(
                format!("uses dub.sdl, which is not supported yet; only {} packages can be locked", self.manifest),
            ));
        } else {
            return Err(failed(format!("has no {}", self.manifest)));
        };
        let dirs: BTreeSet<&str> = relative
            .iter()
            .filter_map(|member| member.split_once('/').map(|(top, _)| top))
            .collect();
        let strings = |key: &str| -> Vec<String> {
            manifest
                .get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|item| item.trim_end_matches('/').to_owned())
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut source_paths = strings("sourcePaths");
        if source_paths.is_empty() {
            source_paths = ["source", "src"]
                .into_iter()
                .filter(|dir| dirs.contains(dir))
                .take(1)
                .map(str::to_owned)
                .collect();
        }
        let source_files = strings("sourceFiles");
        let source_files: Vec<String> = source_files.into_iter().map(|file| file.trim_start_matches("./").to_owned()).collect();
        if source_paths.is_empty() && source_files.is_empty() {
            return Err(failed(
                format!("has no sourcePaths or sourceFiles in {} and no source or src directory", self.manifest),
            ));
        }
        let mut imports = strings("importPaths");
        if imports.is_empty() {
            imports.clone_from(&source_paths);
        }
        let mut string_imports = strings("stringImportPaths");
        if string_imports.is_empty() && dirs.contains("views") {
            string_imports.push("views".to_owned());
        }
        let mut srcs = Vec::new();
        for dir in &source_paths {
            srcs.push(format!("{dir}/**/*.d"));
            srcs.push(format!("{dir}/**/*.di"));
        }
        srcs.extend(source_files.clone());
        let mut provides = BTreeSet::new();
        for dir in &imports {
            let prefix = if dir == "." { String::new() } else { format!("{dir}/") };
            for member in &relative {
                let Some(rest) = member.strip_prefix(&prefix) else {
                    continue;
                };
                let compiled = source_paths
                    .iter()
                    .any(|source| member.starts_with(&format!("{source}/")))
                    || source_files.contains(member);
                if !compiled {
                    continue;
                }
                let root = match rest.split_once('/') {
                    Some((root, _)) => root,
                    None => match rest.strip_suffix(".d").or_else(|| rest.strip_suffix(".di")) {
                        Some(root) => root,
                        None => continue,
                    },
                };
                provides.insert(root.to_owned());
            }
        }
        if provides.is_empty() {
            provides.insert(name.to_owned());
        }
        let mut layout = serde_json::Map::new();
        layout.insert("imports".to_owned(), json!(imports));
        layout.insert("srcs".to_owned(), json!(srcs));
        if !string_imports.is_empty() {
            layout.insert("string_imports".to_owned(), json!(string_imports));
        }
        let mut extra = BTreeMap::new();
        extra.insert("d_library".to_owned(), Value::Object(layout));
        let nested = parse_dependencies(&manifest).map_err(|error| failed(error.to_string()))?;
        Ok(Described {
            nested: nested.clone(),
            package: Package {
                name: name.to_owned(),
                version: version.to_owned(),
                archives: vec![Archive {
                    urls: vec![url],
                    sha256: dx_digest::sha256_hex(&bytes),
                    strip_prefix,
                    platform: String::new(),
                    kind: String::new(),
                }],
                deps: nested.into_keys().collect(),
                provides: provides.into_iter().collect(),
                dev: false,
                extra,
            },
        })
    }
}

fn parse_manifest(name: &str, bytes: &[u8]) -> Result<Value, String> {
    let text = String::from_utf8_lossy(bytes);
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| format!("{name}: {error}"))
}

/// The most bytes read from one archive member.
const MEMBER_LIMIT: u64 = 8 << 20;

/// The regular files of an archive and the contents of its manifests.
struct Members {
    names: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
}

fn read_members(bytes: &[u8], manifest: &str) -> Result<Members, String> {
    let mut members = Members {
        names: BTreeSet::new(),
        files: BTreeMap::new(),
    };
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if !entry.is_file() || entry.is_symlink() {
            continue;
        }
        let name = entry.name().map_err(|error| error.to_string())?.into_owned();
        let name = name.trim_start_matches("./").to_owned();
        let base = name.rsplit('/').next().unwrap_or(&name);
        let depth = name.matches('/').count();
        if depth <= 1 && base == manifest {
            let mut body = Vec::new();
            (&mut entry)
                .take(MEMBER_LIMIT)
                .read_to_end(&mut body)
                .map_err(|error| format!("{name}: {error}"))?;
            members.files.insert(name.clone(), body);
        }
        members.names.insert(name);
    }
    Ok(members)
}

/// The package of `old` that has the name and version.
fn unchanged<'l>(old: &'l Lock, name: &str, version: &str) -> Option<&'l Package> {
    old.packages.iter().find(|package| {
        package.name == name
            && package.version == version
            && matches!(package.archives.as_slice(), [archive] if archive.urls == [archive_url(name, version)])
            && package.extra.contains_key("d_library")
    })
}

/// The versions the previous run selected.
fn selections(text: &str) -> BTreeMap<String, String> {
    let Ok(Value::Object(data)) = serde_json::from_str::<Value>(text) else {
        return BTreeMap::new();
    };
    data.get("versions")
        .and_then(Value::as_object)
        .map(|versions| {
            versions
                .iter()
                .filter_map(|(name, version)| Some((name.clone(), version.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

fn selections_text(versions: &BTreeMap<String, String>) -> String {
    let mut text = String::from("{\n\t\"fileVersion\": 1,\n\t\"versions\": {");
    let lines: Vec<String> = versions
        .iter()
        .map(|(name, version)| format!("\n\t\t{}: {}", json!(name), json!(version)))
        .collect();
    text.push_str(&lines.join(","));
    if !lines.is_empty() {
        text.push_str("\n\t");
    }
    text.push_str("}\n}\n");
    text
}

const ROUNDS: usize = 64;

impl Resolver for Dub<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let read = |name: &str| {
            std::fs::read_to_string(scratch.set().join(name)).map_err(|error| Error::new(format!("{name}: {error}")))
        };
        let manifest = parse_manifest(&self.manifest, read(&self.manifest)?.as_bytes())
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let root = parse_dependencies(&manifest).map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let previous = std::fs::read_to_string(scratch.set().join(&self.state))
            .map(|text| selections(&text))
            .unwrap_or_default();
        let any = Range::parse("*")?;
        let mut listed = BTreeMap::new();
        let mut described: BTreeMap<(String, String), Described> = BTreeMap::new();
        let mut chosen: BTreeMap<String, String> = BTreeMap::new();
        for _ in 0..ROUNDS {
            let mut wanted: BTreeMap<String, Vec<Range>> = root.clone();
            let mut queue: Vec<String> = root.keys().cloned().collect();
            let mut seen: BTreeSet<String> = BTreeSet::new();
            while let Some(name) = queue.pop() {
                if !seen.insert(name.clone()) {
                    continue;
                }
                let Some(version) = chosen.get(&name) else {
                    continue;
                };
                let Some(found) = described.get(&(name.clone(), version.clone())) else {
                    return Err(Error::new(format!(
                        "{name} {version} was chosen but not described"
                    )));
                };
                for (dep, ranges) in &found.nested {
                    let entry = wanted.entry(dep.clone()).or_default();
                    for range in ranges {
                        if !entry.contains(range) {
                            entry.push(range.clone());
                        }
                    }
                    queue.push(dep.clone());
                }
            }
            let mut next: BTreeMap<String, String> = BTreeMap::new();
            for (name, ranges) in &wanted {
                let selected = chosen.get(name).or_else(|| previous.get(name)).map(String::as_str);
                let version = self.pick(name, ranges, selected, &mut listed)?;
                let key = (name.clone(), version.clone());
                if !described.contains_key(&key) {
                    let entry = match old.and_then(|lock| unchanged(lock, name, &version)) {
                        Some(package) => Described {
                            nested: package.deps.iter().map(|dep| (dep.clone(), vec![any.clone()])).collect(),
                            package: package.clone(),
                        },
                        None => self.describe(name, &version)?,
                    };
                    described.insert(key, entry);
                }
                next.insert(name.clone(), version);
            }
            if next == chosen {
                break;
            }
            chosen = next;
        }
        let state = selections_text(&chosen);
        std::fs::write(scratch.set().join(&self.state), state)
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        Ok(Resolution {
            packages: chosen
                .iter()
                .map(|(name, version)| {
                    described
                        .get(&(name.clone(), version.clone()))
                        .map(|found| found.package.clone())
                        .ok_or_else(|| {
                            Error::new(format!("{name} {version} was chosen but not described"))
                        })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}
