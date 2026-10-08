use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::selector::{has_prefix, is_recursive, package_path, SetRequest};
use super::sets::SetId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Cargo,
    Go,
    Maven,
    Npm,
    NuGet,
    PowerShell,
    Ruby,
    Uv,
}

impl BackendKind {
    pub fn name(self) -> &'static str {
        match self {
            BackendKind::Cargo => "cargo",
            BackendKind::Go => "go",
            BackendKind::Maven => "maven",
            BackendKind::Npm => "npm",
            BackendKind::NuGet => "nuget",
            BackendKind::PowerShell => "powershell",
            BackendKind::Ruby => "ruby",
            BackendKind::Uv => "uv",
        }
    }

    pub fn validate_package(self, package: &str) -> Result<(), &'static str> {
        match self {
            BackendKind::Cargo => dx_identity::validate_cargo(package),
            BackendKind::Npm => dx_identity::validate_npm(package),
            BackendKind::Maven => dx_identity::validate_maven(package),
            BackendKind::NuGet => dx_identity::validate_nuget(package),
            BackendKind::Go => dx_identity::validate_go(package),
            BackendKind::Uv => dx_identity::validate_dotted(package, UV_CHARSET),
            BackendKind::Ruby => dx_identity::validate_dotted(package, RUBY_CHARSET),
            BackendKind::PowerShell => dx_identity::validate_dotted(package, POWERSHELL_CHARSET),
        }
    }
}

pub fn backend_kind(set: SetId) -> BackendKind {
    match set {
        SetId::Cargo => BackendKind::Cargo,
        SetId::Go => BackendKind::Go,
        SetId::Maven => BackendKind::Maven,
        SetId::Npm | SetId::NpmTools | SetId::NpmAdopt | SetId::NpmAdoptPolyglot => {
            BackendKind::Npm
        }
        SetId::NuGet => BackendKind::NuGet,
        SetId::PowerShell => BackendKind::PowerShell,
        SetId::Ruby => BackendKind::Ruby,
        SetId::Uv | SetId::UvTools | SetId::UvAdopt | SetId::UvAdoptPolyglot => BackendKind::Uv,
    }
}

const UV_CHARSET: &str = "uv package names use [A-Za-z0-9_.-] only";
const RUBY_CHARSET: &str = "ruby gem names use [A-Za-z0-9_.-] only";
const POWERSHELL_CHARSET: &str = "powershell module names use [A-Za-z0-9_.-] only";
const SET_CHARSET: &str = "set names use [A-Za-z0-9_.-] only";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencySet {
    pub name: String,
    pub backend: BackendKind,
    #[serde(default)]
    pub manifests: Vec<String>,
    #[serde(default)]
    pub locks: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSets {
    pub schema_version: u32,
    #[serde(default)]
    pub dependency_set: Vec<DependencySet>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {message}")]
    Io { path: String, message: String },
    #[error("dx.toml is invalid: {message}")]
    Parse { message: String },
    #[error("unsupported schema_version {actual}: want 1")]
    SchemaVersion { actual: u32 },
    #[error("invalid set name {name:?}: {reason}")]
    BadName { name: String, reason: &'static str },
    #[error("duplicate dependency set {name:?}")]
    DuplicateName { name: String },
    #[error("dependency set {name:?} owns no manifests")]
    EmptyManifests { name: String },
    #[error("dependency set {name:?} owns no locks")]
    EmptyLocks { name: String },
    #[error("invalid path {path:?} for dependency set {name:?}: {reason}")]
    BadPath {
        name: String,
        path: String,
        reason: &'static str,
    },
    #[error(
        "unknown option {key:?} for dependency set {name:?}: {backend} sets accept {allowed:?}"
    )]
    UnknownOption {
        name: String,
        key: String,
        backend: &'static str,
        allowed: Vec<String>,
    },
    #[error(
        "invalid value {value:?} for option {key:?} of dependency set {name:?}: want {want:?}"
    )]
    BadOptionValue {
        name: String,
        key: String,
        value: String,
        want: Vec<String>,
    },
    #[error("conflicting writable ownership of {path:?}: {first:?} and {second:?}")]
    ConflictingOwnership {
        path: String,
        first: String,
        second: String,
    },
}

fn check_set_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("set name is empty");
    }
    let valid = name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(SET_CHARSET)
    }
}

fn check_record_path(name: &str, path: &str) -> Result<(), ConfigError> {
    match dx_path::reject_reason(path) {
        None => Ok(()),
        Some(reason) => Err(ConfigError::BadPath {
            name: name.to_owned(),
            path: path.to_owned(),
            reason,
        }),
    }
}

const NPM_MODES: &[&str] = &["update", "install"];
const NPM_RUNNERS: &[&str] = &["bazel", "host"];

fn normalize_options(record: &mut DependencySet) -> Result<(), ConfigError> {
    if record.backend != BackendKind::Npm {
        if let Some(key) = record.options.keys().next() {
            return Err(ConfigError::UnknownOption {
                name: record.name.clone(),
                key: key.clone(),
                backend: record.backend.name(),
                allowed: Vec::new(),
            });
        }
        return Ok(());
    }
    for key in record.options.keys() {
        if key != "mode" && key != "runner" {
            return Err(ConfigError::UnknownOption {
                name: record.name.clone(),
                key: key.clone(),
                backend: record.backend.name(),
                allowed: vec!["mode".to_owned(), "runner".to_owned()],
            });
        }
    }
    record
        .options
        .entry("mode".to_owned())
        .or_insert_with(|| "update".to_owned());
    record
        .options
        .entry("runner".to_owned())
        .or_insert_with(|| "host".to_owned());
    let mode = record.options["mode"].clone();
    if !NPM_MODES.contains(&mode.as_str()) {
        return Err(ConfigError::BadOptionValue {
            name: record.name.clone(),
            key: "mode".to_owned(),
            value: mode,
            want: NPM_MODES.iter().map(|mode| (*mode).to_owned()).collect(),
        });
    }
    let runner = record.options["runner"].clone();
    if !NPM_RUNNERS.contains(&runner.as_str()) {
        return Err(ConfigError::BadOptionValue {
            name: record.name.clone(),
            key: "runner".to_owned(),
            value: runner,
            want: NPM_RUNNERS
                .iter()
                .map(|runner| (*runner).to_owned())
                .collect(),
        });
    }
    Ok(())
}

pub fn validate_records(records: &mut [DependencySet]) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for record in records.iter() {
        check_set_name(&record.name).map_err(|reason| ConfigError::BadName {
            name: record.name.clone(),
            reason,
        })?;
        if !seen.insert(record.name.clone()) {
            return Err(ConfigError::DuplicateName {
                name: record.name.clone(),
            });
        }
        if record.manifests.is_empty() {
            return Err(ConfigError::EmptyManifests {
                name: record.name.clone(),
            });
        }
        if record.locks.is_empty() {
            return Err(ConfigError::EmptyLocks {
                name: record.name.clone(),
            });
        }
        for path in record
            .manifests
            .iter()
            .chain(record.locks.iter())
            .chain(record.scopes.iter())
        {
            check_record_path(&record.name, path)?;
        }
    }
    for record in records.iter_mut() {
        normalize_options(record)?;
    }
    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for record in records.iter() {
        for path in record.manifests.iter().chain(record.locks.iter()) {
            if let Some(first) = owners.insert(path.as_str(), record.name.as_str()) {
                if first != record.name.as_str() {
                    return Err(ConfigError::ConflictingOwnership {
                        path: path.clone(),
                        first: first.to_owned(),
                        second: record.name.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn builtin_scopes(set: SetId) -> Vec<String> {
    super::selector::owning_prefixes()
        .iter()
        .filter(|(_, sets)| sets.contains(&set))
        .map(|(prefix, _)| (*prefix).to_owned())
        .collect()
}

fn builtin_options(set: SetId) -> BTreeMap<String, String> {
    let mut options = BTreeMap::new();
    match set {
        SetId::Npm => {
            options.insert("mode".to_owned(), "update".to_owned());
            options.insert("runner".to_owned(), "bazel".to_owned());
        }
        SetId::NpmTools => {
            options.insert("mode".to_owned(), "install".to_owned());
            options.insert("runner".to_owned(), "bazel".to_owned());
        }
        SetId::NpmAdopt | SetId::NpmAdoptPolyglot => {
            options.insert("mode".to_owned(), "install".to_owned());
            options.insert("runner".to_owned(), "host".to_owned());
        }
        _ => {}
    }
    options
}

pub fn builtin_sets() -> Vec<DependencySet> {
    SetId::ALL
        .iter()
        .map(|set| DependencySet {
            name: set.name().to_owned(),
            backend: backend_kind(*set),
            manifests: set
                .manifests()
                .iter()
                .map(|path| (*path).to_owned())
                .collect(),
            locks: set.locks().iter().map(|path| (*path).to_owned()).collect(),
            scopes: builtin_scopes(*set),
            options: builtin_options(*set),
        })
        .collect()
}

pub fn builtin_record(name: &str) -> Option<DependencySet> {
    builtin_sets()
        .into_iter()
        .find(|record| record.name == name)
}

pub fn load_workspace_sets(workspace: &Path) -> Result<Vec<DependencySet>, ConfigError> {
    let path = workspace.join("dx.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(builtin_sets()),
        Err(error) => {
            return Err(ConfigError::Io {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
        }
    };
    let parsed: WorkspaceSets = toml::from_str(&text).map_err(|error| ConfigError::Parse {
        message: error.to_string(),
    })?;
    if parsed.schema_version != 1 {
        return Err(ConfigError::SchemaVersion {
            actual: parsed.schema_version,
        });
    }
    let mut records = parsed.dependency_set;
    validate_records(&mut records)?;
    Ok(records)
}

pub fn record_dir(record: &DependencySet) -> String {
    match record
        .manifests
        .first()
        .and_then(|manifest| manifest.rsplit_once('/'))
    {
        Some((dir, _)) => dir.to_owned(),
        None => String::new(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ResolveError {
    #[error("empty selector")]
    Empty,
    #[error("unknown dependency set {selector:?}: want {known}")]
    UnknownSet { selector: String, known: String },
    #[error("invalid package {package:?} for dependency set {set}: {reason}")]
    InvalidPackage {
        set: String,
        package: String,
        reason: &'static str,
    },
    #[error(
        "no owning dependency set for {target:?} (non-dependency paths are out of update scope)"
    )]
    NoOwningSet { target: String },
}

fn is_target_shape(text: &str) -> bool {
    text == "..."
        || text.starts_with("//")
        || text.starts_with('@')
        || text.contains('/')
        || text.contains('.')
}

fn owning_records<'a>(records: &'a [DependencySet], target: &str) -> Vec<&'a DependencySet> {
    if target == "//..." || target == "..." {
        return records.iter().collect();
    }
    let package = package_path(target);
    let mut owners: BTreeSet<usize> = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        for scope in &record.scopes {
            if has_prefix(&package, scope) {
                owners.insert(index);
                break;
            }
        }
    }
    if is_recursive(target) {
        for (index, record) in records.iter().enumerate() {
            for scope in &record.scopes {
                if has_prefix(scope, &package) {
                    owners.insert(index);
                    break;
                }
            }
        }
    }
    if owners.is_empty() {
        for (index, record) in records.iter().enumerate() {
            let owns = record
                .manifests
                .iter()
                .chain(record.locks.iter())
                .any(|path| path == target);
            if owns {
                owners.insert(index);
            }
        }
    }
    owners.into_iter().map(|index| &records[index]).collect()
}

pub fn resolve_named(
    records: &[DependencySet],
    selectors: &[String],
) -> Result<BTreeMap<String, SetRequest>, ResolveError> {
    if selectors.is_empty() {
        return Ok(records
            .iter()
            .map(|record| (record.name.clone(), SetRequest::Full))
            .collect());
    }
    let by_name: BTreeMap<&str, &DependencySet> = records
        .iter()
        .map(|record| (record.name.as_str(), record))
        .collect();
    let mut full: BTreeSet<String> = BTreeSet::new();
    let mut packages: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for raw in selectors {
        if raw.is_empty() {
            return Err(ResolveError::Empty);
        }
        if let Some((head, tail)) = raw.split_once(':') {
            if let Some(record) = by_name.get(head) {
                if tail.is_empty() {
                    return Err(ResolveError::InvalidPackage {
                        set: record.name.clone(),
                        package: tail.to_owned(),
                        reason: "package identity is empty",
                    });
                }
                if let Err(reason) = record.backend.validate_package(tail) {
                    return Err(ResolveError::InvalidPackage {
                        set: record.name.clone(),
                        package: tail.to_owned(),
                        reason,
                    });
                }
                packages
                    .entry(record.name.clone())
                    .or_default()
                    .push(tail.to_owned());
                continue;
            }
        }
        if let Some(record) = by_name.get(raw.as_str()) {
            full.insert(record.name.clone());
            continue;
        }
        if is_target_shape(raw) {
            let owners = owning_records(records, raw);
            if owners.is_empty() {
                return Err(ResolveError::NoOwningSet {
                    target: raw.clone(),
                });
            }
            for record in owners {
                full.insert(record.name.clone());
            }
            continue;
        }
        let mut known: Vec<&str> = by_name.keys().copied().collect();
        known.sort();
        return Err(ResolveError::UnknownSet {
            selector: raw.clone(),
            known: known.join(", "),
        });
    }
    let mut resolved = BTreeMap::new();
    for name in full {
        resolved.insert(name, SetRequest::Full);
    }
    for (name, mut names) in packages {
        if resolved.contains_key(&name) {
            continue;
        }
        names.sort();
        names.dedup();
        resolved.insert(name, SetRequest::Packages(names));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("dx_update_config_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch workspace");
        root
    }

    fn web_record() -> DependencySet {
        DependencySet {
            name: "web".to_owned(),
            backend: BackendKind::Npm,
            manifests: vec!["apps/web/package.json".to_owned()],
            locks: vec!["apps/web/pnpm-lock.yaml".to_owned()],
            scopes: vec!["apps/web".to_owned()],
            options: BTreeMap::new(),
        }
    }

    fn api_record() -> DependencySet {
        DependencySet {
            name: "api".to_owned(),
            backend: BackendKind::Npm,
            manifests: vec!["services/api/package.json".to_owned()],
            locks: vec!["services/api/pnpm-lock.yaml".to_owned()],
            scopes: vec!["services/api".to_owned()],
            options: BTreeMap::new(),
        }
    }

    fn consumer_pair() -> Vec<DependencySet> {
        vec![web_record(), api_record()]
    }

    fn write_tree(root: &Path, files: &[(&str, &str)]) {
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent dirs");
            }
            std::fs::write(&path, content).expect("write fixture");
        }
    }

    fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_owned()];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).expect("read dir");
            for entry in entries {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path
                        .strip_prefix(root)
                        .expect("workspace-relative")
                        .to_string_lossy()
                        .into_owned();
                    out.insert(rel, std::fs::read(&path).expect("read file"));
                }
            }
        }
        out
    }

    #[test]
    fn resolution_and_planning_mutate_nothing() {
        use super::super::backend::{check_named, plan_named};
        let root = scratch("no-mutate");
        write_tree(
            &root,
            &[
                (
                    "dx.toml",
                    "schema_version = 1\n\
                     [[dependency_set]]\n\
                     name = \"web\"\n\
                     backend = \"npm\"\n\
                     manifests = [\"apps/web/package.json\"]\n\
                     locks = [\"apps/web/pnpm-lock.yaml\"]\n\
                     scopes = [\"apps/web\"]\n\
                     [[dependency_set]]\n\
                     name = \"api\"\n\
                     backend = \"npm\"\n\
                     manifests = [\"services/api/package.json\"]\n\
                     locks = [\"services/api/pnpm-lock.yaml\"]\n\
                     scopes = [\"services/api\"]\n",
                ),
                ("apps/web/package.json", "{}\n"),
                ("services/api/package.json", "{}\n"),
            ],
        );
        let before = snapshot(&root);
        let records = load_workspace_sets(&root).expect("consumer sets load");
        let resolved = resolve_named(&records, &strings(&["//..."])).expect("repo resolves");
        assert_eq!(resolved.len(), 2);
        for record in &records {
            let request = SetRequest::Full;
            let _ = plan_named(&root, record, &request, false);
            let _ = check_named(&root, record, &request, false);
        }
        assert_eq!(snapshot(&root), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backend_kinds_cover_the_v1_manager_set() {
        assert_eq!(BackendKind::Cargo.name(), "cargo");
        assert_eq!(BackendKind::Go.name(), "go");
        assert_eq!(BackendKind::Maven.name(), "maven");
        assert_eq!(BackendKind::Npm.name(), "npm");
        assert_eq!(BackendKind::NuGet.name(), "nuget");
        assert_eq!(BackendKind::PowerShell.name(), "powershell");
        assert_eq!(BackendKind::Ruby.name(), "ruby");
        assert_eq!(BackendKind::Uv.name(), "uv");
        assert_eq!(backend_kind(SetId::NpmTools), BackendKind::Npm);
        assert_eq!(backend_kind(SetId::NpmAdopt), BackendKind::Npm);
        assert_eq!(backend_kind(SetId::NpmAdoptPolyglot), BackendKind::Npm);
        assert_eq!(backend_kind(SetId::UvTools), BackendKind::Uv);
        assert_eq!(backend_kind(SetId::UvAdopt), BackendKind::Uv);
        assert_eq!(backend_kind(SetId::UvAdoptPolyglot), BackendKind::Uv);
        assert_eq!(backend_kind(SetId::Cargo), BackendKind::Cargo);
    }

    #[test]
    fn package_validation_matches_the_legacy_selector_ladder() {
        for package in ["react", "@astrojs/compiler"] {
            assert!(BackendKind::Npm.validate_package(package).is_ok());
            assert!(super::super::selector::parse_selector(&format!("npm:{package}")).is_ok());
        }
        for package in ["a b", "a/b", "bad!"] {
            assert!(BackendKind::Npm.validate_package(package).is_err());
            assert!(super::super::selector::parse_selector(&format!("npm:{package}")).is_err());
        }
        assert!(BackendKind::Cargo.validate_package("anyhow").is_ok());
        assert!(BackendKind::Cargo.validate_package("bad name").is_err());
        assert!(BackendKind::Maven.validate_package("junit:junit").is_ok());
        assert!(BackendKind::Maven.validate_package("junit").is_err());
        assert!(BackendKind::Uv.validate_package("pytest").is_ok());
        assert!(BackendKind::Ruby.validate_package("rspec-core").is_ok());
        assert!(BackendKind::PowerShell.validate_package("Pester").is_ok());
        assert!(BackendKind::Go
            .validate_package("github.com/google/go-cmp/cmp")
            .is_ok());
        assert!(BackendKind::Go.validate_package("a:b").is_err());
    }

    #[test]
    fn builtin_sets_are_valid_records_with_stable_names() {
        let mut records = builtin_sets();
        validate_records(&mut records).expect("builtin records validate");
        assert_eq!(records.len(), 14);
        let names: Vec<&str> = records.iter().map(|record| record.name.as_str()).collect();
        assert!(names.contains(&"npm-tools"));
        assert!(names.contains(&"uv-adopt-polyglot"));
        for record in &records {
            assert!(!record.manifests.is_empty(), "{}", record.name);
            assert!(!record.locks.is_empty(), "{}", record.name);
            assert!(!record.scopes.is_empty(), "{}", record.name);
        }
        let npm = builtin_record("npm").expect("npm record");
        assert_eq!(npm.backend, BackendKind::Npm);
        assert_eq!(npm.manifests, vec!["package.json".to_owned()]);
        assert_eq!(npm.options["mode"], "update");
        assert_eq!(npm.options["runner"], "bazel");
        let tools = builtin_record("npm-tools").expect("npm-tools record");
        assert_eq!(tools.options["mode"], "install");
        assert_eq!(tools.options["runner"], "bazel");
        let adopt = builtin_record("npm-adopt").expect("npm-adopt record");
        assert_eq!(adopt.options["mode"], "install");
        assert_eq!(adopt.options["runner"], "host");
        assert!(builtin_record("pip").is_none());
    }

    #[test]
    fn missing_dx_toml_falls_back_to_builtin_sets() {
        let root = scratch("fallback");
        let records = load_workspace_sets(&root).expect("builtin fallback");
        assert_eq!(records.len(), 14);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn consumer_dx_toml_loads_two_npm_sets_without_phantoms() {
        let root = scratch("consumer-pair");
        write_tree(
            &root,
            &[
                (
                    "dx.toml",
                    "schema_version = 1\n\
                     [[dependency_set]]\n\
                     name = \"web\"\n\
                     backend = \"npm\"\n\
                     manifests = [\"apps/web/package.json\"]\n\
                     locks = [\"apps/web/pnpm-lock.yaml\"]\n\
                     scopes = [\"apps/web\"]\n\
                     [[dependency_set]]\n\
                     name = \"api\"\n\
                     backend = \"npm\"\n\
                     manifests = [\"services/api/package.json\"]\n\
                     locks = [\"services/api/pnpm-lock.yaml\"]\n\
                     scopes = [\"services/api\"]\n",
                ),
                ("apps/web/package.json", "{}\n"),
                ("apps/web/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
                ("services/api/package.json", "{}\n"),
                ("services/api/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
            ],
        );
        let records = load_workspace_sets(&root).expect("consumer sets load");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "web");
        assert_eq!(records[1].name, "api");
        assert_eq!(records[0].options["mode"], "update");
        assert_eq!(records[0].options["runner"], "host");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn consumer_pair_selects_each_set_independently() {
        let records = consumer_pair();
        let resolved = resolve_named(&records, &strings(&["web"])).expect("web selects");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved.get("web"), Some(&SetRequest::Full));
        let resolved = resolve_named(&records, &strings(&["api"])).expect("api selects");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved.get("api"), Some(&SetRequest::Full));
        let resolved = resolve_named(&records, &[]).expect("bare selects both");
        assert_eq!(resolved.len(), 2);
        let resolved =
            resolve_named(&records, &strings(&["web:react"])).expect("web package selects");
        assert_eq!(
            resolved.get("web"),
            Some(&SetRequest::Packages(vec!["react".to_owned()]))
        );
        assert!(!resolved.contains_key("api"));
        let resolved =
            resolve_named(&records, &strings(&["//apps/web:bundle"])).expect("web scope selects");
        assert_eq!(resolved.get("web"), Some(&SetRequest::Full));
        assert!(!resolved.contains_key("api"));
        let resolved = resolve_named(&records, &strings(&["//..."])).expect("repo selects both");
        assert_eq!(resolved.len(), 2);
        let resolved = resolve_named(&records, &strings(&["services/api/pnpm-lock.yaml"]))
            .expect("api lock selects");
        assert_eq!(resolved.get("api"), Some(&SetRequest::Full));
        assert!(!resolved.contains_key("web"));
    }

    #[test]
    fn overlapping_read_only_scopes_map_to_both_sets() {
        let mut records = consumer_pair();
        records[0].scopes.push("shared".to_owned());
        records[1].scopes.push("shared".to_owned());
        let mut validated = records.clone();
        validate_records(&mut validated).expect("overlapping scopes validate");
        let resolved =
            resolve_named(&records, &strings(&["//shared/widget:widget"])).expect("shared scope");
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn unknown_and_unowned_consumer_selectors_fail_closed() {
        let records = consumer_pair();
        assert!(matches!(
            resolve_named(&records, &strings(&["shop"])),
            Err(ResolveError::UnknownSet { .. })
        ));
        let error = resolve_named(&records, &strings(&["shop"])).expect_err("unknown set");
        assert!(error.to_string().contains("api"));
        assert!(error.to_string().contains("web"));
        assert!(matches!(
            resolve_named(&records, &strings(&["docs/cli/README.md"])),
            Err(ResolveError::NoOwningSet { .. })
        ));
        assert!(matches!(
            resolve_named(&records, &strings(&[""])),
            Err(ResolveError::Empty)
        ));
        assert!(matches!(
            resolve_named(&records, &strings(&["web:"])),
            Err(ResolveError::InvalidPackage { .. })
        ));
        assert!(matches!(
            resolve_named(&records, &strings(&["web:bad name"])),
            Err(ResolveError::InvalidPackage { .. })
        ));
        assert!(matches!(
            resolve_named(&records, &strings(&["web:react", "web"])),
            Ok(_)
        ));
        let resolved = resolve_named(&records, &strings(&["web:react", "web"])).expect("full wins");
        assert_eq!(resolved.get("web"), Some(&SetRequest::Full));
    }

    #[test]
    fn consumer_records_reject_bad_names_paths_and_options() {
        let mut records = consumer_pair();
        records[0].name = "bad name".to_owned();
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::BadName { .. })
        ));
        let mut records = consumer_pair();
        records[0].name = String::new();
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::BadName { .. })
        ));
        let mut records = consumer_pair();
        records[1].name = "web".to_owned();
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::DuplicateName { .. })
        ));
        let mut records = consumer_pair();
        records[0].manifests.clear();
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::EmptyManifests { .. })
        ));
        let mut records = consumer_pair();
        records[0].locks.clear();
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::EmptyLocks { .. })
        ));
        let mut records = consumer_pair();
        records[0].locks = vec!["/abs/pnpm-lock.yaml".to_owned()];
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::BadPath { .. })
        ));
        let mut records = consumer_pair();
        records[0].manifests = vec!["apps/../evil/package.json".to_owned()];
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::BadPath { .. })
        ));
        let mut records = consumer_pair();
        records[0]
            .options
            .insert("mode".to_owned(), "wipe".to_owned());
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::BadOptionValue { .. })
        ));
        let mut records = consumer_pair();
        records[0]
            .options
            .insert("parallel".to_owned(), "true".to_owned());
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::UnknownOption { .. })
        ));
        let mut records = consumer_pair();
        records[0].backend = BackendKind::Cargo;
        records[0]
            .options
            .insert("mode".to_owned(), "update".to_owned());
        assert!(matches!(
            validate_records(&mut records),
            Err(ConfigError::UnknownOption { .. })
        ));
        let mut records = consumer_pair();
        records[1].locks = vec!["apps/web/pnpm-lock.yaml".to_owned()];
        let error = validate_records(&mut records).expect_err("conflicting lock");
        assert!(matches!(error, ConfigError::ConflictingOwnership { .. }));
        assert!(error.to_string().contains("apps/web/pnpm-lock.yaml"));
        assert!(error.to_string().contains("web"));
        assert!(error.to_string().contains("api"));
    }

    #[test]
    fn dx_toml_rejects_bad_schema_unknown_kinds_and_unknown_keys() {
        let root = scratch("dx-toml-errors");
        write_tree(
            &root,
            &[(
                "dx.toml",
                "schema_version = 2\n[[dependency_set]]\nname = \"web\"\nbackend = \"npm\"\n\
                 manifests = [\"apps/web/package.json\"]\nlocks = [\"apps/web/pnpm-lock.yaml\"]\n",
            )],
        );
        assert!(matches!(
            load_workspace_sets(&root),
            Err(ConfigError::SchemaVersion { actual: 2 })
        ));
        write_tree(
            &root,
            &[(
                "dx.toml",
                "schema_version = 1\n[[dependency_set]]\nname = \"web\"\nbackend = \"pip\"\n\
                 manifests = [\"apps/web/package.json\"]\nlocks = [\"apps/web/lock\"]\n",
            )],
        );
        let error = load_workspace_sets(&root).expect_err("unknown backend");
        assert!(matches!(error, ConfigError::Parse { .. }));
        assert!(error.to_string().contains("pip"));
        write_tree(
            &root,
            &[(
                "dx.toml",
                "schema_version = 1\nstrange = true\n[[dependency_set]]\nname = \"web\"\n\
                 backend = \"npm\"\nmanifests = [\"apps/web/package.json\"]\n\
                 locks = [\"apps/web/pnpm-lock.yaml\"]\n",
            )],
        );
        assert!(matches!(
            load_workspace_sets(&root),
            Err(ConfigError::Parse { .. })
        ));
        write_tree(
            &root,
            &[(
                "dx.toml",
                "schema_version = 1\n[[dependency_set]]\nname = \"web\"\nbackend = \"npm\"\n\
                 manifests = [\"apps/web/package.json\"]\nlocks = [\"apps/web/pnpm-lock.yaml\"]\n\
                 mirrors = [\"everywhere\"]\n",
            )],
        );
        assert!(matches!(
            load_workspace_sets(&root),
            Err(ConfigError::Parse { .. })
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn record_dir_is_the_manifest_parent() {
        assert_eq!(record_dir(&web_record()), "apps/web");
        assert_eq!(record_dir(&builtin_record("npm").expect("npm")), "");
        assert_eq!(
            record_dir(&builtin_record("npm-tools").expect("tools")),
            "quality/tools/javascript"
        );
    }

    #[test]
    fn named_resolution_agrees_with_legacy_on_the_shared_surface() {
        use super::super::selector;
        let builtin = builtin_sets();
        let legacy = |selectors: &[&str]| {
            selector::resolve(&strings(selectors))
                .map(|resolved| {
                    resolved
                        .into_iter()
                        .map(|(set, request)| (set.name().to_owned(), request))
                        .collect::<BTreeMap<_, _>>()
                })
                .map_err(|error| error.to_string())
        };
        let named = |selectors: &[&str]| {
            resolve_named(&builtin, &strings(selectors)).map_err(|error| error.to_string())
        };
        for selectors in [
            vec![],
            vec!["npm"],
            vec!["cargo"],
            vec!["npm:react"],
            vec!["npm:react", "npm:jest"],
            vec!["maven:org.junit.jupiter:junit-jupiter-api"],
            vec!["npm", "npm:react"],
            vec!["//..."],
            vec!["//rust/tests/fixtures/hello:hello"],
            vec!["rust/tests/fixtures/hello/Cargo.toml"],
            vec!["//quality/..."],
            vec!["//quality/tools"],
            vec!["//examples/adopt-polyglot/..."],
            vec!["//third_party/go/..."],
            vec!["pnpm-lock.yaml"],
            vec!["package.json"],
        ] {
            assert_eq!(named(&selectors), legacy(&selectors), "{selectors:?}");
        }
        assert!(matches!(
            selector::resolve(&strings(&["docs/cli/README.md"])),
            Err(selector::SelectorError::NoOwningSet { .. })
        ));
        assert!(matches!(
            resolve_named(&builtin, &strings(&["docs/cli/README.md"])),
            Err(ResolveError::NoOwningSet { .. })
        ));
        assert!(matches!(
            selector::resolve(&strings(&["crates"])),
            Err(selector::SelectorError::UnknownSelector { .. })
        ));
        assert!(matches!(
            resolve_named(&builtin, &strings(&["crates"])),
            Err(ResolveError::UnknownSet { .. })
        ));
    }

    #[test]
    fn nested_scopes_report_every_overlapping_owner() {
        use super::super::selector;
        let builtin = builtin_sets();
        assert_eq!(
            selector::owning_sets("//quality/tools/javascript:bin"),
            vec![SetId::NpmTools],
            "legacy keeps the most specific arm only"
        );
        let named = resolve_named(&builtin, &strings(&["//quality/tools/javascript:bin"]))
            .expect("named reports overlap");
        assert_eq!(named.len(), 2);
        assert_eq!(named.get("npm-tools"), Some(&SetRequest::Full));
        assert_eq!(named.get("cargo"), Some(&SetRequest::Full));
    }

    #[test]
    fn module_bazel_quirk_stays_legacy_only() {
        use super::super::selector;
        let builtin = builtin_sets();
        let legacy =
            selector::resolve(&strings(&["MODULE.bazel"])).expect("legacy maps the module file");
        assert_eq!(legacy.len(), 14);
        let named = resolve_named(&builtin, &strings(&["MODULE.bazel"])).expect("named maps it");
        assert_eq!(named.len(), 1);
        assert_eq!(named.get("maven"), Some(&SetRequest::Full));
    }

    #[test]
    fn bare_cargo_filename_quirk_stays_legacy_only() {
        use super::super::selector;
        let builtin = builtin_sets();
        assert_eq!(
            selector::owning_sets("Cargo.toml"),
            vec![SetId::Cargo],
            "legacy matches the bare filename case-insensitively"
        );
        assert!(resolve_named(&builtin, &strings(&["Cargo.toml"]))
            .expect_err("named wants an owned path")
            .to_string()
            .contains("Cargo.toml"));
    }
}
