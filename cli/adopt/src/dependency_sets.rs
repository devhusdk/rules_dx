use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const DX_TOML_REL: &str = "dx.toml";

pub const SCHEMA_VERSION: u32 = 1;

pub const ECOSYSTEM_UV: &str = "uv";

pub const ECOSYSTEM_NPM: &str = "npm";

pub const ECOSYSTEM_BAZEL: &str = "bazel";

pub const ECOSYSTEM_RUBY: &str = "ruby";

pub const ECOSYSTEM_POWERSHELL: &str = "powershell";

pub const SUPPORTED_ECOSYSTEMS: &str = "uv, npm, bazel, ruby, powershell";

pub const GEMFILE_LOCK: &str = "Gemfile.lock";

pub const PSGALLERY_LOCK: &str = "PSGallery.lock.json";

pub const PNPM_LOCK: &str = "pnpm-lock.yaml";

pub const PNPM_WORKSPACE: &str = "pnpm-workspace.yaml";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Ecosystem {
    Uv,
    Npm,
    Bazel,
    Ruby,
    PowerShell,
}

impl Ecosystem {
    pub fn name(self) -> &'static str {
        match self {
            Ecosystem::Uv => ECOSYSTEM_UV,
            Ecosystem::Npm => ECOSYSTEM_NPM,
            Ecosystem::Bazel => ECOSYSTEM_BAZEL,
            Ecosystem::Ruby => ECOSYSTEM_RUBY,
            Ecosystem::PowerShell => ECOSYSTEM_POWERSHELL,
        }
    }

    pub fn parse(text: &str) -> Option<Ecosystem> {
        match text {
            ECOSYSTEM_UV => Some(Ecosystem::Uv),
            ECOSYSTEM_NPM => Some(Ecosystem::Npm),
            ECOSYSTEM_BAZEL => Some(Ecosystem::Bazel),
            ECOSYSTEM_RUBY => Some(Ecosystem::Ruby),
            ECOSYSTEM_POWERSHELL => Some(Ecosystem::PowerShell),
            _ => None,
        }
    }

    pub fn supported_check(self) -> &'static str {
        match self {
            Ecosystem::Uv | Ecosystem::Npm => "check",
            Ecosystem::Bazel | Ecosystem::Ruby | Ecosystem::PowerShell => "pinned",
        }
    }

    pub fn supported_update(self) -> &'static str {
        match self {
            Ecosystem::Uv | Ecosystem::Npm => "update",
            Ecosystem::Bazel | Ecosystem::Ruby | Ecosystem::PowerShell => "pinned",
        }
    }

    /// Lists what dx does for this ecosystem, folding pinned check and update into one clause.
    pub fn supported_operations(self) -> String {
        let (check, update, audit) = (
            self.supported_check(),
            self.supported_update(),
            self.supported_audit(),
        );
        if check == "pinned" && update == "pinned" {
            format!("{audit}; check and update report pinned")
        } else {
            format!("{check}, {update}, {audit}")
        }
    }

    pub fn supported_audit(self) -> &'static str {
        match self {
            Ecosystem::Uv
            | Ecosystem::Npm
            | Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell => "audit",
        }
    }

    /// Names the advisory snapshot family that covers this ecosystem's locks.
    pub fn advisory_family(self) -> Option<&'static str> {
        match self {
            Ecosystem::Uv => Some("pypi"),
            Ecosystem::Npm => Some("npm"),
            Ecosystem::Ruby => Some("rubygems"),
            Ecosystem::Bazel | Ecosystem::PowerShell => None,
        }
    }

    /// Names the file every manifest of this ecosystem must have.
    pub fn manifest_name(self) -> &'static str {
        match self {
            Ecosystem::Uv => "pyproject.toml",
            Ecosystem::Npm => "package.json",
            Ecosystem::Bazel => "MODULE.bazel",
            Ecosystem::Ruby => "Gemfile",
            Ecosystem::PowerShell => "PSGallery.requirements.psd1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySet {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub manifests: Vec<String>,
    pub locks: Vec<String>,
    pub scopes: Vec<String>,
    pub writable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registry {
    pub sets: Vec<DependencySet>,
}

impl Registry {
    pub fn find(&self, name: &str) -> Option<&DependencySet> {
        self.sets.iter().find(|set| set.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.sets.iter().map(|set| set.name.as_str()).collect()
    }

    pub fn name_list(&self) -> String {
        self.names().join(", ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selection {
    Full,
    Packages(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSet {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub dir: String,
    pub manifests: Vec<String>,
    pub locks: Vec<String>,
    pub writable: bool,
}

impl ResolvedSet {
    pub fn of(set: &DependencySet) -> ResolvedSet {
        ResolvedSet {
            name: set.name.clone(),
            ecosystem: set.ecosystem,
            dir: manifest_dir(&set.manifests),
            manifests: set.manifests.clone(),
            locks: set.locks.clone(),
            writable: set.writable,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DependencySetsError {
    #[error("could not read {rel}: {detail}")]
    Read { rel: String, detail: String },
    #[error("could not read {rel}: not valid UTF-8")]
    NotUtf8 { rel: String },
    #[error("invalid {rel}: {detail}")]
    InvalidToml { rel: String, detail: String },
    #[error("invalid {rel}: missing schema_version (want {SCHEMA_VERSION})")]
    MissingSchema { rel: String },
    #[error("invalid {rel}: unsupported schema_version {version} (want {SCHEMA_VERSION})")]
    UnsupportedSchema { rel: String, version: u32 },
    #[error("invalid {rel}: declares no dependency_set records")]
    NoSets { rel: String },
    #[error("invalid {rel}: dependency set {index} is missing name")]
    MissingName { rel: String, index: usize },
    #[error("invalid {rel}: dependency set name {name:?} is empty or has surrounding whitespace")]
    EmptyName { rel: String, name: String },
    #[error(
        "invalid {rel}: dependency set name {name:?} must start alphanumerically and use [A-Za-z0-9_.-] only"
    )]
    InvalidName { rel: String, name: String },
    #[error("invalid {rel}: dependency set {name:?} is missing ecosystem")]
    MissingEcosystem { rel: String, name: String },
    #[error(
        "invalid {rel}: dependency set {name:?} has unknown ecosystem {kind:?} (want {SUPPORTED_ECOSYSTEMS})"
    )]
    UnknownEcosystem {
        rel: String,
        name: String,
        kind: String,
    },
    #[error("invalid {rel}: dependency set {name:?} owns no manifests")]
    MissingManifests { rel: String, name: String },
    #[error("invalid {rel}: dependency set {name:?} owns no locks")]
    MissingLocks { rel: String, name: String },
    #[error("invalid {rel}: dependency set {name:?} owns no scopes")]
    MissingScopes { rel: String, name: String },
    #[error(
        "invalid {rel}: dependency set {name:?} owns path {path:?} which escapes the workspace (workspace-relative paths only, no absolute paths or '..')"
    )]
    EscapingPath {
        rel: String,
        name: String,
        path: String,
    },
    #[error("invalid {rel}: dependency set {name:?} manifest {path:?} must be named {want} for ecosystem {ecosystem}")]
    InvalidManifest {
        rel: String,
        name: String,
        path: String,
        want: &'static str,
        ecosystem: &'static str,
    },
    #[error("invalid {rel}: dependency set {name:?} owns empty path")]
    EmptyPath { rel: String, name: String },
    #[error(
        "invalid {rel}: dependency set {name:?} spreads manifests across directories (keep one directory per set)"
    )]
    ScatteredManifests { rel: String, name: String },
    #[error("invalid {rel}: duplicate dependency set name {name:?}")]
    DuplicateName { rel: String, name: String },
    #[error(
        "invalid {rel}: {path:?} is owned writable by dependency sets {names} (mark all but one writable = false for shared read-only ownership)"
    )]
    ConflictingOwnership {
        rel: String,
        path: String,
        names: String,
    },
    #[error("unknown dependency set or scope {selector:?}; expected {names}, set:package, or a workspace path")]
    UnknownSelector { selector: String, names: String },
    #[error("invalid package {package:?} for set {set}: {reason}")]
    InvalidPackage {
        set: String,
        package: String,
        reason: &'static str,
    },
    #[error("no owning dependency set for {scope:?} (non-dependency paths are out of dependency-set scope; sets: {names})")]
    NoOwningSet { scope: String, names: String },
    #[error("dependency set {name:?} is read-only (writable = false); refusing to update")]
    ReadOnlyUpdate { name: String },
    #[error("dependency set {name:?} is missing {path} (declared in {rel})")]
    MissingFile {
        name: String,
        path: String,
        rel: String,
    },
    #[error("dependency set {name:?} relies on uv settings dx does not apply: {detail}; keep resolver settings out of uv.toml and [tool.uv]")]
    UnsupportedSettings { name: String, detail: String },
    #[error("dependency set {name:?} cannot run pnpm in {dir}: {detail}")]
    UnsupportedNpmLayout {
        name: String,
        dir: String,
        detail: String,
    },
    #[error("dependency set {name:?} must declare {path} in locks; dx reads the {ecosystem} lock beside its manifest")]
    MissingPinnedLock {
        name: String,
        path: String,
        ecosystem: &'static str,
    },
}

const UV_PROJECT_KEYS: &[&str] = &[
    "conflicts",
    "constraint-dependencies",
    "default-groups",
    "dev-dependencies",
    "managed",
    "override-dependencies",
    "package",
    "sources",
    "workspace",
];

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DxToml {
    schema_version: Option<u32>,
    #[serde(default)]
    dependency_set: Vec<RawSet>,
    #[serde(default)]
    dx: Option<toml::Value>,
    #[serde(default)]
    quality: Option<QualityTable>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct QualityTable {
    #[serde(default)]
    #[allow(dead_code)]
    baseline: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSet {
    name: Option<String>,
    ecosystem: Option<String>,
    #[serde(default)]
    manifests: Vec<String>,
    #[serde(default)]
    locks: Vec<String>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default = "default_writable")]
    writable: bool,
}

fn default_writable() -> bool {
    true
}

pub fn load(workspace: &Path) -> Result<Option<Registry>, DependencySetsError> {
    let rel = DX_TOML_REL.to_owned();
    let bytes = match std::fs::read(workspace.join(DX_TOML_REL)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(DependencySetsError::Read {
                rel,
                detail: error.to_string(),
            });
        }
        Ok(bytes) => bytes,
    };
    let text =
        String::from_utf8(bytes).map_err(|_| DependencySetsError::NotUtf8 { rel: rel.clone() })?;
    parse(&text, &rel)
}

pub fn parse(text: &str, rel: &str) -> Result<Option<Registry>, DependencySetsError> {
    let document: DxToml =
        toml::from_str(text).map_err(|error| DependencySetsError::InvalidToml {
            rel: rel.to_owned(),
            detail: error.to_string(),
        })?;
    let has_defaults = document.dx.is_some() || document.quality.is_some();
    match document.schema_version {
        None if document.dependency_set.is_empty() && has_defaults => return Ok(None),
        None => {
            return Err(DependencySetsError::MissingSchema {
                rel: rel.to_owned(),
            });
        }
        Some(version) if version != SCHEMA_VERSION => {
            return Err(DependencySetsError::UnsupportedSchema {
                rel: rel.to_owned(),
                version,
            });
        }
        Some(_) => {}
    }
    if document.dependency_set.is_empty() {
        if has_defaults {
            return Ok(None);
        }
        return Err(DependencySetsError::NoSets {
            rel: rel.to_owned(),
        });
    }
    let mut sets = Vec::with_capacity(document.dependency_set.len());
    for (index, raw) in document.dependency_set.iter().enumerate() {
        sets.push(validate_set(raw, index, rel)?);
    }
    check_conflicts(&sets, rel)?;
    sets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(Registry { sets }))
}

fn validate_set(
    raw: &RawSet,
    index: usize,
    rel: &str,
) -> Result<DependencySet, DependencySetsError> {
    let name = match &raw.name {
        None => {
            return Err(DependencySetsError::MissingName {
                rel: rel.to_owned(),
                index,
            });
        }
        Some(name) => name.clone(),
    };
    if name.is_empty() || name.trim() != name {
        return Err(DependencySetsError::EmptyName {
            rel: rel.to_owned(),
            name,
        });
    }
    if !valid_name(&name) {
        return Err(DependencySetsError::InvalidName {
            rel: rel.to_owned(),
            name,
        });
    }
    let ecosystem = match &raw.ecosystem {
        None => {
            return Err(DependencySetsError::MissingEcosystem {
                rel: rel.to_owned(),
                name,
            });
        }
        Some(kind) => match Ecosystem::parse(kind) {
            None => {
                return Err(DependencySetsError::UnknownEcosystem {
                    rel: rel.to_owned(),
                    name,
                    kind: kind.clone(),
                });
            }
            Some(ecosystem) => ecosystem,
        },
    };
    if raw.manifests.is_empty() {
        return Err(DependencySetsError::MissingManifests {
            rel: rel.to_owned(),
            name,
        });
    }
    if raw.locks.is_empty() {
        return Err(DependencySetsError::MissingLocks {
            rel: rel.to_owned(),
            name,
        });
    }
    if raw.scopes.is_empty() {
        return Err(DependencySetsError::MissingScopes {
            rel: rel.to_owned(),
            name,
        });
    }
    for path in raw
        .manifests
        .iter()
        .chain(raw.locks.iter())
        .chain(raw.scopes.iter())
    {
        if path.is_empty() {
            return Err(DependencySetsError::EmptyPath {
                rel: rel.to_owned(),
                name: name.clone(),
            });
        }
        if escapes_workspace(path) {
            return Err(DependencySetsError::EscapingPath {
                rel: rel.to_owned(),
                name: name.clone(),
                path: path.clone(),
            });
        }
    }
    for manifest in &raw.manifests {
        let file = manifest.rsplit('/').next().unwrap_or(manifest);
        if file != ecosystem.manifest_name() {
            return Err(DependencySetsError::InvalidManifest {
                rel: rel.to_owned(),
                name,
                path: manifest.clone(),
                want: ecosystem.manifest_name(),
                ecosystem: ecosystem.name(),
            });
        }
    }
    let mut dirs = BTreeSet::new();
    for manifest in &raw.manifests {
        dirs.insert(manifest_dir(std::slice::from_ref(manifest)));
    }
    if dirs.len() != 1 {
        return Err(DependencySetsError::ScatteredManifests {
            rel: rel.to_owned(),
            name,
        });
    }
    Ok(DependencySet {
        name,
        ecosystem,
        manifests: raw.manifests.clone(),
        locks: raw.locks.clone(),
        scopes: raw.scopes.clone(),
        writable: raw.writable,
    })
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    for char in chars {
        if !(char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-') {
            return false;
        }
    }
    true
}

fn escapes_workspace(path: &str) -> bool {
    if path.contains('\\') || path.starts_with('/') || has_drive_prefix(path) {
        return true;
    }
    if Path::new(path).components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return true;
    }
    let mut depth = 0i32;
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => depth += 1,
        }
    }
    false
}

fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn manifest_dir(manifests: &[String]) -> String {
    let first = manifests.first().map(String::as_str).unwrap_or("");
    match first.rfind('/') {
        Some(index) => first[..index].to_owned(),
        None => ".".to_owned(),
    }
}

fn check_conflicts(sets: &[DependencySet], rel: &str) -> Result<(), DependencySetsError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for set in sets {
        if !seen.insert(set.name.as_str()) {
            return Err(DependencySetsError::DuplicateName {
                rel: rel.to_owned(),
                name: set.name.clone(),
            });
        }
    }
    let mut writers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for set in sets {
        if !set.writable {
            continue;
        }
        for path in set.manifests.iter().chain(set.locks.iter()) {
            writers
                .entry(path.as_str())
                .or_default()
                .push(set.name.as_str());
        }
    }
    for (path, mut names) in writers {
        names.sort();
        names.dedup();
        if names.len() > 1 {
            return Err(DependencySetsError::ConflictingOwnership {
                rel: rel.to_owned(),
                path: path.to_owned(),
                names: names.join(", "),
            });
        }
    }
    Ok(())
}

const BAZEL_PACKAGE_CHARSET: &str = "bazel module names use [A-Za-z0-9_.-] only";

const UV_PACKAGE_CHARSET: &str = "uv package names use [A-Za-z0-9_.-] only";

const RUBY_PACKAGE_CHARSET: &str = "gem names use [A-Za-z0-9_.-] only";

const POWERSHELL_PACKAGE_CHARSET: &str = "PowerShell module names use [A-Za-z0-9_.-] only";

const NPM_PACKAGE_CHARSET: &str =
    "npm package names are [@scope/]name with lowercase [a-z0-9._-] parts";

fn npm_part(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with('.')
        && !part.starts_with('_')
        && part.chars().all(|char| {
            char.is_ascii_lowercase()
                || char.is_ascii_digit()
                || char == '.'
                || char == '_'
                || char == '-'
        })
}

fn validate_package(ecosystem: Ecosystem, package: &str) -> Result<(), &'static str> {
    match ecosystem {
        Ecosystem::Npm => {
            let valid = match package.strip_prefix('@') {
                Some(scoped) => scoped
                    .split_once('/')
                    .is_some_and(|(scope, name)| npm_part(scope) && npm_part(name)),
                None => npm_part(package),
            };
            if valid {
                Ok(())
            } else {
                Err(NPM_PACKAGE_CHARSET)
            }
        }
        Ecosystem::Uv | Ecosystem::Bazel | Ecosystem::Ruby | Ecosystem::PowerShell => {
            let charset = match ecosystem {
                Ecosystem::Bazel => BAZEL_PACKAGE_CHARSET,
                Ecosystem::Ruby => RUBY_PACKAGE_CHARSET,
                Ecosystem::PowerShell => POWERSHELL_PACKAGE_CHARSET,
                _ => UV_PACKAGE_CHARSET,
            };
            if package.is_empty() {
                return Err(charset);
            }
            for char in package.chars() {
                if !(char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-') {
                    return Err(charset);
                }
            }
            Ok(())
        }
    }
}

pub fn resolve(
    registry: &Registry,
    selectors: &[String],
) -> Result<Vec<ResolvedTarget>, DependencySetsError> {
    if selectors.is_empty() {
        return Ok(registry
            .sets
            .iter()
            .map(|set| ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Full,
            })
            .collect());
    }
    let mut full: BTreeSet<&str> = BTreeSet::new();
    let mut packages: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for raw in selectors {
        match parse_selector(registry, raw)? {
            ParsedSelector::Set(name) => {
                full.insert(name);
            }
            ParsedSelector::Package(name, package) => {
                packages.entry(name).or_default().push(package);
            }
            ParsedSelector::Scope(owners) => {
                for name in owners {
                    full.insert(name);
                }
            }
        }
    }
    let mut resolved = Vec::new();
    for set in &registry.sets {
        if full.contains(set.name.as_str()) {
            resolved.push(ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Full,
            });
        } else if let Some(mut names) = packages.remove(set.name.as_str()) {
            names.sort();
            names.dedup();
            resolved.push(ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Packages(names),
            });
        }
    }
    Ok(resolved)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTarget {
    pub set: ResolvedSet,
    pub request: Selection,
}

enum ParsedSelector<'a> {
    Set(&'a str),
    Package(&'a str, String),
    Scope(Vec<&'a str>),
}

fn parse_selector<'a>(
    registry: &'a Registry,
    text: &str,
) -> Result<ParsedSelector<'a>, DependencySetsError> {
    let names = registry.name_list();
    if text.contains(':') {
        if let Some((head, tail)) = text.split_once(':') {
            if let Some(set) = registry.find(head) {
                if tail.is_empty() {
                    return Err(DependencySetsError::InvalidPackage {
                        set: set.name.clone(),
                        package: tail.to_owned(),
                        reason: "package identity is empty",
                    });
                }
                if let Err(reason) = validate_package(set.ecosystem, tail) {
                    return Err(DependencySetsError::InvalidPackage {
                        set: set.name.clone(),
                        package: tail.to_owned(),
                        reason,
                    });
                }
                return Ok(ParsedSelector::Package(&set.name, tail.to_owned()));
            }
        }
        if text.starts_with("//") || text.starts_with('@') {
            return scope_selector(registry, text, &names);
        }
        return Err(DependencySetsError::UnknownSelector {
            selector: text.to_owned(),
            names,
        });
    }
    if let Some(set) = registry.find(text) {
        return Ok(ParsedSelector::Set(&set.name));
    }
    if is_scope_shape(text) {
        return scope_selector(registry, text, &names);
    }
    Err(DependencySetsError::UnknownSelector {
        selector: text.to_owned(),
        names,
    })
}

fn is_scope_shape(text: &str) -> bool {
    text == "..."
        || text == "//..."
        || text.starts_with("//")
        || text.starts_with("./")
        || text.starts_with('/')
        || text.contains('/')
        || text.ends_with("/...")
}

fn scope_selector<'a>(
    registry: &'a Registry,
    text: &str,
    names: &str,
) -> Result<ParsedSelector<'a>, DependencySetsError> {
    let owners = owning_sets(registry, text);
    if owners.is_empty() {
        return Err(DependencySetsError::NoOwningSet {
            scope: text.to_owned(),
            names: names.to_owned(),
        });
    }
    Ok(ParsedSelector::Scope(owners))
}

fn normalize_scope(text: &str) -> (String, bool) {
    if text == "..." || text == "//..." {
        return (String::new(), true);
    }
    let mut path = text.to_owned();
    let mut recursive = false;
    if let Some(prefix) = path.strip_suffix("/...") {
        path = prefix.to_owned();
        recursive = true;
    }
    let label = path.starts_with("//") || path.starts_with('@');
    if let Some(rest) = path.strip_prefix("//") {
        path = rest.to_owned();
    }
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.to_owned();
    }
    while path.ends_with('/') && path.len() > 1 {
        path.pop();
    }
    if label {
        if let Some(colon) = path.find(':') {
            path = path[..colon].to_owned();
        }
    }
    (path, recursive)
}

fn scope_covers(scope: &str, package: &str) -> bool {
    match package.strip_prefix(scope) {
        Some("") => true,
        Some(rest) => rest.starts_with('/'),
        None => false,
    }
}

pub fn owning_sets<'a>(registry: &'a Registry, target: &str) -> Vec<&'a str> {
    if target.starts_with('@') {
        return Vec::new();
    }
    let (package, recursive) = normalize_scope(target);
    if package.is_empty() {
        if recursive || target.contains("MODULE.bazel") {
            return registry.sets.iter().map(|set| set.name.as_str()).collect();
        }
        return Vec::new();
    }
    let mut owners: BTreeSet<&str> = BTreeSet::new();
    for set in &registry.sets {
        for scope in &set.scopes {
            if scope_covers(scope, &package) {
                owners.insert(set.name.as_str());
                break;
            }
        }
    }
    if recursive {
        for set in &registry.sets {
            for scope in &set.scopes {
                if scope_covers(&package, scope) {
                    owners.insert(set.name.as_str());
                    break;
                }
            }
        }
    }
    let mut ordered: Vec<&str> = registry
        .sets
        .iter()
        .map(|set| set.name.as_str())
        .filter(|name| owners.contains(name))
        .collect();
    ordered.sort();
    ordered
}

pub fn check_files(
    workspace: &Path,
    set: &ResolvedSet,
    need_lock: bool,
) -> Result<(), DependencySetsError> {
    for path in set.manifests.iter().chain(if need_lock {
        set.locks.iter()
    } else {
        [].iter()
    }) {
        if !workspace.join(path).is_file() {
            return Err(DependencySetsError::MissingFile {
                name: set.name.clone(),
                path: path.clone(),
                rel: DX_TOML_REL.to_owned(),
            });
        }
    }
    Ok(())
}

fn uv_workspace_root(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .and_then(|table| table.get("tool")?.get("uv")?.get("workspace").map(|_| ()))
        .is_some()
}

fn parent_dirs(dir: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut current = Some(dir);
    while let Some(here) = current {
        out.push(here);
        current = match here.rfind('/') {
            Some(index) => Some(&here[..index]),
            None if here != "." => Some("."),
            None => None,
        };
    }
    out
}

fn in_dir(dir: &str, file: &str) -> String {
    match dir {
        "." => file.to_owned(),
        _ => format!("{dir}/{file}"),
    }
}

/// Refuses a set whose backend would read settings or write files outside the set.
pub fn check_backend_settings(
    workspace: &Path,
    set: &ResolvedSet,
) -> Result<(), DependencySetsError> {
    match set.ecosystem {
        Ecosystem::Uv => check_uv_settings(workspace, set),
        Ecosystem::Npm => check_npm_layout(workspace, set),
        Ecosystem::Bazel => Ok(()),
        Ecosystem::Ruby => check_pinned_lock(set, GEMFILE_LOCK),
        Ecosystem::PowerShell => check_pinned_lock(set, PSGALLERY_LOCK),
    }
}

fn check_pinned_lock(set: &ResolvedSet, file: &str) -> Result<(), DependencySetsError> {
    let lock = in_dir(&set.dir, file);
    if set.locks.contains(&lock) {
        return Ok(());
    }
    Err(DependencySetsError::MissingPinnedLock {
        name: set.name.clone(),
        path: lock,
        ecosystem: set.ecosystem.name(),
    })
}

fn check_npm_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let lock = in_dir(&set.dir, PNPM_LOCK);
    let unsupported = |detail: String| DependencySetsError::UnsupportedNpmLayout {
        name: set.name.clone(),
        dir: set.dir.clone(),
        detail,
    };
    if !set.locks.contains(&lock) {
        return Err(unsupported(format!(
            "locks must include {lock}, the lock pnpm writes"
        )));
    }
    for dir in parent_dirs(&set.dir).into_iter().skip(1) {
        let config = in_dir(dir, PNPM_WORKSPACE);
        if workspace.join(&config).is_file() {
            return Err(unsupported(format!(
                "{config} makes pnpm write the parent workspace lock"
            )));
        }
    }
    Ok(())
}

fn check_uv_settings(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let mut dir = Some(set.dir.as_str());
    while let Some(current) = dir {
        let prefix = match current {
            "." => String::new(),
            _ => format!("{current}/"),
        };
        let config = format!("{prefix}uv.toml");
        if workspace.join(&config).is_file() {
            return Err(DependencySetsError::UnsupportedSettings {
                name: set.name.clone(),
                detail: format!("{config} exists"),
            });
        }
        if current != set.dir {
            let parent = format!("{prefix}pyproject.toml");
            if uv_workspace_root(&workspace.join(&parent)) {
                return Err(DependencySetsError::UnsupportedSettings {
                    name: set.name.clone(),
                    detail: format!("{parent} sets [tool.uv.workspace]"),
                });
            }
        }
        dir = match current.rfind('/') {
            Some(index) => Some(&current[..index]),
            None if current != "." => Some("."),
            None => None,
        };
    }
    for manifest in &set.manifests {
        let bytes =
            std::fs::read(workspace.join(manifest)).map_err(|error| DependencySetsError::Read {
                rel: manifest.clone(),
                detail: error.to_string(),
            })?;
        let text = String::from_utf8(bytes).map_err(|_| DependencySetsError::NotUtf8 {
            rel: manifest.clone(),
        })?;
        let table: toml::Table =
            toml::from_str(&text).map_err(|error| DependencySetsError::InvalidToml {
                rel: manifest.clone(),
                detail: error.to_string(),
            })?;
        let unsupported: Vec<&str> = table
            .get("tool")
            .and_then(|tool| tool.get("uv"))
            .and_then(toml::Value::as_table)
            .map(|uv| {
                uv.keys()
                    .map(String::as_str)
                    .filter(|key| !UV_PROJECT_KEYS.contains(key))
                    .collect()
            })
            .unwrap_or_default();
        if !unsupported.is_empty() {
            return Err(DependencySetsError::UnsupportedSettings {
                name: set.name.clone(),
                detail: format!("{manifest} sets [tool.uv] {}", unsupported.join(", ")),
            });
        }
    }
    Ok(())
}

pub fn check_writable(set: &ResolvedSet) -> Result<(), DependencySetsError> {
    if set.writable {
        return Ok(());
    }
    Err(DependencySetsError::ReadOnlyUpdate {
        name: set.name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_UV_SETS: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "worker"
ecosystem = "uv"
manifests = ["services/worker/pyproject.toml"]
locks = ["services/worker/uv.lock"]
scopes = ["services/worker"]
"#;

    fn load_text(text: &str) -> Registry {
        parse(text, DX_TOML_REL)
            .expect("valid registry")
            .expect("registry present")
    }

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn two_same_ecosystem_sets_load_sorted_by_name() {
        let registry = load_text(TWO_UV_SETS);
        assert_eq!(registry.names(), vec!["frontend", "worker"]);
        let frontend = registry.find("frontend").expect("frontend");
        assert_eq!(frontend.ecosystem, Ecosystem::Uv);
        assert_eq!(frontend.manifests, vec!["apps/frontend/pyproject.toml"]);
        assert_eq!(frontend.locks, vec!["apps/frontend/uv.lock"]);
        assert_eq!(frontend.scopes, vec!["apps/frontend"]);
        assert!(frontend.writable);
        assert_eq!(ResolvedSet::of(frontend).dir, "apps/frontend");
    }

    #[test]
    fn missing_file_is_not_a_registry() {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-absent-");
        let workspace = scratch.path().join("absent");
        assert_eq!(load(&workspace).expect("absent"), None);
    }

    #[test]
    fn invocation_defaults_alone_are_not_a_registry() {
        let rel = DX_TOML_REL;
        let registry = parse("schema_version = 1\n[dx]\noutput = \"json\"\n", rel)
            .expect("defaults-only parses")
            .is_none();
        assert!(registry, "defaults without sets mean no registry");
    }

    #[test]
    fn shared_sections_coexist_with_sets_in_any_order() {
        let rel = DX_TOML_REL;
        let set = "[[dependency_set]]\nname = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n";
        let quality = "[quality]\nbaseline = \"q.json\"\n";
        let defaults = "[dx]\noutput = \"json\"\n";
        for text in [
            format!("schema_version = 1\n{set}{quality}{defaults}"),
            format!("schema_version = 1\n{defaults}{set}{quality}"),
        ] {
            assert!(parse(&text, rel).expect("parses").is_some());
        }
        assert!(parse(&format!("{quality}{defaults}"), rel)
            .expect("defaults only")
            .is_none());
        assert!(parse(
            "schema_version = 1\n[quality]\nbaseline = \"q.json\"\n",
            rel
        )
        .expect("quality only")
        .is_none());
        assert!(matches!(
            parse(&format!("{quality}{set}"), rel),
            Err(DependencySetsError::MissingSchema { .. })
        ));
        assert!(matches!(
            parse(
                "schema_version = 2\n[quality]\nbaseline = \"q.json\"\n",
                rel
            ),
            Err(DependencySetsError::UnsupportedSchema { version: 2, .. })
        ));
        assert!(matches!(
            parse("[quality]\nbaselnie = \"q.json\"\n", rel),
            Err(DependencySetsError::InvalidToml { .. })
        ));
    }

    #[test]
    fn missing_schema_version_names_the_want() {
        let error = parse("[[dependency_set]]\nname = \"a\"\n", DX_TOML_REL).expect_err("schema");
        assert_eq!(
            error.to_string(),
            format!("invalid {}: missing schema_version (want 1)", DX_TOML_REL)
        );
    }

    #[test]
    fn schema_problems_fail_closed() {
        let rel = DX_TOML_REL;
        assert!(matches!(
            parse("[[dependency_set]]\nname = \"a\"\n", rel),
            Err(DependencySetsError::MissingSchema { .. })
        ));
        assert!(matches!(
            parse("schema_version = 2\n", rel),
            Err(DependencySetsError::UnsupportedSchema { version: 2, .. })
        ));
        assert!(matches!(
            parse("schema_version = 1\n", rel),
            Err(DependencySetsError::NoSets { .. })
        ));
        assert!(matches!(
            parse("schema_version = 1\nunknown_key = true\n", rel),
            Err(DependencySetsError::InvalidToml { .. })
        ));
        assert!(matches!(
            parse(
                "schema_version = 1\n[[dependency_set]]\nname = \"a\"\nunknown = 1\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n",
                rel
            ),
            Err(DependencySetsError::InvalidToml { .. })
        ));
    }

    #[test]
    fn record_problems_fail_closed() {
        let rel = DX_TOML_REL;
        let base = |body: &str| format!("schema_version = 1\n[[dependency_set]]\n{body}");
        assert!(matches!(
            parse(&base("ecosystem = \"uv\"\n"), rel),
            Err(DependencySetsError::MissingName { index: 0, .. })
        ));
        assert!(matches!(
            parse(&base("name = \"\"\necosystem = \"uv\"\n"), rel),
            Err(DependencySetsError::EmptyName { .. })
        ));
        assert!(matches!(
            parse(&base("name = \"a:b\"\necosystem = \"uv\"\n"), rel),
            Err(DependencySetsError::InvalidName { .. })
        ));
        assert!(matches!(
            parse(&base("name = \"a\"\n"), rel),
            Err(DependencySetsError::MissingEcosystem { .. })
        ));
        assert!(matches!(
            parse(&base("name = \"a\"\necosystem = \"cargo\"\n"), rel),
            Err(DependencySetsError::UnknownEcosystem { .. })
        ));
        assert!(matches!(
            parse(
                &base(
                    "name = \"a\"\necosystem = \"uv\"\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n"
                ),
                rel
            ),
            Err(DependencySetsError::MissingManifests { .. })
        ));
        assert!(matches!(
            parse(
                &base("name = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nscopes = [\"a\"]\n"),
                rel
            ),
            Err(DependencySetsError::MissingLocks { .. })
        ));
        assert!(matches!(
            parse(
                &base("name = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\n"),
                rel
            ),
            Err(DependencySetsError::MissingScopes { .. })
        ));
        for bad in [
            "/abs/pyproject.toml",
            "C:/abs/pyproject.toml",
            "C:rel.toml",
            "../up/pyproject.toml",
            "a/../../up.toml",
        ] {
            assert!(
                matches!(
                    parse(
                        &base(&format!(
                            "name = \"a\"\necosystem = \"uv\"\nmanifests = [\"{bad}\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n"
                        )),
                        rel
                    ),
                    Err(DependencySetsError::EscapingPath { .. })
                ),
                "{bad} must not escape"
            );
        }
        assert!(
            matches!(
                parse(
                    &base("name = \"a\"\necosystem = \"uv\"\nmanifests = ['a\\b.toml']\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n"),
                    rel
                ),
                Err(DependencySetsError::EscapingPath { .. })
            ),
            "a literal backslash must not escape"
        );
        assert!(matches!(
            parse(
                &base("name = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\", \"b/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n"),
                rel
            ),
            Err(DependencySetsError::ScatteredManifests { .. })
        ));
    }

    #[test]
    fn duplicate_names_and_shared_writable_ownership_fail_closed() {
        let rel = DX_TOML_REL;
        let dup = format!("{TWO_UV_SETS}\n[[dependency_set]]\nname = \"frontend\"\necosystem = \"uv\"\nmanifests = [\"other/pyproject.toml\"]\nlocks = [\"other/uv.lock\"]\nscopes = [\"other\"]\n");
        assert!(matches!(
            parse(&dup, rel),
            Err(DependencySetsError::DuplicateName { .. })
        ));
        let shared = r#"
schema_version = 1

[[dependency_set]]
name = "first"
ecosystem = "uv"
manifests = ["apps/shared/pyproject.toml"]
locks = ["apps/shared/uv.lock"]
scopes = ["apps/first"]

[[dependency_set]]
name = "second"
ecosystem = "uv"
manifests = ["apps/shared/pyproject.toml"]
locks = ["apps/second/uv.lock"]
scopes = ["apps/second"]
"#;
        assert!(matches!(
            parse(shared, rel),
            Err(DependencySetsError::ConflictingOwnership { .. })
        ));
    }

    #[test]
    fn shared_read_only_ownership_is_allowed() {
        let text = r#"
schema_version = 1

[[dependency_set]]
name = "first"
ecosystem = "uv"
manifests = ["apps/shared/pyproject.toml"]
locks = ["apps/shared/uv.lock"]
scopes = ["apps/first"]
writable = false

[[dependency_set]]
name = "second"
ecosystem = "uv"
manifests = ["apps/shared/pyproject.toml"]
locks = ["apps/second/uv.lock"]
scopes = ["apps/second"]
"#;
        let registry = load_text(text);
        assert!(!registry.find("first").expect("first").writable);
        assert!(registry.find("second").expect("second").writable);
        assert!(matches!(
            check_writable(&ResolvedSet::of(registry.find("first").expect("first"))),
            Err(DependencySetsError::ReadOnlyUpdate { .. })
        ));
        assert!(check_writable(&ResolvedSet::of(registry.find("second").expect("second"))).is_ok());
    }

    #[test]
    fn bare_selection_resolves_every_configured_set_full() {
        let registry = load_text(TWO_UV_SETS);
        let resolved = resolve(&registry, &[]).expect("bare");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].set.name, "frontend");
        assert_eq!(resolved[0].request, Selection::Full);
        assert_eq!(resolved[1].set.name, "worker");
        assert_eq!(resolved[1].request, Selection::Full);
    }

    #[test]
    fn names_scopes_and_packages_resolve() {
        let registry = load_text(TWO_UV_SETS);
        let resolved = resolve(&registry, &strings(&["frontend"])).expect("name");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "frontend");
        let resolved = resolve(&registry, &strings(&["apps/frontend/app"])).expect("nested scope");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "frontend");
        let resolved =
            resolve(&registry, &strings(&["//services/worker:target"])).expect("label scope");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "worker");
        let resolved = resolve(&registry, &strings(&["//..."])).expect("repo scope");
        assert_eq!(resolved.len(), 2);
        let resolved =
            resolve(&registry, &strings(&["frontend:anyio", "frontend:httpx"])).expect("packages");
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0].request,
            Selection::Packages(vec!["anyio".to_owned(), "httpx".to_owned()])
        );
        let resolved =
            resolve(&registry, &strings(&["frontend", "frontend:httpx"])).expect("full wins");
        assert_eq!(resolved[0].request, Selection::Full);
    }

    #[test]
    fn unknown_selectors_and_scopes_fail_closed() {
        let registry = load_text(TWO_UV_SETS);
        assert!(matches!(
            resolve(&registry, &strings(&["cargo"])),
            Err(DependencySetsError::UnknownSelector { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["docs/cli/README.md"])),
            Err(DependencySetsError::NoOwningSet { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["frontend:"])),
            Err(DependencySetsError::InvalidPackage { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["frontend:bad name"])),
            Err(DependencySetsError::InvalidPackage { .. })
        ));
    }

    #[test]
    fn overlapping_scopes_resolve_to_every_owner() {
        let text = r#"
schema_version = 1

[[dependency_set]]
name = "mono"
ecosystem = "uv"
manifests = ["apps/mono/pyproject.toml"]
locks = ["apps/mono/uv.lock"]
scopes = ["apps"]

[[dependency_set]]
name = "leaf"
ecosystem = "uv"
manifests = ["apps/leaf/pyproject.toml"]
locks = ["apps/leaf/uv.lock"]
scopes = ["apps/leaf"]
"#;
        let registry = load_text(text);
        let resolved = resolve(&registry, &strings(&["apps/leaf/app"])).expect("overlap");
        assert_eq!(resolved.len(), 2);
        let resolved = resolve(&registry, &strings(&["//apps/..."])).expect("recursive");
        assert_eq!(resolved.len(), 2);
    }

    fn settings_workspace(
        name: &str,
        pyproject: &str,
        config: Option<&str>,
    ) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-settings-{name}-"));
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("apps/frontend")).expect("mkdir");
        std::fs::write(workspace.join("apps/frontend/pyproject.toml"), pyproject)
            .expect("pyproject");
        if let Some(config) = config {
            std::fs::write(
                workspace.join(config),
                "exclude-newer = \"2020-01-01T00:00:00Z\"\n",
            )
            .expect("uv.toml");
        }
        scratch
    }

    #[test]
    fn uv_settings_allow_project_metadata_only() {
        let registry = load_text(TWO_UV_SETS);
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let metadata = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\npackage = false\ndev-dependencies = []\n\n[tool.uv.sources]\nlocal = { path = \"../local\" }\n\n[tool.ruff]\nline-length = 100\n";
        let scratch = settings_workspace("ok", metadata, None);
        let workspace = scratch.path().to_path_buf();
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        let resolver = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\nexclude-newer = \"2020-01-01T00:00:00Z\"\nindex-url = \"https://example.invalid/simple\"\n";
        let scratch = settings_workspace("tool", resolver, None);
        let workspace = scratch.path().to_path_buf();
        let error = check_backend_settings(&workspace, &frontend).expect_err("resolver settings");
        assert_eq!(
            error,
            DependencySetsError::UnsupportedSettings {
                name: "frontend".to_owned(),
                detail: "apps/frontend/pyproject.toml sets [tool.uv] exclude-newer, index-url"
                    .to_owned(),
            }
        );
        let message = error.to_string();
        assert!(message.contains("\"frontend\""), "{message}");
        assert!(message.contains("does not apply"), "{message}");
    }

    #[test]
    fn uv_config_files_in_the_set_or_its_parents_are_unsupported() {
        let registry = load_text(TWO_UV_SETS);
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
        for (config, shown) in [
            ("apps/frontend/uv.toml", "apps/frontend/uv.toml exists"),
            ("apps/uv.toml", "apps/uv.toml exists"),
            ("uv.toml", "uv.toml exists"),
        ] {
            let scratch = settings_workspace(&config.replace('/', "-"), plain, Some(config));
            let workspace = scratch.path().to_path_buf();
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedSettings {
                    name: "frontend".to_owned(),
                    detail: shown.to_owned(),
                }),
                "{config}"
            );
        }
        let scratch = settings_workspace("missing", plain, None);
        let workspace = scratch.path().to_path_buf();
        std::fs::remove_file(workspace.join("apps/frontend/pyproject.toml")).expect("rm");
        assert!(matches!(
            check_backend_settings(&workspace, &frontend),
            Err(DependencySetsError::Read { .. })
        ));
    }

    #[test]
    fn parent_uv_workspaces_are_unsupported() {
        let registry = load_text(TWO_UV_SETS);
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
        for (parent, shown) in [
            (
                "apps/pyproject.toml",
                "apps/pyproject.toml sets [tool.uv.workspace]",
            ),
            ("pyproject.toml", "pyproject.toml sets [tool.uv.workspace]"),
        ] {
            let scratch = settings_workspace(&parent.replace('/', "-"), plain, None);
            let workspace = scratch.path().to_path_buf();
            std::fs::write(
                workspace.join(parent),
                "[tool.uv.workspace]\nmembers = [\"frontend\"]\n",
            )
            .expect("parent pyproject");
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedSettings {
                    name: "frontend".to_owned(),
                    detail: shown.to_owned(),
                }),
                "{parent}"
            );
        }
        let scratch = settings_workspace("plain-parent", plain, None);
        let workspace = scratch.path().to_path_buf();
        std::fs::write(workspace.join("pyproject.toml"), plain).expect("parent pyproject");
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    }

    #[test]
    fn missing_files_fail_closed_with_paths() {
        let registry = load_text(TWO_UV_SETS);
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-missing-");
        let workspace = scratch.path().join("absent");
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        assert!(matches!(
            check_files(&workspace, &frontend, true),
            Err(DependencySetsError::MissingFile { .. })
        ));
    }

    const TWO_NPM_SETS: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "npm"
manifests = ["apps/web/package.json"]
locks = ["apps/web/pnpm-lock.yaml"]
scopes = ["apps/web"]

[[dependency_set]]
name = "worker"
ecosystem = "npm"
manifests = ["services/worker/package.json"]
locks = ["services/worker/pnpm-lock.yaml"]
scopes = ["services/worker"]
"#;

    #[test]
    fn npm_sets_load_under_their_own_names() {
        let registry = load_text(TWO_NPM_SETS);
        assert_eq!(registry.names(), vec!["frontend", "worker"]);
        let frontend = registry.find("frontend").expect("frontend");
        assert_eq!(frontend.ecosystem, Ecosystem::Npm);
        assert_eq!(frontend.manifests, vec!["apps/web/package.json"]);
        assert_eq!(frontend.locks, vec!["apps/web/pnpm-lock.yaml"]);
        assert_eq!(ResolvedSet::of(frontend).dir, "apps/web");
        assert_eq!(Ecosystem::parse("npm"), Some(Ecosystem::Npm));
        assert_eq!(Ecosystem::Npm.name(), "npm");
        assert_eq!(Ecosystem::Npm.supported_check(), "check");
        assert_eq!(Ecosystem::Npm.supported_update(), "update");
        assert_eq!(Ecosystem::Npm.supported_audit(), "audit");
        assert_eq!(SUPPORTED_ECOSYSTEMS, "uv, npm, bazel, ruby, powershell");
    }

    #[test]
    fn advisory_families_follow_the_ecosystem_never_the_set_name() {
        assert_eq!(Ecosystem::Uv.advisory_family(), Some("pypi"));
        assert_eq!(Ecosystem::Npm.advisory_family(), Some("npm"));
        assert_eq!(Ecosystem::Bazel.advisory_family(), None);
        assert_eq!(Ecosystem::Ruby.advisory_family(), Some("rubygems"));
        assert_eq!(Ecosystem::PowerShell.advisory_family(), None);
        let registry = load_text(TWO_NPM_SETS);
        for set in &registry.sets {
            assert_eq!(set.ecosystem.advisory_family(), Some("npm"), "{}", set.name);
        }
    }

    #[test]
    fn manifests_must_carry_the_ecosystem_file_name() {
        let rel = DX_TOML_REL;
        for (ecosystem, manifest, want) in [
            ("uv", "a/package.json", "pyproject.toml"),
            ("uv", "a/pyproject.toml.bak", "pyproject.toml"),
            ("npm", "a/pyproject.toml", "package.json"),
            ("npm", "a/package.json5", "package.json"),
            ("npm", "package.yaml", "package.json"),
            ("bazel", "a/package.json", "MODULE.bazel"),
            ("ruby", "a/gems.rb", "Gemfile"),
            ("ruby", "a/Gemfile.lock", "Gemfile"),
            (
                "powershell",
                "a/requirements.psd1",
                "PSGallery.requirements.psd1",
            ),
        ] {
            let text = format!(
                "schema_version = 1\n[[dependency_set]]\nname = \"a\"\necosystem = \"{ecosystem}\"\nmanifests = [\"{manifest}\"]\nlocks = [\"a/lock\"]\nscopes = [\"a\"]\n"
            );
            let error = parse(&text, rel).expect_err("wrong manifest name");
            assert_eq!(
                error,
                DependencySetsError::InvalidManifest {
                    rel: rel.to_owned(),
                    name: "a".to_owned(),
                    path: manifest.to_owned(),
                    want,
                    ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
                },
                "{ecosystem} {manifest}"
            );
            assert!(error.to_string().contains(want), "{error}");
        }
        assert!(parse(
            "schema_version = 1\n[[dependency_set]]\nname = \"root\"\necosystem = \"npm\"\nmanifests = [\"package.json\"]\nlocks = [\"pnpm-lock.yaml\"]\nscopes = [\".\"]\n",
            rel
        )
        .expect("root npm set")
        .is_some());
    }

    #[test]
    fn npm_packages_allow_one_scope_and_lowercase_names() {
        let registry = load_text(TWO_NPM_SETS);
        for package in [
            "react",
            "@types/node",
            "lodash.merge",
            "a-b_c",
            "@scope/x.y",
        ] {
            let resolved = resolve(&registry, &[format!("frontend:{package}")])
                .unwrap_or_else(|error| panic!("{package}: {error}"));
            assert_eq!(
                resolved,
                vec![ResolvedTarget {
                    set: ResolvedSet::of(registry.find("frontend").expect("frontend")),
                    request: Selection::Packages(vec![package.to_owned()]),
                }]
            );
        }
        for package in [
            "React",
            "@types",
            "@/node",
            "@types/",
            "@a/b/c",
            ".hidden",
            "_private",
            "a b",
            "a;rm",
            "--registry=x",
            "pkg@1.0.0",
        ] {
            assert!(
                matches!(
                    resolve(&registry, &[format!("frontend:{package}")]),
                    Err(DependencySetsError::InvalidPackage { .. })
                ),
                "{package} must be rejected"
            );
        }
    }

    fn npm_workspace(name: &str) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-npm-{name}-"));
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("apps/web")).expect("set dir");
        std::fs::write(workspace.join("apps/web/package.json"), "{}\n").expect("manifest");
        scratch
    }

    #[test]
    fn npm_sets_need_their_own_pnpm_lock_and_no_parent_pnpm_workspace() {
        let registry = load_text(TWO_NPM_SETS);
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let scratch = npm_workspace("plain");
        let workspace = scratch.path().to_path_buf();
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        std::fs::write(
            workspace.join("apps/web/pnpm-workspace.yaml"),
            "packages: []\n",
        )
        .expect("own workspace");
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        for parent in ["apps/pnpm-workspace.yaml", "pnpm-workspace.yaml"] {
            let scratch = npm_workspace(&parent.replace('/', "-"));
            let workspace = scratch.path().to_path_buf();
            std::fs::write(workspace.join(parent), "packages: [\"apps/*\"]\n")
                .expect("parent workspace");
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedNpmLayout {
                    name: "frontend".to_owned(),
                    dir: "apps/web".to_owned(),
                    detail: format!("{parent} makes pnpm write the parent workspace lock"),
                }),
                "{parent}"
            );
        }
        let scratch = npm_workspace("other-lock");
        let workspace = scratch.path().to_path_buf();
        let mut other = frontend.clone();
        other.locks = vec!["apps/web/package-lock.json".to_owned()];
        assert_eq!(
            check_backend_settings(&workspace, &other),
            Err(DependencySetsError::UnsupportedNpmLayout {
                name: "frontend".to_owned(),
                dir: "apps/web".to_owned(),
                detail: "locks must include apps/web/pnpm-lock.yaml, the lock pnpm writes"
                    .to_owned(),
            })
        );
    }

    const PINNED_SETS: &str = r#"
schema_version = 1

[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]

[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
"#;

    #[test]
    fn ruby_and_powershell_sets_load_as_pinned_ecosystems() {
        let registry = load_text(PINNED_SETS);
        assert_eq!(registry.names(), vec!["gems", "modules"]);
        let gems = registry.find("gems").expect("gems");
        assert_eq!(gems.ecosystem, Ecosystem::Ruby);
        assert_eq!(ResolvedSet::of(gems).dir, "apps/rb");
        let modules = registry.find("modules").expect("modules");
        assert_eq!(modules.ecosystem, Ecosystem::PowerShell);
        for (ecosystem, name, manifest) in [
            (Ecosystem::Ruby, "ruby", "Gemfile"),
            (
                Ecosystem::PowerShell,
                "powershell",
                "PSGallery.requirements.psd1",
            ),
        ] {
            assert_eq!(Ecosystem::parse(name), Some(ecosystem));
            assert_eq!(ecosystem.name(), name);
            assert_eq!(ecosystem.manifest_name(), manifest);
            assert_eq!(ecosystem.supported_check(), "pinned");
            assert_eq!(ecosystem.supported_update(), "pinned");
            assert_eq!(ecosystem.supported_audit(), "audit");
            assert_eq!(
                ecosystem.supported_operations(),
                "audit; check and update report pinned"
            );
        }
        assert_eq!(Ecosystem::Uv.supported_operations(), "check, update, audit");
        assert_eq!(
            Ecosystem::Npm.supported_operations(),
            "check, update, audit"
        );
        assert_eq!(Ecosystem::parse("rubygems"), None);
        assert_eq!(Ecosystem::parse("PowerShell"), None);
    }

    #[test]
    fn pinned_sets_must_declare_the_lock_beside_their_manifest() {
        let registry = load_text(PINNED_SETS);
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-pinned-");
        let workspace = scratch.path().to_path_buf();
        for name in ["gems", "modules"] {
            let set = ResolvedSet::of(registry.find(name).expect("set"));
            assert_eq!(check_backend_settings(&workspace, &set), Ok(()), "{name}");
        }
        for (ecosystem, manifest, lock, want) in [
            (
                "ruby",
                "apps/rb/Gemfile",
                "apps/rb/gems.locked",
                "apps/rb/Gemfile.lock",
            ),
            ("ruby", "Gemfile", "other/Gemfile.lock", "Gemfile.lock"),
            (
                "powershell",
                "tools/ps/PSGallery.requirements.psd1",
                "tools/ps/lock.json",
                "tools/ps/PSGallery.lock.json",
            ),
        ] {
            let text = format!(
                "schema_version = 1\n[[dependency_set]]\nname = \"a\"\necosystem = \"{ecosystem}\"\nmanifests = [\"{manifest}\"]\nlocks = [\"{lock}\"]\nscopes = [\".\"]\n"
            );
            let registry = parse(&text, DX_TOML_REL).expect("valid").expect("registry");
            let set = ResolvedSet::of(registry.find("a").expect("a"));
            let error = check_backend_settings(&workspace, &set).expect_err("missing lock");
            assert_eq!(
                error,
                DependencySetsError::MissingPinnedLock {
                    name: "a".to_owned(),
                    path: want.to_owned(),
                    ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
                },
                "{ecosystem} {lock}"
            );
            assert!(error.to_string().contains(want), "{error}");
        }
    }

    #[test]
    fn pinned_packages_use_the_plain_charset() {
        let registry = load_text(PINNED_SETS);
        for (set, package) in [
            ("gems", "rspec-core"),
            ("gems", "nokogiri"),
            ("gems", "Ruby_Gem.2"),
            ("modules", "Pester"),
            ("modules", "Az.Accounts"),
        ] {
            let resolved = resolve(&registry, &[format!("{set}:{package}")])
                .unwrap_or_else(|error| panic!("{set}:{package}: {error}"));
            assert_eq!(
                resolved[0].request,
                Selection::Packages(vec![package.to_owned()])
            );
        }
        for (set, package, reason) in [
            ("gems", "a b", RUBY_PACKAGE_CHARSET),
            ("gems", "rails@7", RUBY_PACKAGE_CHARSET),
            ("gems", "a;rm", RUBY_PACKAGE_CHARSET),
            ("modules", "Az/Accounts", POWERSHELL_PACKAGE_CHARSET),
            ("modules", "Az Accounts", POWERSHELL_PACKAGE_CHARSET),
        ] {
            assert_eq!(
                resolve(&registry, &[format!("{set}:{package}")]),
                Err(DependencySetsError::InvalidPackage {
                    set: set.to_owned(),
                    package: package.to_owned(),
                    reason,
                }),
                "{set}:{package}"
            );
        }
    }
}
