//! Resolves a Zig package set from the `.dependencies` of its manifest and the archives they name.

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

/// Downloads archives.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Github(HttpFetch);

impl Github {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Github {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Github {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// The root files tried for the module of a package, best first.
#[must_use]
pub fn main_candidates(name: &str) -> Vec<String> {
    vec![
        format!("src/{name}.zig"),
        "src/root.zig".to_owned(),
        format!("{name}.zig"),
        "src/main.zig".to_owned(),
        "root.zig".to_owned(),
        "main.zig".to_owned(),
    ]
}

/// One entry of a `.dependencies` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    /// The dependency name.
    pub name: String,
    /// The archive URL.
    pub url: String,
    /// The `.hash` field.
    pub hash: String,
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut quoted = false;
            let mut escaped = false;
            let mut previous = '\0';
            for (at, ch) in line.char_indices() {
                if !quoted && ch == '/' && previous == '/' {
                    return &line[..at - 1];
                }
                if escaped {
                    escaped = false;
                } else if quoted && ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quoted = !quoted;
                }
                previous = if quoted { '\0' } else { ch };
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The index of the `}` that closes the brace opened just before `start`.
fn matching_brace(text: &str, start: usize) -> usize {
    let mut depth = 1;
    let mut quoted = false;
    let mut escaped = false;
    for (at, ch) in text[start..].char_indices() {
        if escaped {
            escaped = false;
        } else if quoted && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if !quoted && ch == '{' {
            depth += 1;
        } else if !quoted && ch == '}' {
            depth -= 1;
            if depth == 0 {
                return start + at;
            }
        }
    }
    text.len()
}

fn string_field(body: &str, field: &str) -> String {
    let key = format!(".{field}");
    let mut from = 0;
    while let Some(found) = body[from..].find(&key) {
        let after = from + found + key.len();
        from = after;
        let Some(rest) = body[after..].trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            return String::new();
        };
        return rest
            .find('"')
            .map_or_else(String::new, |end| rest[..end].to_owned());
    }
    String::new()
}

fn dependency_name(head: &str) -> String {
    let head = head.trim().trim_start_matches('.');
    head.strip_prefix("@\"")
        .and_then(|rest| rest.find('"').map(|end| rest[..end].to_owned()))
        .unwrap_or_else(|| head.to_owned())
}

/// The URL-and-hash dependencies of a manifest, by name; path dependencies are skipped.
///
/// # Errors
/// Fails on a dependency without a URL or hash, or with a URL that is not HTTPS.
pub fn parse_dependencies(text: &str) -> Result<BTreeMap<String, Dependency>, Error> {
    let text = strip_comments(text);
    let mut deps = BTreeMap::new();
    let Some(marker) = text.find(".dependencies") else {
        return Ok(deps);
    };
    let Some(open) = text[marker..].find(".{") else {
        return Ok(deps);
    };
    let start = marker + open + 2;
    let block = &text[start..matching_brace(&text, start)];
    let mut at = 0;
    while let Some(entry) = block[at..].find("= .{") {
        let entry = at + entry;
        let name = dependency_name(block[at..entry].trim().trim_matches(',').trim());
        let body_start = entry + 4;
        let body_end = matching_brace(block, body_start);
        let body = &block[body_start..body_end];
        at = body_end + 1;
        if body.contains(".path") && !body.contains(".url") {
            continue;
        }
        let url = string_field(body, "url");
        let hash = string_field(body, "hash");
        if url.is_empty() {
            return Err(Error::new(format!("dependency {name} has no .url")));
        }
        if !url.starts_with("https://") {
            return Err(Error::new(format!(
                "dependency {name} uses {url}; only https archive URLs are supported"
            )));
        }
        if hash.is_empty() {
            return Err(Error::new(format!("dependency {name} has no .hash")));
        }
        deps.insert(name.clone(), Dependency { name, url, hash });
    }
    Ok(deps)
}

/// The resolver of the zig ecosystem.
pub struct Zig<'a> {
    manifest: String,
    download: &'a dyn Download,
}

/// What one downloaded archive holds.
struct Described {
    dependency: Dependency,
    sha256: String,
    strip_prefix: String,
    main: String,
    deps: Vec<String>,
}

impl<'a> Zig<'a> {
    /// A resolver that downloads with `download`.
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
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

    fn describe(
        &self,
        dependency: &Dependency,
        selected: &BTreeMap<String, Dependency>,
    ) -> Result<Described, Error> {
        let name = &dependency.name;
        let failed = |message: String| Error::new(format!("dependency {name}: {message}"));
        let url = &dependency.url;
        let kind = archive_kind(url).ok_or_else(|| {
            failed(format!(
                "{url} is not supported; supported archives are .tar.gz, .tgz and .zip"
            ))
        })?;
        let bytes = self
            .download
            .bytes(url)
            .map_err(|error| failed(format!("{url}: {error}")))?;
        let wanted = main_candidates(name)
            .into_iter()
            .chain([self.manifest.clone()])
            .collect::<Vec<_>>();
        let members = read_members(&bytes, kind, &wanted)
            .map_err(|error| failed(format!("{url}: {error}")))?;
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
        let main = main_candidates(name)
            .into_iter()
            .find(|candidate| members.names.contains(&path(candidate)))
            .ok_or_else(|| {
                failed(format!(
                    "no root source file; expected one of {}",
                    main_candidates(name).join(", ")
                ))
            })?;
        let deps = match members.files.get(&path(&self.manifest)) {
            Some(zon) => {
                nested_dependencies(&self.manifest, &String::from_utf8_lossy(zon), selected)
                    .map_err(failed)?
            }
            None => Vec::new(),
        };
        Ok(Described {
            dependency: dependency.clone(),
            sha256: dx_digest::sha256_hex(&bytes),
            strip_prefix,
            main,
            deps,
        })
    }
}

/// The most bytes read from one archive member.
const MEMBER_LIMIT: u64 = 8 << 20;

#[derive(Clone, Copy)]
enum Kind {
    TarGz,
    Zip,
}

fn archive_kind(url: &str) -> Option<Kind> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
        Some(Kind::TarGz)
    } else if path.ends_with(".zip") {
        Some(Kind::Zip)
    } else {
        None
    }
}

/// The regular files of an archive and the contents of those named `<top>/<wanted>` or `<wanted>`.
struct Members {
    names: BTreeSet<String>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Members {
    fn add(&mut self, name: &str, wanted: &[String], body: &mut dyn Read) -> Result<(), String> {
        let name = name.trim_start_matches("./").to_owned();
        let is_wanted = wanted.iter().any(|file| {
            name == *file || name.split_once('/').is_some_and(|(_, rest)| rest == file)
        });
        if is_wanted {
            let mut bytes = Vec::new();
            body.take(MEMBER_LIMIT)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("{name}: {error}"))?;
            self.files.insert(name.clone(), bytes);
        }
        self.names.insert(name);
        Ok(())
    }
}

fn read_members(bytes: &[u8], kind: Kind, wanted: &[String]) -> Result<Members, String> {
    let mut members = Members {
        names: BTreeSet::new(),
        files: BTreeMap::new(),
    };
    match kind {
        Kind::TarGz => {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
            for entry in archive.entries().map_err(|error| error.to_string())? {
                let mut entry = entry.map_err(|error| error.to_string())?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let name = String::from_utf8(entry.path_bytes().into_owned())
                    .map_err(|_| "a member name is not UTF-8".to_owned())?;
                members.add(&name, wanted, &mut entry)?;
            }
        }
        Kind::Zip => {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
                .map_err(|error| error.to_string())?;
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
                if !entry.is_file() || entry.is_symlink() {
                    continue;
                }
                let name = entry
                    .name()
                    .map_err(|error| error.to_string())?
                    .into_owned();
                members.add(&name, wanted, &mut entry)?;
            }
        }
    }
    Ok(members)
}

/// The names of the dependencies a package's manifest lists, each of which the root lists with the same hash.
fn nested_dependencies(
    manifest: &str,
    text: &str,
    selected: &BTreeMap<String, Dependency>,
) -> Result<Vec<String>, String> {
    let nested = parse_dependencies(text).map_err(|error| format!("{manifest}: {error}"))?;
    let mut names = Vec::new();
    for (name, dep) in nested {
        match selected.get(&name) {
            None => {
                return Err(format!(
                    "depends on {name}, which the root {manifest} does not list; add it there"
                ));
            }
            Some(root) if root.hash != dep.hash => {
                return Err(format!(
                    "depends on {name} with hash {:?} but the root {manifest} has {:?}",
                    dep.hash, root.hash
                ));
            }
            Some(_) => names.push(name),
        }
    }
    Ok(names)
}

/// The package of `old` that has the name, version and archive URL of `dependency`.
fn unchanged<'l>(old: &'l Lock, dependency: &Dependency) -> Option<&'l Package> {
    old.packages.iter().find(|package| {
        package.name == dependency.name
            && package.version == dependency.hash
            && package.archives.len() == 1
            && package.archives[0].urls == [dependency.url.as_str()]
            && package.extra.contains_key("zig_main")
    })
}

impl Resolver for Zig<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let text = std::fs::read_to_string(scratch.set().join(&self.manifest))
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let selected = parse_dependencies(&text)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let mut kept: BTreeMap<&str, &Package> = selected
            .values()
            .filter_map(|dependency| {
                Some((
                    dependency.name.as_str(),
                    old.and_then(|lock| unchanged(lock, dependency))?,
                ))
            })
            .collect();
        loop {
            let stale: Vec<&str> = kept
                .iter()
                .filter(|(_, package)| {
                    package
                        .deps
                        .iter()
                        .any(|dep| !kept.contains_key(dep.as_str()))
                })
                .map(|(name, _)| *name)
                .collect();
            if stale.is_empty() {
                break;
            }
            for name in stale {
                kept.remove(name);
            }
        }
        let mut packages = Vec::new();
        for dependency in selected.values() {
            if let Some(package) = kept.get(dependency.name.as_str()) {
                packages.push((*package).clone());
                continue;
            }
            let described = self.describe(dependency, &selected)?;
            let mut extra = BTreeMap::new();
            extra.insert("zig_main".to_owned(), json!(described.main));
            packages.push(Package {
                name: described.dependency.name.clone(),
                version: described.dependency.hash.clone(),
                archives: vec![Archive {
                    urls: vec![described.dependency.url.clone()],
                    sha256: described.sha256,
                    strip_prefix: described.strip_prefix,
                    platform: String::new(),
                }],
                deps: described.deps,
                provides: vec![described.dependency.name.clone()],
                dev: false,
                extra,
            });
        }
        Ok(Resolution { packages })
    }
}
