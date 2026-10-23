use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{Call, ModuleFile, Value};

use crate::vuln::LockedPackage;

pub const BAZEL_SET: &str = "bazel";

const HTTP_BZL_SUFFIX: &str = "repo:http.bzl";
const HTTP_RULES: &[&str] = &["http_archive", "http_file", "http_jar"];
const OVERRIDES: &[&str] = &[
    "archive_override",
    "git_override",
    "local_path_override",
    "single_version_override",
    "multiple_version_override",
];

/// Where one selected Bazel input comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BazelSource {
    Registry,
    ArchiveOverride,
    GitOverride,
    LocalOverride,
    HttpArchive,
}

impl BazelSource {
    pub fn as_str(self) -> &'static str {
        match self {
            BazelSource::Registry => "registry",
            BazelSource::ArchiveOverride => "archive_override",
            BazelSource::GitOverride => "git_override",
            BazelSource::LocalOverride => "local_path_override",
            BazelSource::HttpArchive => "http_archive",
        }
    }
}

/// One selected module, overridden module, or downloaded archive with its origin facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BazelInput {
    pub name: String,
    pub version: String,
    pub source: BazelSource,
    pub origin: String,
    pub digest: Option<String>,
    pub patched: bool,
}

impl BazelInput {
    /// The origin, digest, and patch facts as one line.
    pub fn provenance(&self) -> String {
        let mut text = format!("{} {}", self.source.as_str(), self.origin);
        if let Some(digest) = &self.digest {
            text.push(' ');
            text.push_str(digest);
        }
        if self.patched {
            text.push_str(" patched");
        }
        text
    }

    pub fn locked(&self) -> LockedPackage {
        LockedPackage {
            name: self.name.clone(),
            version: self.version.clone(),
            set: BAZEL_SET.to_owned(),
            is_git: self.source == BazelSource::GitOverride,
            is_private: self.source == BazelSource::LocalOverride,
            location: Some(self.provenance()),
            platform: None,
        }
    }
}

struct Selected {
    name: String,
    version: String,
    registry: String,
    digest: String,
}

/// Reads the selected modules, overrides, and archives from `MODULE.bazel` and `MODULE.bazel.lock`.
pub fn parse_bazel_inputs(module_text: &str, lock_text: &str) -> Result<Vec<BazelInput>, String> {
    let module = ModuleFile::parse(module_text).map_err(|error| error.to_string())?;
    let selected = selected_modules(lock_text)?;
    let overrides = module_overrides(&module)?;
    let deps = declared_deps(&module)?;
    let mut out: Vec<BazelInput> = Vec::new();
    for pick in &selected {
        if let Some(over) = overrides.get(&pick.name) {
            if over.source != BazelSource::Registry {
                return Err(format!(
                    "MODULE.bazel.lock selects {}@{} from a registry but MODULE.bazel overrides it with {}; refresh the lock with `bazel mod deps`",
                    pick.name,
                    pick.version,
                    over.source.as_str()
                ));
            }
        }
        out.push(BazelInput {
            name: pick.name.clone(),
            version: pick.version.clone(),
            source: BazelSource::Registry,
            origin: pick.registry.clone(),
            digest: Some(format!("source.json sha256:{}", pick.digest)),
            patched: overrides.get(&pick.name).is_some_and(|over| over.patched),
        });
    }
    for (name, over) in &overrides {
        if over.source == BazelSource::Registry {
            continue;
        }
        let version = match (deps.get(name), over.version.as_deref()) {
            (_, Some(version)) => version.to_owned(),
            (Some(dep), None) if !dep.is_empty() => dep.clone(),
            _ => over.digest.clone().unwrap_or_else(|| over.origin.clone()),
        };
        out.push(BazelInput {
            name: name.clone(),
            version,
            source: over.source,
            origin: over.origin.clone(),
            digest: over.digest.clone(),
            patched: over.patched,
        });
    }
    for (name, dep_version) in &deps {
        if overrides
            .get(name)
            .is_some_and(|over| over.source != BazelSource::Registry)
        {
            continue;
        }
        let picks: Vec<&Selected> = selected.iter().filter(|pick| &pick.name == name).collect();
        let Some(best) = picks
            .iter()
            .max_by(|left, right| bazel_version_cmp(&left.version, &right.version))
        else {
            return Err(format!(
                "bazel_dep {name} is not selected in MODULE.bazel.lock; refresh the lock with `bazel mod deps`"
            ));
        };
        if !dep_version.is_empty()
            && bazel_version_cmp(&best.version, dep_version) == Ordering::Less
        {
            return Err(format!(
                "bazel_dep {name} declares {dep_version} but MODULE.bazel.lock selects {}; refresh the lock with `bazel mod deps`",
                best.version
            ));
        }
    }
    out.extend(http_inputs(&module)?);
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    for pair in out.windows(2) {
        if pair[0].name == pair[1].name && pair[0].version == pair[1].version {
            return Err(format!(
                "{}@{} is declared twice in MODULE.bazel",
                pair[0].name, pair[0].version
            ));
        }
    }
    Ok(out)
}

/// Parses the inputs and returns them as lock packages.
pub fn parse_bazel_modules(
    module_text: &str,
    lock_text: &str,
) -> Result<Vec<LockedPackage>, String> {
    Ok(parse_bazel_inputs(module_text, lock_text)?
        .iter()
        .map(BazelInput::locked)
        .collect())
}

fn selected_modules(lock_text: &str) -> Result<Vec<Selected>, String> {
    let root: serde_json::Value = serde_json::from_str(lock_text)
        .map_err(|error| format!("invalid MODULE.bazel.lock: {error}"))?;
    if root
        .get("lockFileVersion")
        .and_then(|v| v.as_u64())
        .is_none()
    {
        return Err("invalid MODULE.bazel.lock: missing lockFileVersion".to_owned());
    }
    let Some(hashes) = root.get("registryFileHashes").and_then(|v| v.as_object()) else {
        return Err("invalid MODULE.bazel.lock: missing registryFileHashes".to_owned());
    };
    let mut out = Vec::new();
    for (url, hash) in hashes {
        if !url.ends_with("/source.json") {
            continue;
        }
        let Some((registry, rest)) = url.split_once("/modules/") else {
            return Err(format!(
                "invalid MODULE.bazel.lock: {url} is not a registry module source"
            ));
        };
        let parts: Vec<&str> = rest.split('/').collect();
        let [name, version, "source.json"] = parts.as_slice() else {
            return Err(format!(
                "invalid MODULE.bazel.lock: {url} is not <registry>/modules/<name>/<version>/source.json"
            ));
        };
        if name.is_empty() || version.is_empty() {
            return Err(format!(
                "invalid MODULE.bazel.lock: {url} has an empty module name or version"
            ));
        }
        let digest = hash
            .as_str()
            .filter(|text| is_sha256_hex(text))
            .ok_or_else(|| format!("invalid MODULE.bazel.lock: {url} has no sha256 digest"))?;
        out.push(Selected {
            name: (*name).to_owned(),
            version: (*version).to_owned(),
            registry: registry.to_owned(),
            digest: digest.to_owned(),
        });
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

struct Override {
    source: BazelSource,
    version: Option<String>,
    origin: String,
    digest: Option<String>,
    patched: bool,
}

fn literal<'a>(call: &'a Call, key: &str) -> Result<Option<&'a str>, String> {
    match call.arg(key) {
        None => Ok(None),
        Some(Value::Str(text)) => Ok(Some(text.value.as_str())),
        Some(_) => Err(format!(
            "{} at MODULE.bazel line {} has a computed {key}; write it as a string literal",
            call.name, call.line
        )),
    }
}

fn required<'a>(call: &'a Call, key: &str) -> Result<&'a str, String> {
    literal(call, key)?
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            format!(
                "{} at MODULE.bazel line {} has no {key}",
                call.name, call.line
            )
        })
}

fn module_overrides(module: &ModuleFile) -> Result<BTreeMap<String, Override>, String> {
    let mut out: BTreeMap<String, Override> = BTreeMap::new();
    for call in module.calls() {
        if !OVERRIDES.contains(&call.name.as_str()) {
            continue;
        }
        let name = required(call, "module_name")?.to_owned();
        let patched = !call.str_list("patches").is_empty();
        let over = match call.name.as_str() {
            "archive_override" => {
                let urls = urls_of(call)?;
                Override {
                    source: BazelSource::ArchiveOverride,
                    version: None,
                    origin: urls[0].clone(),
                    digest: Some(digest_of(call)?),
                    patched,
                }
            }
            "git_override" => {
                let commit = literal(call, "commit")?.unwrap_or_default();
                if commit.is_empty() {
                    return Err(format!(
                        "git_override for {name} at MODULE.bazel line {} is not pinned to a commit",
                        call.line
                    ));
                }
                Override {
                    source: BazelSource::GitOverride,
                    version: Some(commit.to_owned()),
                    origin: required(call, "remote")?.to_owned(),
                    digest: Some(format!("commit:{commit}")),
                    patched,
                }
            }
            "local_path_override" => Override {
                source: BazelSource::LocalOverride,
                version: None,
                origin: required(call, "path")?.to_owned(),
                digest: None,
                patched: false,
            },
            _ => Override {
                source: BazelSource::Registry,
                version: None,
                origin: String::new(),
                digest: None,
                patched,
            },
        };
        if out.insert(name.clone(), over).is_some() {
            return Err(format!(
                "MODULE.bazel overrides module {name} more than once"
            ));
        }
    }
    Ok(out)
}

fn declared_deps(module: &ModuleFile) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for call in module.calls_named("bazel_dep") {
        let name = required(call, "name")?.to_owned();
        let version = literal(call, "version")?.unwrap_or_default().to_owned();
        if out.insert(name.clone(), version).is_some() {
            return Err(format!("MODULE.bazel declares bazel_dep {name} twice"));
        }
    }
    Ok(out)
}

fn urls_of(call: &Call) -> Result<Vec<String>, String> {
    let mut urls: Vec<String> = Vec::new();
    match call.arg("urls") {
        Some(Value::List(items)) => {
            for item in items {
                match item {
                    Value::Str(text) => urls.push(text.value.clone()),
                    _ => {
                        return Err(format!(
                            "{} at MODULE.bazel line {} has a computed url",
                            call.name, call.line
                        ))
                    }
                }
            }
        }
        Some(_) => {
            return Err(format!(
                "{} at MODULE.bazel line {} has computed urls",
                call.name, call.line
            ))
        }
        None => {
            if let Some(url) = literal(call, "url")? {
                urls.push(url.to_owned());
            }
        }
    }
    if urls.is_empty() {
        return Err(format!(
            "{} at MODULE.bazel line {} has no urls",
            call.name, call.line
        ));
    }
    Ok(urls)
}

fn digest_of(call: &Call) -> Result<String, String> {
    if let Some(integrity) = literal(call, "integrity")?.filter(|text| !text.is_empty()) {
        let valid = ["sha256-", "sha384-", "sha512-"]
            .iter()
            .any(|prefix| integrity.starts_with(prefix) && integrity.len() > prefix.len());
        if !valid {
            return Err(format!(
                "{} at MODULE.bazel line {} has an integrity that is not sha256-, sha384-, or sha512-",
                call.name, call.line
            ));
        }
        return Ok(integrity.to_owned());
    }
    if let Some(sha) = literal(call, "sha256")?.filter(|text| !text.is_empty()) {
        if !is_sha256_hex(sha) {
            return Err(format!(
                "{} at MODULE.bazel line {} has a sha256 that is not 64 lowercase hex digits",
                call.name, call.line
            ));
        }
        return Ok(format!("sha256:{sha}"));
    }
    Err(format!(
        "{} at MODULE.bazel line {} has no sha256 or integrity",
        call.name, call.line
    ))
}

fn http_inputs(module: &ModuleFile) -> Result<Vec<BazelInput>, String> {
    let mut rules: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_repo_rule") {
        let file = match call
            .positional
            .first()
            .or_else(|| call.arg("repo_rule_bzl_file"))
        {
            Some(Value::Str(text)) => text.value.as_str(),
            _ => continue,
        };
        let rule = match call
            .positional
            .get(1)
            .or_else(|| call.arg("repo_rule_name"))
        {
            Some(Value::Str(text)) => text.value.as_str(),
            _ => continue,
        };
        if file.starts_with("@bazel_tools//")
            && file.ends_with(HTTP_BZL_SUFFIX)
            && HTTP_RULES.contains(&rule)
        {
            if let Some(binding) = call.binding.as_deref() {
                rules.insert(binding);
            }
        }
    }
    let mut out = Vec::new();
    for call in module.calls() {
        if !rules.contains(call.name.as_str()) {
            continue;
        }
        let name = required(call, "name")?.to_owned();
        let urls = urls_of(call)?;
        let digest = digest_of(call)?;
        out.push(BazelInput {
            name,
            version: digest.clone(),
            source: BazelSource::HttpArchive,
            origin: urls[0].clone(),
            digest: Some(digest),
            patched: !call.str_list("patches").is_empty(),
        });
    }
    Ok(out)
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Orders two Bazel module versions the way Bazel does.
pub fn bazel_version_cmp(left: &str, right: &str) -> Ordering {
    fn split(version: &str) -> (&str, &str) {
        let main = version.split('+').next().unwrap_or_default();
        match main.split_once('-') {
            Some((release, pre)) => (release, pre),
            None => (main, ""),
        }
    }
    let (left_release, left_pre) = split(left);
    let (right_release, right_pre) = split(right);
    match (left_release.is_empty(), right_release.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    compare_identifiers(left_release, right_release).then_with(|| {
        match (left_pre.is_empty(), right_pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => compare_identifiers(left_pre, right_pre),
        }
    })
}

fn compare_identifiers(left: &str, right: &str) -> Ordering {
    let mut l = left.split('.');
    let mut r = right.split('.');
    loop {
        match (l.next(), r.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let ordering = match (a.parse::<u128>(), b.parse::<u128>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "locks_bazel_tests.rs"]
mod locks_bazel_tests;
