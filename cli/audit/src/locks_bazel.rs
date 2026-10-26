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
    RepoRule,
    Extension,
}

impl BazelSource {
    pub fn as_str(self) -> &'static str {
        match self {
            BazelSource::Registry => "registry",
            BazelSource::ArchiveOverride => "archive_override",
            BazelSource::GitOverride => "git_override",
            BazelSource::LocalOverride => "local_path_override",
            BazelSource::HttpArchive => "http_archive",
            BazelSource::RepoRule => "repo_rule",
            BazelSource::Extension => "module_extension",
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
            is_git: self.source == BazelSource::GitOverride,
            is_private: self.source == BazelSource::LocalOverride,
            location: Some(self.provenance()),
            ..LockedPackage::new(BAZEL_SET, self.name.clone(), self.version.clone())
        }
    }
}

struct Selected {
    name: String,
    version: String,
    registry: String,
    digest: String,
}

/// Reads the selected modules, overrides, and downloads from `MODULE.bazel`, its includes, and the lock.
pub fn parse_bazel_inputs(
    module_text: &str,
    lock_text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<BazelInput>, String> {
    let modules = dx_module_bazel::load_with_includes(module_text, read_include)?;
    let calls: Vec<&Call> = modules.iter().flat_map(ModuleFile::calls).collect();
    let lock: serde_json::Value = serde_json::from_str(lock_text)
        .map_err(|error| format!("invalid MODULE.bazel.lock: {error}"))?;
    let selected = selected_modules(&lock)?;
    let overrides = module_overrides(&calls)?;
    let deps = declared_deps(&calls)?;
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
        let floor = overrides
            .get(name)
            .and_then(|over| over.version.as_deref())
            .unwrap_or(dep_version);
        if !floor.is_empty() && bazel_version_cmp(&best.version, floor) == Ordering::Less {
            return Err(format!(
                "bazel_dep {name} declares {floor} but MODULE.bazel.lock selects {}; refresh the lock with `bazel mod deps`",
                best.version
            ));
        }
    }
    for module in &modules {
        out.extend(download_inputs(module)?);
    }
    out.extend(extension_inputs(&lock)?);
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    for pair in out.windows(2) {
        if pair[0].name == pair[1].name && pair[0].version == pair[1].version {
            return Err(format!(
                "{}@{} is declared twice across MODULE.bazel and MODULE.bazel.lock",
                pair[0].name, pair[0].version
            ));
        }
    }
    Ok(out)
}

fn selected_modules(root: &serde_json::Value) -> Result<Vec<Selected>, String> {
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

fn module_overrides(calls: &[&Call]) -> Result<BTreeMap<String, Override>, String> {
    let mut out: BTreeMap<String, Override> = BTreeMap::new();
    for call in calls.iter().copied() {
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
            name_kind => Override {
                source: BazelSource::Registry,
                version: if name_kind == "single_version_override" {
                    literal(call, "version")?
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                } else {
                    None
                },
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

fn declared_deps(calls: &[&Call]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for call in calls
        .iter()
        .copied()
        .filter(|call| call.name == "bazel_dep")
    {
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
        Some(Value::Str(text)) => urls.push(text.value.clone()),
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
    let label = format!("{} at MODULE.bazel line {}", call.name, call.line);
    checked_digest(
        literal(call, "sha256")?,
        literal(call, "integrity")?,
        &label,
    )?
    .ok_or_else(|| format!("{label} has no sha256 or integrity"))
}

/// Validates an integrity or sha256 value and returns it in provenance form.
fn checked_digest(
    sha256: Option<&str>,
    integrity: Option<&str>,
    label: &str,
) -> Result<Option<String>, String> {
    if let Some(integrity) = integrity.filter(|text| !text.is_empty()) {
        let valid = ["sha256-", "sha384-", "sha512-"]
            .iter()
            .any(|prefix| integrity.starts_with(prefix) && integrity.len() > prefix.len());
        if !valid {
            return Err(format!(
                "{label} has an integrity that is not sha256-, sha384-, or sha512-"
            ));
        }
        return Ok(Some(integrity.to_owned()));
    }
    if let Some(sha) = sha256.filter(|text| !text.is_empty()) {
        if !is_sha256_hex(sha) {
            return Err(format!(
                "{label} has a sha256 that is not 64 lowercase hex digits"
            ));
        }
        return Ok(Some(format!("sha256:{sha}")));
    }
    Ok(None)
}

fn download_inputs(module: &ModuleFile) -> Result<Vec<BazelInput>, String> {
    let mut http: BTreeSet<&str> = BTreeSet::new();
    let mut custom: BTreeMap<&str, (&str, &str)> = BTreeMap::new();
    for call in module.calls_named("use_repo_rule") {
        let (Some(Value::Str(file)), Some(Value::Str(rule))) = (
            call.positional
                .first()
                .or_else(|| call.arg("repo_rule_bzl_file")),
            call.positional
                .get(1)
                .or_else(|| call.arg("repo_rule_name")),
        ) else {
            return Err(format!(
                "use_repo_rule at MODULE.bazel line {} has a computed file or rule name",
                call.line
            ));
        };
        let Some(binding) = call.binding.as_deref() else {
            continue;
        };
        let (file, rule) = (file.value.as_str(), rule.value.as_str());
        if file.starts_with("@bazel_tools//")
            && file.ends_with(HTTP_BZL_SUFFIX)
            && HTTP_RULES.contains(&rule)
        {
            http.insert(binding);
        } else {
            custom.insert(binding, (file, rule));
        }
    }
    let mut out = Vec::new();
    for call in module.calls() {
        if http.contains(call.name.as_str()) {
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
        } else if let Some((file, rule)) = custom.get(call.name.as_str()) {
            out.push(repo_rule_input(call, file, rule)?);
        }
    }
    Ok(out)
}

fn repo_rule_input(call: &Call, file: &str, rule: &str) -> Result<BazelInput, String> {
    let name = required(call, "name")?.to_owned();
    let commit = call.str("commit").filter(|text| !text.is_empty());
    let sha = call.str("sha256").filter(|text| !text.is_empty());
    let digest = match (commit, sha) {
        (Some(commit), _) => Some(format!("commit:{commit}")),
        (None, Some(sha)) => Some(format!("sha256:{sha}")),
        (None, None) => None,
    };
    let version = digest
        .clone()
        .or_else(|| call.str("tag").map(str::to_owned))
        .unwrap_or_else(|| "unpinned".to_owned());
    let origin = call
        .str("remote")
        .or_else(|| call.str("url"))
        .or_else(|| call.str_list("urls").first().copied())
        .map_or_else(|| format!("{file} {rule}"), str::to_owned);
    Ok(BazelInput {
        name,
        version,
        source: BazelSource::RepoRule,
        origin,
        digest,
        patched: !call.str_list("patches").is_empty(),
    })
}

/// Lists the downloaded repos that module extensions record in `MODULE.bazel.lock`.
fn extension_inputs(root: &serde_json::Value) -> Result<Vec<BazelInput>, String> {
    let Some(extensions) = root.get("moduleExtensions") else {
        return Ok(Vec::new());
    };
    let extensions = lock_object(extensions, "moduleExtensions")?;
    let mut found: BTreeMap<(String, String), (String, BazelInput)> = BTreeMap::new();
    for (extension, platforms) in extensions {
        for (platform, entry) in lock_object(platforms, extension)? {
            let place = format!("{extension} {platform}");
            let Some(specs) = lock_object(entry, &place)?.get("generatedRepoSpecs") else {
                continue;
            };
            for (repo, spec) in lock_object(specs, &format!("{place} generatedRepoSpecs"))? {
                let Some(input) = extension_input(extension, repo, spec)? else {
                    continue;
                };
                let key = (input.name.clone(), input.version.clone());
                match found.get(&key) {
                    Some((_, seen)) if seen.origin == input.origin => {}
                    Some((other, seen)) => {
                        return Err(format!(
                            "invalid MODULE.bazel.lock: extensions {other} and {extension} both generate repo {repo} at {} from {} and {}",
                            input.version, seen.origin, input.origin
                        ));
                    }
                    None => {
                        found.insert(key, (extension.clone(), input));
                    }
                }
            }
        }
    }
    Ok(found.into_values().map(|(_, input)| input).collect())
}

/// Builds the input for one generated repo spec, or None when it downloads nothing.
fn extension_input(
    extension: &str,
    repo: &str,
    spec: &serde_json::Value,
) -> Result<Option<BazelInput>, String> {
    let label = format!("{extension} {repo}");
    let spec = lock_object(spec, &label)?;
    let has_rule = lock_str(spec, "repoRuleId", &label)?.is_some()
        || (lock_str(spec, "bzlFile", &label)?.is_some()
            && lock_str(spec, "ruleClassName", &label)?.is_some());
    if !has_rule {
        return Err(format!(
            "invalid MODULE.bazel.lock: {label} has no repo rule"
        ));
    }
    let empty = serde_json::Map::new();
    let attributes = match spec.get("attributes") {
        Some(value) => lock_object(value, &format!("{label} attributes"))?,
        None => &empty,
    };
    let origin = match attributes.get("urls") {
        Some(serde_json::Value::Array(items)) => {
            let mut first = None;
            for item in items {
                let Some(text) = item.as_str() else {
                    return Err(format!(
                        "invalid MODULE.bazel.lock: {label} has a url that is not a string"
                    ));
                };
                if first.is_none() && !text.is_empty() {
                    first = Some(text);
                }
            }
            first
        }
        _ => lock_str(attributes, "urls", &label)?,
    };
    let origin = match origin {
        Some(text) => Some(text),
        None => lock_str(attributes, "url", &label)?,
    };
    let origin = match origin {
        Some(text) => Some(text),
        None => lock_str(attributes, "remote", &label)?,
    };
    let Some(origin) = origin else {
        return Ok(None);
    };
    let digest = match checked_digest(
        lock_str(attributes, "sha256", &label)?,
        lock_str(attributes, "integrity", &label)?,
        &format!("invalid MODULE.bazel.lock: {label}"),
    )? {
        Some(digest) => Some(digest),
        None => lock_str(attributes, "commit", &label)?.map(|commit| format!("commit:{commit}")),
    };
    let patched = ["patches", "remote_patches"].iter().any(|key| {
        attributes.get(*key).is_some_and(|value| match value {
            serde_json::Value::Array(items) => !items.is_empty(),
            serde_json::Value::Object(items) => !items.is_empty(),
            _ => false,
        })
    });
    Ok(Some(BazelInput {
        name: repo.to_owned(),
        version: digest.clone().unwrap_or_else(|| "unpinned".to_owned()),
        source: BazelSource::Extension,
        origin: origin.to_owned(),
        digest,
        patched,
    }))
}

/// Returns the value as a JSON object or names the lock path that is not one.
fn lock_object<'a>(
    value: &'a serde_json::Value,
    place: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("invalid MODULE.bazel.lock: {place} is not an object"))
}

/// Reads an optional non-empty string field, failing on any other JSON type.
fn lock_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<Option<&'a str>, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(text)) => Ok(Some(text.as_str()).filter(|t| !t.is_empty())),
        Some(_) => Err(format!(
            "invalid MODULE.bazel.lock: {label} has a {key} that is not a string"
        )),
    }
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
