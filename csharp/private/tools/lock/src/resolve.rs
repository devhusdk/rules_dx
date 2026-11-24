#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::io::Cursor;

use base64::Engine as _;
use dx_lock::{
    Archive, Error, HttpFetch, Lock, Package, RESOLVER_TIMEOUT, Resolution, Resolver, Scratch, Spec,
};
use serde_json::Value;

pub const SOURCE: &str = "https://api.nuget.org/v3/index.json";

pub const FLAT: &str = "https://api.nuget.org/v3-flatcontainer";

pub const MAIN: &str = "main";

pub trait Download {
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

pub struct Registry(HttpFetch);

impl Registry {
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Pinned {
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
}

/// # Errors
/// Fails on a source other than [`SOURCE`] and on a group named twice.
pub fn manifest_groups(text: &str) -> Result<Vec<String>, Error> {
    let mut groups = vec![MAIN.to_owned()];
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match (words.next(), words.next()) {
            (Some("group"), Some(name)) => {
                if groups.iter().any(|group| group.eq_ignore_ascii_case(name)) {
                    return Err(Error::new(format!("group {name} is named twice")));
                }
                groups.push(name.to_owned());
            }
            (Some("source"), Some(url)) if url != SOURCE => {
                return Err(Error::new(format!("source {url} is not {SOURCE}")));
            }
            (Some(kind @ ("github" | "gist" | "http" | "git" | "clitool")), _) => {
                return Err(Error::new(format!(
                    "a `{kind}` dependency has no NuGet archive"
                )));
            }
            _ => {}
        }
    }
    Ok(groups)
}

#[must_use]
pub fn manifest_frameworks(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut frameworks: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut group = MAIN.to_owned();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("group ") {
            name.trim().clone_into(&mut group);
        } else if let Some(list) = line.strip_prefix("framework:") {
            let tfms = frameworks.entry(group.clone()).or_default();
            tfms.extend(
                list.split(',')
                    .map(|tfm| tfm.trim().trim_start_matches(">=").trim().to_owned())
                    .filter(|tfm| !tfm.is_empty()),
            );
            tfms.sort();
            tfms.dedup();
        }
    }
    frameworks
}

#[must_use]
pub fn normalize_version(version: &str) -> String {
    let (release, label) = match version.find(['-', '+']) {
        Some(at) => version.split_at(at),
        None => (version, ""),
    };
    let mut parts: Vec<String> = release
        .split('.')
        .map(|part| part.trim_start_matches('0'))
        .map(|part| if part.is_empty() { "0" } else { part }.to_owned())
        .collect();
    while parts.len() < 3 {
        parts.push("0".to_owned());
    }
    if parts.len() == 4 && parts.last().is_some_and(|part| part == "0") {
        parts.pop();
    }
    let label = label.split('+').next().unwrap_or_default();
    format!("{}{label}", parts.join("."))
}

/// # Errors
/// Fails on a section other than `NUGET`, a remote other than [`SOURCE`] and a line outside a package.
pub fn lock_groups(text: &str) -> Result<BTreeMap<String, Vec<Pinned>>, Error> {
    let mut groups: BTreeMap<String, Vec<Pinned>> = BTreeMap::new();
    let mut group = MAIN.to_owned();
    groups.entry(group.clone()).or_default();
    let mut in_nuget = false;
    for (number, line) in text.lines().enumerate() {
        let at = || format!("line {}", number + 1);
        let indent = line.len() - line.trim_start().len();
        let body = line.trim();
        if body.is_empty() {
            continue;
        }
        if indent == 0 {
            if let Some(name) = body.strip_prefix("GROUP ") {
                name.trim().clone_into(&mut group);
                groups.entry(group.clone()).or_default();
                in_nuget = false;
            } else if body == "NUGET" {
                in_nuget = true;
            } else if matches!(body, "GITHUB" | "HTTP" | "GIT" | "GIST") {
                return Err(Error::new(format!(
                    "{}: a {body} section has no NuGet archive",
                    at()
                )));
            } else {
                in_nuget = false;
            }
            continue;
        }
        if !in_nuget {
            continue;
        }
        if let Some(remote) = body.strip_prefix("remote:") {
            if remote.trim() != SOURCE {
                return Err(Error::new(format!(
                    "{}: remote {} is not {SOURCE}",
                    at(),
                    remote.trim()
                )));
            }
            continue;
        }
        let (name, version) = package_line(body)
            .ok_or_else(|| Error::new(format!("{}: {body:?} is no package", at())))?;
        let packages = groups.entry(group.clone()).or_default();
        match indent {
            4 => packages.push(Pinned {
                name: name.to_owned(),
                version: normalize_version(version),
                deps: Vec::new(),
            }),
            6 => packages
                .last_mut()
                .ok_or_else(|| Error::new(format!("{}: a dependency before its package", at())))?
                .deps
                .push(name.to_owned()),
            _ => return Err(Error::new(format!("{}: unexpected indent {indent}", at()))),
        }
    }
    Ok(groups)
}

fn package_line(body: &str) -> Option<(&str, &str)> {
    let (name, rest) = body.split_once(" (")?;
    let (version, _) = rest.split_once(')')?;
    Some((name.trim(), version.trim()))
}

#[must_use]
pub fn archive_url(name: &str, version: &str) -> String {
    let id = name.to_ascii_lowercase();
    let version = version.to_ascii_lowercase();
    format!("{FLAT}/{id}/{version}/{id}.{version}.nupkg")
}

/// # Errors
/// Fails when the bytes are not a zip archive.
pub fn assemblies(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    for path in members(bytes, |_| false)?.keys() {
        let Some(file) = path
            .strip_prefix("lib/")
            .and_then(|rest| rest.rsplit('/').next())
        else {
            continue;
        };
        if let Some(stem) = file.len().checked_sub(4).and_then(|at| {
            file.get(at..)
                .filter(|suffix| suffix.eq_ignore_ascii_case(".dll"))
                .and(file.get(..at))
        }) {
            names.push(stem.to_owned());
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn elements(text: &str, tag: &str) -> Vec<BTreeMap<String, String>> {
    let open = format!("<{tag}");
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(&open) {
        rest = rest.get(at + open.len()..).unwrap_or_default();
        if !rest.starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        let mut body = rest.get(..end).unwrap_or_default();
        let mut attributes = BTreeMap::new();
        while let Some((key, after)) = body.split_once('=') {
            let after = after.trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                break;
            };
            let value = after.get(1..).unwrap_or_default();
            let Some((value, tail)) = value.split_once(quote) else {
                break;
            };
            let key = key.split_whitespace().last().unwrap_or_default();
            attributes.insert(key.to_owned(), value.to_owned());
            body = tail;
        }
        found.push(attributes);
    }
    found
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tool {
    pub name: String,
    pub entrypoint: String,
    pub runner: String,
}

/// # Errors
/// Fails when the bytes are not a zip archive or a tool settings file is unreadable.
pub fn tools(bytes: &[u8]) -> Result<BTreeMap<String, Vec<Tool>>, Error> {
    let files = members(bytes, |name| {
        is_nuspec(name) || tool_settings(name).is_some()
    })?;
    let Some(nuspec) = files
        .iter()
        .find_map(|(name, text)| text.as_ref().filter(|_| is_nuspec(name)))
    else {
        return Ok(BTreeMap::new());
    };
    let is_tool = elements(nuspec, "packageType").iter().any(|attributes| {
        attributes
            .get("name")
            .is_some_and(|name| name == "DotnetTool")
    });
    let mut found = BTreeMap::new();
    if !is_tool {
        return Ok(found);
    }
    for (name, text) in &files {
        let (Some(tfm), Some(text)) = (tool_settings(name), text) else {
            continue;
        };
        if tfm == "any" {
            continue;
        }
        let commands = elements(text, "Command")
            .into_iter()
            .map(|attributes| {
                let field = |key: &str| {
                    attributes
                        .get(key)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("{name}: a Command without {key}")))
                };
                Ok(Tool {
                    name: field("Name")?,
                    entrypoint: field("EntryPoint")?,
                    runner: field("Runner")?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        found.insert(tfm.to_owned(), commands);
    }
    Ok(found)
}

const MEMBER_LIMIT: u64 = 8 << 20;

fn members(
    bytes: &[u8],
    read: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Option<String>>, Error> {
    let limits = dx_archive::Limits {
        max_entries: 1 << 16,
        max_entry_bytes: MEMBER_LIMIT,
        max_total_bytes: 64 << 20,
    };
    let mut found = BTreeMap::new();
    dx_archive::visit(
        Cursor::new(bytes),
        dx_archive::Format::Zip,
        limits,
        |name, kind, body| {
            if kind != dx_archive::EntryKind::File {
                return Ok(());
            }
            let text = if read(name) {
                let mut text = String::new();
                body.read_to_string(&mut text)
                    .map_err(|error| std::io::Error::other(format!("{name}: {error}")))?;
                Some(text)
            } else {
                None
            };
            found.insert(name.to_owned(), text);
            Ok(())
        },
    )
    .map_err(|error| Error::new(error.to_string()))?;
    Ok(found)
}

fn is_nuspec(name: &str) -> bool {
    !name.contains('/') && name.to_ascii_lowercase().ends_with(".nuspec")
}

fn tool_settings(name: &str) -> Option<&str> {
    let mut parts = name.split('/');
    match (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) {
        (Some("tools"), Some(tfm), Some(_), Some("DotnetToolSettings.xml"), None) => Some(tfm),
        _ => None,
    }
}

#[must_use]
pub fn sha512(bytes: &[u8]) -> String {
    format!(
        "sha512-{}",
        base64::engine::general_purpose::STANDARD.encode(dx_digest::sha512(bytes))
    )
}

#[derive(Clone, Debug)]
pub struct Paket {
    pub dotnet: std::path::PathBuf,
    pub nupkg: std::path::PathBuf,
}

impl Paket {
    /// # Errors
    /// Fails when the package cannot be unpacked or Paket fails.
    pub fn install(&self, scratch: &Scratch) -> Result<(), Error> {
        let failed = |error: &dyn std::fmt::Display| Error::new(format!("paket: {error}"));
        let root = scratch.tmp().join("paket");
        dx_archive::extract(
            &self.nupkg,
            dx_archive::Format::Zip,
            &root,
            dx_archive::Limits::default(),
        )
        .map_err(|error| failed(&error))?;
        let dll = std::fs::read_dir(root.join("tools"))
            .map_err(|error| failed(&error))?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("any").join("paket.dll"))
            .find(|path| path.is_file())
            .ok_or_else(|| failed(&"the package has no paket.dll"))?;
        let mut command = scratch.command(&self.dotnet);
        command
            .env("DOTNET_ROLL_FORWARD", "Major")
            .env("DOTNET_NOLOGO", "1")
            .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
            .env("DOTNET_GENERATE_ASPNET_CERTIFICATE", "false")
            .env("DOTNET_SYSTEM_GLOBALIZATION_INVARIANT", "1")
            .arg(&dll)
            .arg("install");
        Scratch::run(&mut command, RESOLVER_TIMEOUT)
            .map(drop)
            .map_err(|error| failed(&error))
    }
}

pub struct Nuget<'a> {
    manifest: String,
    state: String,
    download: &'a dyn Download,
    paket: Option<Paket>,
}

impl<'a> Nuget<'a> {
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            state: spec.state.first().cloned().unwrap_or_default(),
            download,
            paket: None,
        }
    }

    #[must_use]
    pub fn with_paket(mut self, paket: Paket) -> Self {
        self.paket = Some(paket);
        self
    }

    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }

    fn package(
        &self,
        pinned: &Pinned,
        names: &[&str],
        frameworks: &[String],
        old: Option<&[Package]>,
    ) -> Result<Package, Error> {
        let frameworks = Value::from(frameworks.to_vec());
        let url = archive_url(&pinned.name, &pinned.version);
        let deps: Vec<String> = pinned
            .deps
            .iter()
            .filter_map(|dep| names.iter().find(|name| name.eq_ignore_ascii_case(dep)))
            .map(|name| (*name).to_owned())
            .collect();
        let previous = old.and_then(|packages| {
            packages.iter().find(|package| {
                package.name == pinned.name
                    && package.version == pinned.version
                    && package
                        .archives
                        .iter()
                        .any(|archive| archive.urls == [url.clone()])
                    && package.extra.contains_key("nuget_sha512")
            })
        });
        if let Some(previous) = previous {
            let mut extra = previous.extra.clone();
            extra.insert("nuget_frameworks".to_owned(), frameworks);
            return Ok(Package {
                deps,
                extra,
                ..previous.clone()
            });
        }
        let bytes = self.download.bytes(&url).map_err(|error| {
            Error::new(format!(
                "package {} {}: {error}",
                pinned.name, pinned.version
            ))
        })?;
        let provides = assemblies(&bytes).map_err(|error| Error::new(format!("{url}: {error}")))?;
        let commands = tools(&bytes).map_err(|error| Error::new(format!("{url}: {error}")))?;
        let mut extra = BTreeMap::from([
            ("nuget_frameworks".to_owned(), frameworks),
            ("nuget_sha512".to_owned(), Value::String(sha512(&bytes))),
        ]);
        if !commands.is_empty() {
            let commands = commands
                .into_iter()
                .map(|(tfm, commands)| {
                    let commands = commands
                        .into_iter()
                        .map(|tool| {
                            serde_json::json!({
                                "entrypoint": tool.entrypoint,
                                "name": tool.name,
                                "runner": tool.runner,
                            })
                        })
                        .collect::<Vec<_>>();
                    (tfm, Value::from(commands))
                })
                .collect::<serde_json::Map<_, _>>();
            extra.insert("nuget_tools".to_owned(), Value::Object(commands));
        }
        Ok(Package {
            name: pinned.name.clone(),
            version: pinned.version.clone(),
            archives: vec![Archive {
                urls: vec![url],
                sha256: dx_digest::sha256_hex(&bytes),
                kind: "zip".to_owned(),
                ..Archive::default()
            }],
            deps,
            provides,
            extra,
            ..Package::default()
        })
    }
}

impl Resolver for Nuget<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let read = |name: &str| {
            std::fs::read_to_string(scratch.set().join(name))
                .map_err(|error| Error::new(format!("{name}: {error}")))
        };
        if let Some(paket) = &self.paket {
            paket.install(scratch)?;
        }
        let manifest = read(&self.manifest)?;
        let wanted = manifest_groups(&manifest)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let frameworks = manifest_frameworks(&manifest);
        let locked = lock_groups(&read(&self.state)?)
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        let mut groups = BTreeMap::new();
        for group in wanted {
            let pinned = locked
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&group))
                .map(|(_, pinned)| pinned)
                .ok_or_else(|| Error::new(format!("{} has no group {group}", self.state)))?;
            if pinned.is_empty() {
                continue;
            }
            let tfms = frameworks
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&group))
                .map(|(_, tfms)| tfms.as_slice())
                .filter(|tfms| !tfms.is_empty())
                .ok_or_else(|| {
                    Error::new(format!(
                        "{}: group {group} has no framework restriction",
                        self.manifest
                    ))
                })?;
            let names: Vec<&str> = pinned.iter().map(|package| package.name.as_str()).collect();
            let previous = old
                .and_then(|lock| lock.groups.get(&group))
                .map(Vec::as_slice);
            let packages = pinned
                .iter()
                .map(|package| self.package(package, &names, tfms, previous))
                .collect::<Result<Vec<_>, _>>()?;
            groups.insert(group, packages);
        }
        Ok(Resolution {
            packages: Vec::new(),
            groups,
        })
    }
}

pub fn run_process(args: &[String]) -> i32 {
    let (paths, rest) = match dx_lock::tool_paths(args, &["--dotnet", "--paket"]) {
        Ok(split) => split,
        Err(error) => {
            eprintln!("csharp_lock: {error}");
            return 2;
        }
    };
    let mut paths = paths.into_iter();
    let (Some(dotnet), Some(nupkg)) = (paths.next(), paths.next()) else {
        return 2;
    };
    let Some(hub) = dx_families::hub("nuget") else {
        return dx_lock::missing_hub("nuget");
    };
    let spec = Nuget::spec(hub);
    let registry = Registry::new();
    let nuget = Nuget::new(&spec, &registry).with_paket(Paket { dotnet, nupkg });
    dx_lock::run_process(&spec, &nuget, &rest)
}
