use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const DX_TOML_REL: &str = "dx.toml";

pub const SCHEMA_VERSION: u32 = 1;

pub const ECOSYSTEM_UV: &str = "uv";

pub const SUPPORTED_ECOSYSTEMS: &str = ECOSYSTEM_UV;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Ecosystem {
    Uv,
}

impl Ecosystem {
    pub fn name(self) -> &'static str {
        match self {
            Ecosystem::Uv => ECOSYSTEM_UV,
        }
    }

    pub fn parse(text: &str) -> Option<Ecosystem> {
        match text {
            ECOSYSTEM_UV => Some(Ecosystem::Uv),
            _ => None,
        }
    }

    pub fn supported_check(self) -> &'static str {
        match self {
            Ecosystem::Uv => "check",
        }
    }

    pub fn supported_update(self) -> &'static str {
        match self {
            Ecosystem::Uv => "update",
        }
    }

    pub fn supported_audit(self) -> &'static str {
        match self {
            Ecosystem::Uv => "audit",
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
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DxToml {
    schema_version: Option<u32>,
    #[serde(default)]
    dependency_set: Vec<RawSet>,
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
    match document.schema_version {
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
    if path.contains('\\') {
        return true;
    }
    if Path::new(path).is_absolute() {
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

const UV_PACKAGE_CHARSET: &str = "uv package names use [A-Za-z0-9_.-] only";

fn validate_package(ecosystem: Ecosystem, package: &str) -> Result<(), &'static str> {
    match ecosystem {
        Ecosystem::Uv => {
            if package.is_empty() {
                return Err(UV_PACKAGE_CHARSET);
            }
            for char in package.chars() {
                if !(char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-') {
                    return Err(UV_PACKAGE_CHARSET);
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
        let workspace = std::env::temp_dir().join("dx-dependency-sets-absent");
        assert_eq!(load(&workspace).expect("absent"), None);
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

    #[test]
    fn missing_files_fail_closed_with_paths() {
        let registry = load_text(TWO_UV_SETS);
        let workspace = std::env::temp_dir().join("dx-dependency-sets-missing");
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        assert!(matches!(
            check_files(&workspace, &frontend, true),
            Err(DependencySetsError::MissingFile { .. })
        ));
    }
}
