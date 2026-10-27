use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{Call, ModuleFile, Value};

use super::rule_graph::apparent_repo_matches;

/// Changed crate names with every base and head version that changed.
pub(crate) type ChangedCrates = BTreeMap<String, BTreeSet<String>>;

/// What a lock diff changed: crates plus hub alias names (renames, extra
/// aliased targets, binaries) that cannot be derived from crate names alone.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LockChange {
    pub(crate) crates: ChangedCrates,
    pub(crate) aliases: BTreeSet<String>,
    /// The names are Python packages of a `pip.parse` hub, not crates.
    pub(crate) pip: bool,
    /// The names are Maven artifacts of a `maven.install` hub.
    pub(crate) maven: bool,
    /// Gazelle repository names of the changed Go modules.
    pub(crate) go_repos: BTreeSet<String>,
    /// Whether the names are npm packages of an `npm_translate_lock` hub.
    pub(crate) npm: bool,
}

/// One `crate.from_cargo` hub with the lock files it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Hub {
    pub(crate) name: String,
    pub(crate) cargo_lockfile: String,
    pub(crate) lockfile: Option<String>,
    /// A `pip.parse` hub; `cargo_lockfile` then holds a requirements file.
    pub(crate) pip: bool,
    /// A `maven.install` hub; `cargo_lockfile` then holds `lock_file`.
    pub(crate) maven: bool,
    /// A `go_deps.from_file` hub: `cargo_lockfile` is its go.mod, `lockfile` its go.sum.
    pub(crate) go: bool,
    /// The `pnpm_lock` of an `npm.npm_translate_lock` hub, which has no Cargo
    /// lock (`cargo_lockfile` is empty).
    pub(crate) pnpm_lockfile: Option<String>,
    /// A `ruby.bundle_fetch` repo; `cargo_lockfile` then holds its Gemfile.lock.
    pub(crate) gems: bool,
}

/// The lock format of one changed path and the hubs it feeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LockKind {
    Cargo(Vec<String>),
    CrateBazelLock(Vec<String>),
    Pip(Vec<String>),
    Maven(Vec<String>),
    /// A go.mod or go.sum named by `go_deps.from_file`.
    Go(Vec<String>),
    Pnpm(Vec<String>),
    /// A Gemfile.lock named by `ruby.bundle_fetch`.
    Gemfile(Vec<String>),
    Unknown,
}

impl LockKind {
    pub(crate) fn hubs(&self) -> &[String] {
        match self {
            LockKind::Cargo(hubs)
            | LockKind::CrateBazelLock(hubs)
            | LockKind::Pip(hubs)
            | LockKind::Maven(hubs)
            | LockKind::Go(hubs)
            | LockKind::Pnpm(hubs)
            | LockKind::Gemfile(hubs) => hubs,
            LockKind::Unknown => &[],
        }
    }

    pub(crate) fn changed(&self, base: &str, head: &str) -> Result<LockChange, String> {
        match self {
            LockKind::Cargo(_) => Ok(LockChange {
                crates: changed_crates(base, head)?,
                aliases: BTreeSet::new(),
                pip: false,
                maven: false,
                go_repos: BTreeSet::new(),
                npm: false,
            }),
            LockKind::Pip(_) => Ok(LockChange {
                crates: changed_requirements(base, head),
                aliases: BTreeSet::new(),
                pip: true,
                maven: false,
                go_repos: BTreeSet::new(),
                npm: false,
            }),
            LockKind::Maven(_) => Ok(LockChange {
                crates: changed_maven_artifacts(base, head)?,
                aliases: BTreeSet::new(),
                pip: false,
                maven: true,
                go_repos: BTreeSet::new(),
                npm: false,
            }),
            LockKind::CrateBazelLock(_) => changed_crates_from_bazel_lock(base, head),
            LockKind::Go(_) => Err("use changed_go".to_owned()),
            LockKind::Pnpm(_) => changed_npm_packages(base, head),
            LockKind::Gemfile(_) => changed_gems(base, head),
            LockKind::Unknown => Err("unknown lock format".to_owned()),
        }
    }
}

pub(crate) const CRATE_EXTENSION_BZL: &str = "crate_universe:extensions.bzl";
const DEFAULT_HUB: &str = "crates";
const NPM_EXTENSION_BZL: &str = "npm:extensions.bzl";

type Key = (String, String);

#[derive(Debug, PartialEq, Eq)]
struct Node {
    content: String,
    deps: Vec<Key>,
}

type Nodes = BTreeMap<Key, Node>;

fn literal(call: &Call, key: &str, index: Option<usize>) -> Result<Option<String>, String> {
    let value = index
        .and_then(|index| call.positional.get(index))
        .or_else(|| call.arg(key));
    match value {
        None => Ok(None),
        Some(Value::Str(literal)) => Ok(Some(literal.value.clone())),
        Some(_) => Err(format!(
            "{} at MODULE.bazel line {} has a computed {key}",
            call.name, call.line
        )),
    }
}

fn workspace_path(label: &str) -> Result<String, String> {
    let rest = if let Some(relative) = label.strip_prefix(':') {
        relative
    } else {
        let local = label
            .strip_prefix("@@")
            .or_else(|| label.strip_prefix('@'))
            .unwrap_or(label);
        local
            .strip_prefix("//")
            .ok_or_else(|| format!("label {label} is not in the main repository"))?
    };
    let (package, name) = if label.starts_with(':') {
        ("", rest)
    } else {
        rest.split_once(':')
            .ok_or_else(|| format!("label {label} names no file"))?
    };
    if name.is_empty() {
        return Err(format!("label {label} names no file"));
    }
    if package.is_empty() {
        Ok(name.to_owned())
    } else {
        Ok(format!("{package}/{name}"))
    }
}

pub(crate) fn crate_hubs(module_text: &str) -> Result<Vec<Hub>, String> {
    let module = ModuleFile::parse(module_text).map_err(|error| error.to_string())?;
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    let mut npm_bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        if call.unpacked {
            return Err(format!(
                "use_extension at MODULE.bazel line {} unpacks its arguments",
                call.line
            ));
        }
        let bzl = literal(call, "extension_bzl_file", Some(0))?.unwrap_or_default();
        let name = literal(call, "extension_name", Some(1))?.unwrap_or_default();
        if bzl.ends_with(CRATE_EXTENSION_BZL) && name == "crate" {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
        if bzl.ends_with(NPM_EXTENSION_BZL) && name == "npm" {
            if let Some(binding) = &call.binding {
                npm_bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        if let Some((binding, "npm_translate_lock")) = call.name.split_once('.') {
            if npm_bindings.contains(binding) {
                if call.unpacked {
                    return Err(format!(
                        "{} at MODULE.bazel line {} unpacks its arguments",
                        call.name, call.line
                    ));
                }
                let name = literal(call, "name", Some(0))?.ok_or_else(|| {
                    format!(
                        "{} at MODULE.bazel line {} has no name",
                        call.name, call.line
                    )
                })?;
                let pnpm_lock = literal(call, "pnpm_lock", None)?.ok_or_else(|| {
                    format!(
                        "{} at MODULE.bazel line {} has no pnpm_lock",
                        call.name, call.line
                    )
                })?;
                hubs.push(Hub {
                    name,
                    cargo_lockfile: String::new(),
                    lockfile: None,
                    pip: false,
                    maven: false,
                    go: false,
                    gems: false,
                    pnpm_lockfile: Some(workspace_path(&pnpm_lock)?),
                });
            }
            continue;
        }
        let Some((binding, "from_cargo")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) {
            continue;
        }
        if call.unpacked {
            return Err(format!(
                "{} at MODULE.bazel line {} unpacks its arguments",
                call.name, call.line
            ));
        }
        let name = literal(call, "name", None)?.unwrap_or_else(|| DEFAULT_HUB.to_owned());
        let cargo_lockfile = literal(call, "cargo_lockfile", None)?.ok_or_else(|| {
            format!(
                "{} at MODULE.bazel line {} has no cargo_lockfile",
                call.name, call.line
            )
        })?;
        let lockfile = literal(call, "lockfile", None)?
            .map(|label| workspace_path(&label))
            .transpose()?;
        hubs.push(Hub {
            name,
            cargo_lockfile: workspace_path(&cargo_lockfile)?,
            lockfile,
            pip: false,
            maven: false,
            go: false,
            gems: false,
            pnpm_lockfile: None,
        });
    }
    hubs.extend(go_hubs(&module));
    Ok(hubs)
}

/// `go_deps.from_file` calls with a literal go.mod label; anything computed is
/// skipped, which leaves its files as workspace inputs.
fn go_hubs(module: &ModuleFile) -> Vec<Hub> {
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        let bzl = literal(call, "extension_bzl_file", Some(0)).ok().flatten();
        let name = literal(call, "extension_name", Some(1)).ok().flatten();
        if bzl.is_some_and(|bzl| bzl.ends_with("extensions.bzl"))
            && name.as_deref() == Some("go_deps")
        {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        let Some((binding, "from_file")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) || call.unpacked {
            continue;
        }
        let Some(go_mod) = literal(call, "go_mod", None)
            .ok()
            .flatten()
            .and_then(|label| workspace_path(&label).ok())
        else {
            continue;
        };
        let dir = go_mod.strip_suffix("go.mod").unwrap_or("");
        if dir.len() + "go.mod".len() != go_mod.len() {
            continue;
        }
        hubs.push(Hub {
            name: "go_deps".to_owned(),
            lockfile: Some(format!("{dir}go.sum")),
            cargo_lockfile: go_mod,
            pip: false,
            maven: false,
            go: true,
            gems: false,
            pnpm_lockfile: None,
        });
    }
    hubs
}

/// Gazelle's repository name for a Go module path.
pub(crate) fn go_repo_name(module: &str) -> String {
    let lower = module.to_lowercase();
    let mut parts = lower.split('/');
    let host = parts.next().unwrap_or_default();
    let mut names: Vec<&str> = host.split('.').rev().collect();
    names.extend(parts);
    names.join("_").replace(['-', '.'], "_")
}

/// The Gazelle repositories of the modules a go.mod or go.sum edit changes, or
/// an error when the edit is more than requirement or checksum lines.
pub(crate) fn changed_go(path: &str, base: &str, head: &str) -> Result<LockChange, String> {
    let sum = path.ends_with("go.sum");
    let parse = |text: &str| {
        let mut modules: BTreeSet<(String, String)> = BTreeSet::new();
        let mut other: BTreeSet<String> = BTreeSet::new();
        for line in text.lines() {
            let line = line.split("//").next().unwrap_or_default().trim();
            let line = line.strip_prefix("require ").unwrap_or(line).trim();
            let words: Vec<&str> = line.split_whitespace().collect();
            let is_module = if sum {
                words.len() == 3
            } else {
                words.len() == 2 && words[1].starts_with('v') && words[0] != "go"
            };
            if is_module {
                modules.insert((words[0].to_owned(), words[1..].join(" ")));
            } else if !sum && !line.is_empty() && line != "require (" && line != ")" {
                other.insert(line.to_owned());
            }
        }
        (modules, other)
    };
    let (base_modules, base_other) = parse(base);
    let (head_modules, head_other) = parse(head);
    if base_other != head_other {
        return Err("go.mod changed beyond its requirements".to_owned());
    }
    let go_repos = base_modules
        .symmetric_difference(&head_modules)
        .map(|(module, _)| go_repo_name(module))
        .collect();
    Ok(LockChange {
        crates: ChangedCrates::new(),
        aliases: BTreeSet::new(),
        pip: false,
        maven: false,
        go_repos,
        npm: false,
    })
}

/// Labels inside the changed Go module repositories.
pub(crate) fn go_seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    repos: &BTreeSet<String>,
) -> Vec<String> {
    let mut out: Vec<String> = labels
        .filter(|label| {
            label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
                .is_some_and(|(repo, _)| repos.iter().any(|name| apparent_repo_matches(repo, name)))
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn closure(base: &Nodes, head: &Nodes) -> ChangedCrates {
    let mut changed: BTreeSet<&Key> = BTreeSet::new();
    for (key, node) in base {
        if head.get(key) != Some(node) {
            changed.insert(key);
        }
    }
    changed.extend(head.keys().filter(|key| !base.contains_key(*key)));
    loop {
        let before = changed.len();
        for nodes in [base, head] {
            for (key, node) in nodes {
                if !changed.contains(key) && node.deps.iter().any(|dep| changed.contains(dep)) {
                    changed.insert(key);
                }
            }
        }
        if changed.len() == before {
            break;
        }
    }
    let mut out = ChangedCrates::new();
    for (name, version) in changed {
        out.entry(name.clone()).or_default().insert(version.clone());
    }
    out
}

fn cargo_nodes(text: &str) -> Result<Nodes, String> {
    let mut nodes = Nodes::new();
    for entry in dx_audit::locks::cargo_lock_graph(text)? {
        let content = format!(
            "{:?} {:?} {:?}",
            entry.source, entry.checksum, entry.dependencies
        );
        nodes.insert(
            (entry.name, entry.version),
            Node {
                content,
                deps: entry.dependencies,
            },
        );
    }
    Ok(nodes)
}

pub(crate) fn changed_crates(base: &str, head: &str) -> Result<ChangedCrates, String> {
    Ok(closure(&cargo_nodes(base)?, &cargo_nodes(head)?))
}

fn crate_key(id: &str) -> Result<Key, String> {
    id.split_once(' ')
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .ok_or_else(|| format!("crate id {id:?} has no version"))
}

fn collect_ids<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                match (key.as_str(), item.as_str()) {
                    ("id", Some(id)) => out.push(id),
                    _ => collect_ids(item, out),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, out);
            }
        }
        _ => {}
    }
}

fn bazel_lock_nodes(lock: &serde_json::Map<String, serde_json::Value>) -> Result<Nodes, String> {
    let Some(serde_json::Value::Object(crates)) = lock.get("crates") else {
        return Err("crate_universe lockfile has no crates object".to_owned());
    };
    let mut nodes = Nodes::new();
    for (id, entry) in crates {
        let mut ids = Vec::new();
        collect_ids(entry, &mut ids);
        let mut deps = ids
            .into_iter()
            .map(crate_key)
            .collect::<Result<Vec<Key>, String>>()?;
        deps.sort();
        deps.dedup();
        nodes.insert(
            crate_key(id)?,
            Node {
                content: entry.to_string(),
                deps,
            },
        );
    }
    Ok(nodes)
}

fn json_object(text: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match serde_json::from_str(text) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err("crate_universe lockfile is not a JSON object".to_owned()),
        Err(error) => Err(format!("invalid crate_universe lockfile: {error}")),
    }
}

/// Top-level keys that only list ids already present under `crates`; a change
/// there always shows up as a `crates` change too, so it need not widen.
const ID_LIST_KEYS: [&str; 4] = [
    "direct_deps",
    "direct_dev_deps",
    "workspace_members",
    "binary_crates",
];

fn id_set(lock: &serde_json::Map<String, serde_json::Value>, key: &str) -> BTreeSet<String> {
    match lock.get(key) {
        Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn collect_aliases<'a>(value: &'a serde_json::Value, out: &mut Vec<(&'a str, &'a str)>) {
    match value {
        serde_json::Value::Object(map) => {
            if let (Some(id), Some(alias)) = (
                map.get("id").and_then(serde_json::Value::as_str),
                map.get("alias").and_then(serde_json::Value::as_str),
            ) {
                out.push((id, alias));
            }
            for item in map.values() {
                collect_aliases(item, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_aliases(item, out);
            }
        }
        _ => {}
    }
}

/// Hub alias names rules_rust renders besides the plain crate names, each with
/// the crates it points at.
fn hub_aliases(
    lock: &serde_json::Map<String, serde_json::Value>,
) -> Result<BTreeMap<String, BTreeSet<Key>>, String> {
    let mut out: BTreeMap<String, BTreeSet<Key>> = BTreeMap::new();
    let Some(serde_json::Value::Object(crates)) = lock.get("crates") else {
        return Err("crate_universe lockfile has no crates object".to_owned());
    };
    let members = id_set(lock, "workspace_members");
    let binaries = id_set(lock, "binary_crates");
    for (id, entry) in crates {
        let key = crate_key(id)?;
        if members.contains(id) {
            let mut found = Vec::new();
            collect_aliases(entry, &mut found);
            for (dep, alias) in found {
                let dep = crate_key(dep)?;
                out.entry(format!("{alias}-{}", dep.1))
                    .or_default()
                    .insert(dep.clone());
                out.entry(alias.to_owned()).or_default().insert(dep);
            }
        }
        if let Some(serde_json::Value::Object(extra)) = entry.get("extra_aliased_targets") {
            for name in extra.keys() {
                out.entry(name.clone()).or_default().insert(key.clone());
            }
        }
        if binaries.contains(id) {
            if let Some(serde_json::Value::Array(targets)) = entry.get("targets") {
                for bin in targets
                    .iter()
                    .filter_map(|target| target.get("Binary")?.get("crate_name")?.as_str())
                {
                    out.entry(format!("{}__{bin}", key.0))
                        .or_default()
                        .insert(key.clone());
                    out.entry(format!("{}-{}__{bin}", key.0, key.1))
                        .or_default()
                        .insert(key.clone());
                }
            }
        }
    }
    Ok(out)
}

pub(crate) fn changed_crates_from_bazel_lock(base: &str, head: &str) -> Result<LockChange, String> {
    let base = json_object(base)?;
    let head = json_object(head)?;
    let keys: BTreeSet<&String> = base.keys().chain(head.keys()).collect();
    for key in keys {
        if key != "checksum"
            && key != "crates"
            && !ID_LIST_KEYS.contains(&key.as_str())
            && base.get(key) != head.get(key)
        {
            return Err(format!("crate_universe lockfile changed {key}"));
        }
    }
    let mut crates = closure(&bazel_lock_nodes(&base)?, &bazel_lock_nodes(&head)?);
    let (base_aliases, head_aliases) = (hub_aliases(&base)?, hub_aliases(&head)?);
    let in_closure = |key: &Key| crates.get(&key.0).is_some_and(|v| v.contains(&key.1));
    let names: BTreeSet<&String> = base_aliases.keys().chain(head_aliases.keys()).collect();
    let aliases = names
        .into_iter()
        .filter(|name| {
            let (b, h) = (base_aliases.get(*name), head_aliases.get(*name));
            b != h || b.into_iter().chain(h).flatten().any(in_closure)
        })
        .cloned()
        .collect();
    // Members are never hub aliases or spokes; they only matter as graph nodes.
    for member in id_set(&base, "workspace_members")
        .into_iter()
        .chain(id_set(&head, "workspace_members"))
    {
        let (name, version) = crate_key(&member)?;
        if let Some(versions) = crates.get_mut(&name) {
            versions.remove(&version);
            if versions.is_empty() {
                crates.remove(&name);
            }
        }
    }
    Ok(LockChange {
        crates,
        aliases,
        pip: false,
        maven: false,
        go_repos: BTreeSet::new(),
        npm: false,
    })
}

pub(crate) fn seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    hub: &str,
    crates: &ChangedCrates,
    aliases: &BTreeSet<String>,
) -> Vec<String> {
    let mut names: BTreeSet<String> = aliases.clone();
    let mut prefixes: Vec<String> = Vec::new();
    let mut spokes: BTreeSet<String> = BTreeSet::new();
    for (name, versions) in crates {
        names.insert(name.clone());
        prefixes.push(format!("{name}__"));
        for version in versions {
            names.insert(format!("{name}-{version}"));
            prefixes.push(format!("{name}-{version}__"));
            spokes.insert(format!("{hub}__{name}-{version}"));
        }
    }
    let mut out: Vec<String> = labels
        .filter(|label| {
            let Some((repo, target)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            if apparent_repo_matches(repo, hub) {
                let name = target.rsplit_once(':').map_or(target, |(_, name)| name);
                return names.contains(name)
                    || prefixes.iter().any(|prefix| name.starts_with(prefix));
            }
            spokes
                .iter()
                .any(|spoke| apparent_repo_matches(repo, spoke))
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

const PIP_EXTENSION_BZL: &str = "python/extensions:pip.bzl";
const PIP_REQUIREMENT_ARGS: &[&str] = &[
    "requirements_lock",
    "requirements_linux",
    "requirements_darwin",
    "requirements_windows",
];

/// `pip.parse` hubs and the literal requirements files they read. Calls with
/// computed arguments are skipped, so their files keep the wide fallback.
pub(crate) fn pip_hubs(module_text: &str) -> Vec<Hub> {
    let Ok(module) = ModuleFile::parse(module_text) else {
        return Vec::new();
    };
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        if call.unpacked {
            continue;
        }
        let bzl = literal(call, "extension_bzl_file", Some(0))
            .ok()
            .flatten()
            .unwrap_or_default();
        let name = literal(call, "extension_name", Some(1))
            .ok()
            .flatten()
            .unwrap_or_default();
        if bzl.ends_with(PIP_EXTENSION_BZL) && name == "pip" {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        let Some((binding, "parse")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) || call.unpacked {
            continue;
        }
        let Ok(Some(name)) = literal(call, "hub_name", None) else {
            continue;
        };
        for arg in PIP_REQUIREMENT_ARGS {
            let Ok(Some(label)) = literal(call, arg, None) else {
                continue;
            };
            if let Ok(path) = workspace_path(&label) {
                hubs.push(Hub {
                    name: name.clone(),
                    cargo_lockfile: path,
                    lockfile: None,
                    pip: true,
                    maven: false,
                    go: false,
                    gems: false,
                    pnpm_lockfile: None,
                });
            }
        }
    }
    hubs
}

/// PEP 503 name with `-`, `_` and `.` runs folded to one `-`.
fn pip_name(raw: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for ch in raw.trim().chars() {
        if matches!(ch, '-' | '_' | '.') {
            separator = true;
        } else {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(ch.to_ascii_lowercase());
        }
    }
    out
}

/// Requirement name to its whole logical line (version, markers, hashes).
fn requirement_lines(text: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut logical = String::new();
    let mut flush = |logical: &mut String| {
        let line = logical.split_whitespace().collect::<Vec<_>>().join(" ");
        logical.clear();
        if line.is_empty() || line.starts_with('-') {
            return;
        }
        let end = line
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')))
            .unwrap_or(line.len());
        let name = pip_name(&line[..end]);
        if !name.is_empty() {
            let rest = &line[end..];
            out.entry(name.clone())
                .or_default()
                .push_str(&format!("{name}{rest}\n"));
        }
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_suffix('\\') {
            logical.push_str(rest);
            logical.push(' ');
        } else {
            logical.push_str(line);
            flush(&mut logical);
        }
    }
    flush(&mut logical);
    out
}

pub(crate) fn changed_requirements(base: &str, head: &str) -> ChangedCrates {
    let (base, head) = (requirement_lines(base), requirement_lines(head));
    let mut out = ChangedCrates::new();
    for name in base.keys().chain(head.keys()) {
        if base.get(name) != head.get(name) {
            out.entry(name.clone()).or_default();
        }
    }
    out
}

/// Labels of the changed packages in a `pip.parse` hub, and in its per
/// package spoke repos (`<hub>_<name>` or `<hub>_<python>_<name>`).
pub(crate) fn pip_seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    hub: &str,
    packages: &ChangedCrates,
) -> Vec<String> {
    let forms: BTreeSet<String> = packages
        .keys()
        .flat_map(|name| [name.clone(), name.replace('-', "_")])
        .collect();
    let mut out: Vec<String> = labels
        .filter(|label| {
            let Some((repo, target)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            if apparent_repo_matches(repo, hub) {
                let package = target
                    .split_once(':')
                    .map_or(target, |(package, _)| package);
                let top = package.split('/').next().unwrap_or_default();
                return forms.contains(&pip_name(top)) || forms.contains(top);
            }
            let last = repo.rsplit(['+', '~']).next().unwrap_or(repo);
            last.strip_prefix(&format!("{hub}_")).is_some_and(|rest| {
                forms
                    .iter()
                    .any(|form| rest.ends_with(&format!("_{form}")) || rest == form)
            })
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

const MAVEN_EXTENSION: &str = "extensions.bzl";

/// `maven.install` hubs and their literal `lock_file`. Calls with computed
/// arguments are skipped, so their files keep the wide fallback.
pub(crate) fn maven_hubs(module_text: &str) -> Vec<Hub> {
    let Ok(module) = ModuleFile::parse(module_text) else {
        return Vec::new();
    };
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        if call.unpacked {
            continue;
        }
        let bzl = literal(call, "extension_bzl_file", Some(0))
            .ok()
            .flatten()
            .unwrap_or_default();
        let name = literal(call, "extension_name", Some(1))
            .ok()
            .flatten()
            .unwrap_or_default();
        if bzl.contains("rules_jvm_external") && bzl.ends_with(MAVEN_EXTENSION) && name == "maven" {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        let Some((binding, "install")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) || call.unpacked {
            continue;
        }
        let Ok(name) = literal(call, "name", None) else {
            continue;
        };
        let Ok(Some(label)) = literal(call, "lock_file", None) else {
            continue;
        };
        if let Ok(path) = workspace_path(&label) {
            hubs.push(Hub {
                name: name.unwrap_or_else(|| "maven".to_owned()),
                cargo_lockfile: path,
                lockfile: None,
                pip: false,
                maven: true,
                go: false,
                gems: false,
                pnpm_lockfile: None,
            });
        }
    }
    hubs
}

/// Underscore form of a Maven coordinate, as used in repo and target names.
fn maven_name(coordinate: &str) -> String {
    coordinate.replace(['.', '-', ':'], "_")
}

fn maven_artifacts(text: &str) -> Result<BTreeMap<String, String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("invalid JSON: {error}"))?;
    let artifacts = value
        .get("artifacts")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "no artifacts table".to_owned())?;
    Ok(artifacts
        .iter()
        .map(|(key, entry)| (maven_name(key), entry.to_string()))
        .collect())
}

/// Artifacts added, removed or changed (version or checksum) between two
/// `maven_install.json` files, by underscore name.
pub(crate) fn changed_maven_artifacts(base: &str, head: &str) -> Result<ChangedCrates, String> {
    let (base, head) = (maven_artifacts(base)?, maven_artifacts(head)?);
    let mut out = ChangedCrates::new();
    for name in base.keys().chain(head.keys()) {
        if base.get(name) != head.get(name) {
            out.entry(name.clone()).or_default();
        }
    }
    Ok(out)
}

/// Labels of the changed artifacts in a `maven.install` hub, and in its
/// per artifact repos (`<hub>_<artifact>`).
pub(crate) fn maven_seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    hub: &str,
    artifacts: &ChangedCrates,
) -> Vec<String> {
    let mut out: Vec<String> = labels
        .filter(|label| {
            let Some((repo, target)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            if apparent_repo_matches(repo, hub) {
                let name = target.rsplit_once(':').map_or(target, |(_, name)| name);
                return artifacts.contains_key(name);
            }
            let last = repo.rsplit(['+', '~']).next().unwrap_or(repo);
            last.strip_prefix(&format!("{hub}_"))
                .is_some_and(|rest| artifacts.contains_key(rest))
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn classify(path: &str, hubs: &[Hub]) -> LockKind {
    let gems: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.gems && hub.cargo_lockfile == path)
        .map(|hub| hub.name.clone())
        .collect();
    if !gems.is_empty() {
        return LockKind::Gemfile(gems);
    }
    let maven: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.maven && hub.cargo_lockfile == path)
        .map(|hub| hub.name.clone())
        .collect();
    if !maven.is_empty() {
        return LockKind::Maven(maven);
    }
    let pip: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.pip && hub.cargo_lockfile == path)
        .map(|hub| hub.name.clone())
        .collect();
    if !pip.is_empty() {
        return LockKind::Pip(pip);
    }
    let go: Vec<String> = hubs
        .iter()
        .filter(|hub| {
            hub.go && (hub.cargo_lockfile == path || hub.lockfile.as_deref() == Some(path))
        })
        .map(|hub| hub.name.clone())
        .collect();
    if !go.is_empty() {
        return LockKind::Go(go);
    }
    let hubs: Vec<Hub> = hubs.iter().filter(|hub| !hub.go).cloned().collect();
    let cargo: Vec<String> = hubs
        .iter()
        .filter(|hub| !hub.pip && !hub.maven && hub.cargo_lockfile == path)
        .map(|hub| hub.name.clone())
        .collect();
    if !cargo.is_empty() {
        return LockKind::Cargo(cargo);
    }
    let bazel: Vec<String> = hubs
        .iter()
        .filter(|hub| !hub.pip && !hub.maven && hub.lockfile.as_deref() == Some(path))
        .map(|hub| hub.name.clone())
        .collect();
    if !bazel.is_empty() {
        return LockKind::CrateBazelLock(bazel);
    }
    let pnpm: Vec<String> = hubs
        .iter()
        .filter(|hub| hub.pnpm_lockfile.as_deref() == Some(path))
        .map(|hub| hub.name.clone())
        .collect();
    if !pnpm.is_empty() {
        return LockKind::Pnpm(pnpm);
    }
    LockKind::Unknown
}

/// The `ruby.bundle_fetch` repos with the Gemfile.lock each one reads.
pub(crate) fn gem_hubs(module_text: &str) -> Result<Vec<Hub>, String> {
    let module = ModuleFile::parse(module_text).map_err(|error| error.to_string())?;
    let mut bindings: BTreeSet<&str> = BTreeSet::new();
    for call in module.calls_named("use_extension") {
        if call.unpacked {
            return Err(format!(
                "use_extension at MODULE.bazel line {} unpacks its arguments",
                call.line
            ));
        }
        let bzl = literal(call, "extension_bzl_file", Some(0))?.unwrap_or_default();
        let name = literal(call, "extension_name", Some(1))?.unwrap_or_default();
        if bzl.contains("rules_ruby") && name == "ruby" {
            if let Some(binding) = &call.binding {
                bindings.insert(binding);
            }
        }
    }
    let mut hubs = Vec::new();
    for call in module.calls() {
        let Some((binding, "bundle_fetch")) = call.name.split_once('.') else {
            continue;
        };
        if !bindings.contains(binding) {
            continue;
        }
        if call.unpacked {
            return Err(format!(
                "{} at MODULE.bazel line {} unpacks its arguments",
                call.name, call.line
            ));
        }
        let name = literal(call, "name", None)?.ok_or_else(|| {
            format!(
                "{} at MODULE.bazel line {} has no name",
                call.name, call.line
            )
        })?;
        let gemfile_lock = literal(call, "gemfile_lock", None)?.ok_or_else(|| {
            format!(
                "{} at MODULE.bazel line {} has no gemfile_lock",
                call.name, call.line
            )
        })?;
        hubs.push(Hub {
            name,
            cargo_lockfile: workspace_path(&gemfile_lock)?,
            lockfile: None,
            pip: false,
            maven: false,
            go: false,
            pnpm_lockfile: None,
            gems: true,
        });
    }
    Ok(hubs)
}

/// The Gemfile.lock diff as crate-style name/version changes.
fn changed_gems(base: &str, head: &str) -> Result<LockChange, String> {
    let gems = |text: &str| -> Result<BTreeSet<(String, String)>, String> {
        Ok(dx_audit::locks::parse_gemfile_lock(text)?
            .into_iter()
            .map(|package| {
                let version = match package.platform {
                    Some(platform) => format!("{}-{platform}", package.version),
                    None => package.version,
                };
                (package.name, version)
            })
            .collect())
    };
    let (base, head) = (gems(base)?, gems(head)?);
    let mut crates = ChangedCrates::new();
    for (name, version) in base.symmetric_difference(&head) {
        crates
            .entry(name.clone())
            .or_default()
            .insert(version.clone());
    }
    Ok(LockChange {
        crates,
        aliases: BTreeSet::new(),
        pip: false,
        maven: false,
        go_repos: BTreeSet::new(),
        npm: false,
    })
}

/// Labels of a bundle repo; rules_ruby names its per-gem targets and repos
/// differently across versions, so any label in the repo counts as a user.
pub(crate) fn gem_seeds<'a>(labels: impl Iterator<Item = &'a str>, hub: &str) -> Vec<String> {
    let mut out: Vec<String> = labels
        .filter(|label| {
            label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
                .is_some_and(|(repo, _)| apparent_repo_matches(repo, hub))
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Top-level sections of a pnpm lock other than the package tables, which
/// feed `npm_translate_lock` through settings, overrides and patches.
fn pnpm_settings(text: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut key: Option<String> = None;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('-') {
            let name = line.split(':').next().unwrap_or(line).to_owned();
            key = Some(name);
        }
        if let Some(key) = &key {
            if !matches!(key.as_str(), "importers" | "packages" | "snapshots") {
                let entry = out.entry(key.clone()).or_default();
                entry.push_str(line);
                entry.push('\n');
            }
        }
    }
    out
}

/// The npm packages (name and version) that differ between two pnpm locks.
pub(crate) fn changed_npm_packages(base: &str, head: &str) -> Result<LockChange, String> {
    if pnpm_settings(base) != pnpm_settings(head) {
        return Err("pnpm lock settings changed".to_owned());
    }
    let packages = |text: &str| -> Result<BTreeSet<(String, String)>, String> {
        Ok(dx_audit::locks::parse_pnpm_lock(text)?
            .into_iter()
            .map(|package| (package.name, package.version))
            .collect())
    };
    let (base, head) = (packages(base)?, packages(head)?);
    let mut crates = ChangedCrates::new();
    for (name, version) in base.symmetric_difference(&head) {
        crates
            .entry(name.clone())
            .or_default()
            .insert(version.clone());
    }
    Ok(LockChange {
        crates,
        aliases: BTreeSet::new(),
        pip: false,
        maven: false,
        go_repos: BTreeSet::new(),
        npm: true,
    })
}

/// Repo-name form of an npm package: `@scope/name` becomes `at_scope_name`.
fn npm_repo_name(name: &str) -> String {
    match name.strip_prefix('@') {
        Some(scoped) => format!("at_{}", scoped.replace('/', "_")),
        None => name.to_owned(),
    }
}

/// Labels of changed npm packages: the hub's `@npm//name` targets and the
/// per-version `npm__name__version` spoke repositories.
pub(crate) fn npm_seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    hub: &str,
    packages: &ChangedCrates,
) -> Vec<String> {
    let mut packages_dirs: BTreeSet<&str> = BTreeSet::new();
    let mut prefixes: Vec<String> = Vec::new();
    for (name, versions) in packages {
        packages_dirs.insert(name);
        let repo = npm_repo_name(name);
        for version in versions {
            prefixes.push(format!("{hub}__{repo}__{version}"));
        }
    }
    let mut out: Vec<String> = labels
        .filter(|label| {
            let Some((repo, target)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            if apparent_repo_matches(repo, hub) {
                let package = target
                    .split_once(':')
                    .map_or(target, |(package, _)| package);
                let package = package.strip_prefix("node_modules/").unwrap_or(package);
                return packages_dirs.contains(package);
            }
            prefixes.iter().any(|prefix| {
                let local = repo.rsplit(['+', '~']).next().unwrap_or(repo);
                local.starts_with(prefix.as_str())
            })
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str = r#"
module(name = "demo")
crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(
    cargo_lockfile = "//:Cargo.lock",
    lockfile = "//:cargo-bazel-lock.json",
    manifests = ["//:Cargo.toml"],
)
crate.from_cargo(
    name = "tools",
    cargo_lockfile = "//tools/rust:Cargo.lock",
)
go = use_extension("@gazelle//:extensions.bzl", "go_deps")
go.from_cargo(name = "nope", cargo_lockfile = "//:other.lock")
"#;

    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

    fn lock(packages: &[(&str, &str, &[&str])]) -> String {
        let mut out = String::from("version = 3\n");
        for (name, version, deps) in packages {
            out.push_str(&format!(
                "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{REGISTRY}\"\n"
            ));
            if !deps.is_empty() {
                let list: Vec<String> = deps.iter().map(|dep| format!(" \"{dep}\",")).collect();
                out.push_str(&format!("dependencies = [\n{}\n]\n", list.join("\n")));
            }
        }
        out
    }

    fn crates(entries: &[(&str, &[&str])]) -> ChangedCrates {
        entries
            .iter()
            .map(|(name, versions)| {
                (
                    (*name).to_owned(),
                    versions.iter().map(ToString::to_string).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn maven_artifacts_diff_by_entry() {
        let base = r#"{"artifacts":{"com.a:b-c":{"version":"1","shasums":{"jar":"x"}},"d:e":{"version":"2"}}}"#;
        let head = r#"{"artifacts":{"com.a:b-c":{"version":"1","shasums":{"jar":"x"}},"d:e":{"version":"3"},"f:g":{"version":"1"}}}"#;
        let changed = changed_maven_artifacts(base, head).expect("diff");
        assert_eq!(changed.keys().cloned().collect::<Vec<_>>(), ["d_e", "f_g"]);
        assert!(changed_maven_artifacts("{}", head).is_err());
        let labels = [
            "@@//:x",
            "@@maven//:d_e",
            "@@maven//:other",
            "@@rules_jvm_external++maven+maven_f_g//jar:jar",
        ];
        assert_eq!(
            maven_seeds(labels.iter().copied(), "maven", &changed),
            [
                "@@maven//:d_e",
                "@@rules_jvm_external++maven+maven_f_g//jar:jar"
            ]
        );
    }

    #[test]
    fn gem_hubs_and_changed_gems() {
        let module = "ruby = use_extension(\"@rules_ruby//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"bundle\", gemfile_lock = \"//third_party/ruby:Gemfile.lock\")\n";
        let hubs = gem_hubs(module).expect("hubs");
        assert_eq!(
            classify("third_party/ruby/Gemfile.lock", &hubs),
            LockKind::Gemfile(vec!["bundle".to_owned()])
        );
        assert_eq!(classify("Cargo.lock", &hubs), LockKind::Unknown);
        let lock = |rake: &str| {
            format!("GEM\n  remote: https://rubygems.org/\n  specs:\n    rake ({rake})\n    racc (1.8.1)\n\nPLATFORMS\n  ruby\n")
        };
        let change = changed_gems(&lock("13.0.0"), &lock("13.1.0")).expect("diff");
        assert_eq!(change.crates, crates(&[("rake", &["13.0.0", "13.1.0"])]));
        assert!(changed_gems(&lock("1"), &lock("1"))
            .expect("same")
            .crates
            .is_empty());
    }

    #[test]
    fn crate_hubs_read_literal_from_cargo_calls() {
        let hubs = crate_hubs(MODULE).expect("hubs");
        assert_eq!(
            hubs,
            [
                Hub {
                    name: "crates".to_owned(),
                    cargo_lockfile: "Cargo.lock".to_owned(),
                    lockfile: Some("cargo-bazel-lock.json".to_owned()),
                    pip: false,
                    maven: false,
                    go: false,
                    gems: false,
                    pnpm_lockfile: None,
                },
                Hub {
                    name: "tools".to_owned(),
                    cargo_lockfile: "tools/rust/Cargo.lock".to_owned(),
                    lockfile: None,
                    pip: false,
                    maven: false,
                    go: false,
                    gems: false,
                    pnpm_lockfile: None,
                },
            ]
        );
        assert_eq!(crate_hubs("module(name = \"x\")\n"), Ok(Vec::new()));
        let computed = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = NAME, cargo_lockfile = \"//:Cargo.lock\")\n";
        assert!(crate_hubs(computed).is_err());
        let other_repo = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"@other//:Cargo.lock\")\n";
        assert!(crate_hubs(other_repo).is_err());
        assert!(crate_hubs("crate.from_cargo(\n").is_err());
    }

    #[test]
    fn go_deps_files_classify_and_diff_to_repos() {
        let module = "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n";
        let hubs = crate_hubs(module).expect("hubs");
        let kind = LockKind::Go(vec!["go_deps".to_owned()]);
        assert_eq!(classify("third_party/go/go.mod", &hubs), kind);
        assert_eq!(classify("third_party/go/go.sum", &hubs), kind);
        assert_eq!(classify("go.sum", &hubs), LockKind::Unknown);
        assert_eq!(
            go_repo_name("github.com/Foo-Bar/baz.v2"),
            "com_github_foo_bar_baz_v2"
        );
        let base = "module x\n\ngo 1.22\n\nrequire (\n\tgithub.com/a/b v1.0.0\n\tgolang.org/x/net v0.1.0 // indirect\n)\n";
        let head = base.replace("v1.0.0", "v1.1.0");
        let change = changed_go("go.mod", base, &head).expect("go.mod");
        assert_eq!(
            change.go_repos,
            BTreeSet::from(["com_github_a_b".to_owned()])
        );
        let toolchain = base.replace("go 1.22", "go 1.23");
        assert!(changed_go("go.mod", base, &toolchain).is_err());
        let sum = "github.com/a/b v1.0.0 h1:x\ngithub.com/a/b v1.0.0/go.mod h1:y\ngolang.org/x/net v0.1.0 h1:z\n";
        let sum_head = sum.replace("h1:x", "h1:w");
        let change = changed_go("go.sum", sum, &sum_head).expect("go.sum");
        assert_eq!(
            change.go_repos,
            BTreeSet::from(["com_github_a_b".to_owned()])
        );
        let labels = [
            "@@gazelle++go_deps+com_github_a_b//:b",
            "@@gazelle++go_deps+org_golang_x_net//:net",
        ];
        assert_eq!(
            go_seeds(labels.iter().copied(), &change.go_repos),
            ["@@gazelle++go_deps+com_github_a_b//:b"]
        );
    }

    #[test]
    fn classify_names_the_hubs_of_each_lock() {
        let hubs = crate_hubs(MODULE).expect("hubs");
        assert_eq!(
            classify("Cargo.lock", &hubs),
            LockKind::Cargo(vec!["crates".to_owned()])
        );
        assert_eq!(
            classify("cargo-bazel-lock.json", &hubs),
            LockKind::CrateBazelLock(vec!["crates".to_owned()])
        );
        assert_eq!(
            classify("tools/rust/Cargo.lock", &hubs),
            LockKind::Cargo(vec!["tools".to_owned()])
        );
        assert_eq!(classify("other.lock", &hubs), LockKind::Unknown);
    }

    #[test]
    fn one_version_bump_changes_only_that_crate() {
        let base = lock(&[("serde", "1.0.1", &[]), ("itoa", "1.0.0", &[])]);
        let head = lock(&[("serde", "1.0.2", &[]), ("itoa", "1.0.0", &[])]);
        assert_eq!(
            changed_crates(&base, &head),
            Ok(crates(&[("serde", &["1.0.1", "1.0.2"])]))
        );
        assert_eq!(changed_crates(&base, &base), Ok(ChangedCrates::new()));
        assert!(changed_crates(&base, "not toml [[[").is_err());
    }

    #[test]
    fn a_bumped_dependency_selects_every_crate_that_reaches_it() {
        let base = lock(&[
            ("mio", "1.0.0", &[]),
            ("tokio", "1.40.0", &["mio"]),
            ("hyper", "1.0.0", &["tokio"]),
            ("serde", "1.0.0", &[]),
        ]);
        let head = lock(&[
            ("mio", "1.0.1", &[]),
            ("tokio", "1.40.0", &["mio"]),
            ("hyper", "1.0.0", &["tokio"]),
            ("serde", "1.0.0", &[]),
        ]);
        assert_eq!(
            changed_crates(&base, &head),
            Ok(crates(&[
                ("hyper", &["1.0.0"]),
                ("mio", &["1.0.0", "1.0.1"]),
                ("tokio", &["1.40.0"]),
            ]))
        );
    }

    fn bazel_lock_with(serde: &str, direct: &[&str], rename: Option<&str>) -> serde_json::Value {
        let mut dep = serde_json::json!({"id": format!("serde {serde}"), "target": "serde"});
        if let Some(rename) = rename {
            dep["alias"] = rename.into();
        }
        serde_json::json!({
            "checksum": format!("sum-{serde}"),
            "crates": {
                format!("serde {serde}"): {
                    "name": "serde",
                    "extra_aliased_targets": {"serde_extra": "x"},
                },
                "serde_json 1.0.0": {
                    "name": "serde_json",
                    "common_attrs": {"deps": [{"id": format!("serde {serde}"), "target": "serde"}]},
                },
                "app 0.1.0": {
                    "name": "app",
                    "common_attrs": {"deps": {"selects": {"cfg(unix)": [{"id": "serde_json 1.0.0", "target": "serde_json"}]}, "common": [dep]}},
                },
                "tool 0.2.0": {
                    "name": "tool",
                    "targets": [{"Binary": {"crate_name": "tool"}}],
                },
                "itoa 1.0.0": {"name": "itoa"},
            },
            "conditions": {"cfg(unix)": ["x86_64-unknown-linux-gnu"]},
            "direct_deps": direct,
            "direct_dev_deps": [],
            "workspace_members": {"app 0.1.0": "app"},
            "binary_crates": ["tool 0.2.0"],
        })
    }

    fn bazel_lock(serde: &str, extra: &str) -> String {
        bazel_lock_with(serde, &["serde_json 1.0.0", extra], None).to_string()
    }

    fn change(base: &serde_json::Value, head: &serde_json::Value) -> Result<LockChange, String> {
        changed_crates_from_bazel_lock(&base.to_string(), &head.to_string())
    }

    fn names(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn bazel_lock_diffs_crates_and_their_dependents() {
        let change =
            changed_crates_from_bazel_lock(&bazel_lock("1.0.1", ""), &bazel_lock("1.0.2", ""))
                .expect("change");
        assert_eq!(
            change.crates,
            crates(&[("serde", &["1.0.1", "1.0.2"]), ("serde_json", &["1.0.0"])])
        );
        assert_eq!(change.aliases, names(&["serde_extra"]));
    }

    #[test]
    fn bazel_lock_direct_dep_bump_narrows() {
        let base = bazel_lock_with("1.0.1", &["serde 1.0.1"], None);
        let head = bazel_lock_with("1.0.2", &["serde 1.0.2"], None);
        let change = change(&base, &head).expect("narrowed");
        assert!(change.crates.contains_key("serde"));
        assert!(!change.crates.contains_key("app"));
    }

    #[test]
    fn bazel_lock_changes_outside_crates_fail() {
        let base = bazel_lock_with("1.0.1", &[], None);
        let mut head = base.clone();
        head["direct_deps"] = serde_json::json!(["serde 1.0.1"]);
        head["workspace_members"] = serde_json::json!({});
        assert_eq!(change(&base, &head), Ok(LockChange::default()));
        let mut head = base.clone();
        head["conditions"] = serde_json::json!({});
        assert_eq!(
            change(&base, &head),
            Err("crate_universe lockfile changed conditions".to_owned())
        );
        let mut head = base.clone();
        head["mystery"] = serde_json::json!(1);
        assert!(change(&base, &head).is_err());
        assert!(changed_crates_from_bazel_lock("[]", &bazel_lock("1.0.1", "")).is_err());
        assert!(changed_crates_from_bazel_lock("{", &bazel_lock("1.0.1", "")).is_err());
    }

    #[test]
    fn bazel_lock_aliases_cover_renames_extras_and_binaries() {
        let base = bazel_lock_with("1.0.1", &[], Some("serde1"));
        let head = bazel_lock_with("1.0.2", &[], Some("serde1"));
        let aliases = change(&base, &head).expect("change").aliases;
        assert_eq!(
            aliases,
            names(&["serde1", "serde1-1.0.1", "serde1-1.0.2", "serde_extra"])
        );
        // A dropped rename counts even though crates[serde] is unchanged.
        let dropped = bazel_lock_with("1.0.1", &[], None);
        let aliases = change(&base, &dropped).expect("change").aliases;
        assert_eq!(aliases, names(&["serde1", "serde1-1.0.1"]));
        // A changed binary crate yields its aliases; unchanged ones do not.
        let mut head = base.clone();
        head["crates"]["tool 0.2.0"]["version"] = "x".into();
        let aliases = change(&base, &head).expect("change").aliases;
        assert_eq!(aliases, names(&["tool__tool", "tool-0.2.0__tool"]));
    }

    #[test]
    fn seeds_match_hub_names_versioned_names_and_spoke_repos() {
        let labels = [
            "@@rules_rust++crate+crates//:serde",
            "@@rules_rust++crate+crates//:serde-1.0.2",
            "@@rules_rust++crate+crates//:serde_json",
            "@@rules_rust++crate+crates__serde-1.0.1//:serde",
            "@@rules_rust~~crate~crates__serde-1.0.2//:serde",
            "@@rules_rust++crate+my_crates//:serde",
            "@@crates//:serde",
            "@@mycrates+//:serde",
            "@@crates+//:serde",
            "//a:serde",
            "@@rules_rust++crate+crates//:serde__bin",
            "@@rules_rust++crate+crates//:serde-1.0.2__bin",
            "@@rules_rust++crate+crates//serde:serde",
            "@@rules_rust++crate+crates//:serde1",
            "@@rules_rust++crate+crates//:serde_json__x",
            "@@rules_rust++crate+crates//:serdex",
            "@@rules_rust++crate+crates//:serde-1.0.3__x",
        ];
        assert_eq!(
            seeds(
                labels.into_iter(),
                "crates",
                &crates(&[("serde", &["1.0.1", "1.0.2"])]),
                &names(&["serde1"])
            ),
            [
                "@@crates+//:serde",
                "@@crates//:serde",
                "@@rules_rust++crate+crates//:serde",
                "@@rules_rust++crate+crates//:serde-1.0.2",
                "@@rules_rust++crate+crates//:serde-1.0.2__bin",
                "@@rules_rust++crate+crates//:serde1",
                "@@rules_rust++crate+crates//:serde__bin",
                "@@rules_rust++crate+crates//serde:serde",
                "@@rules_rust++crate+crates__serde-1.0.1//:serde",
                "@@rules_rust~~crate~crates__serde-1.0.2//:serde",
            ]
        );
    }

    #[test]
    fn pip_requirements_diff_by_package() {
        let base = "requests==2.0 \\\n    --hash=sha256:a\nFoo_Bar==1\n# c\nnumpy==1\n";
        let head = "requests==2.1 \\\n    --hash=sha256:b\nfoo-bar==1\nflask==3\n";
        let changed = changed_requirements(base, head);
        let names: Vec<&str> = changed.keys().map(String::as_str).collect();
        assert_eq!(names, ["flask", "numpy", "requests"]);
    }

    #[test]
    fn pip_hubs_and_seeds() {
        let module = "pip = use_extension(\"@rules_python//python/extensions:pip.bzl\", \"pip\")\npip.parse(hub_name = \"pypi\", requirements_lock = \"//py:requirements_lock.txt\")\n";
        let hubs = pip_hubs(module);
        assert_eq!(
            classify("py/requirements_lock.txt", &hubs),
            LockKind::Pip(vec!["pypi".to_owned()])
        );
        let changed = changed_requirements("foo-bar==1\n", "foo-bar==2\n");
        let labels = [
            "@@rules_python++pip+pypi//foo_bar:pkg",
            "@@rules_python++pip+pypi_311_foo_bar//:pkg",
            "@@rules_python++pip+pypi//other:pkg",
            "@@rules_python++pip+pypi_311_other//:pkg",
        ];
        assert_eq!(
            pip_seeds(labels.iter().copied(), "pypi", &changed),
            [labels[0], labels[1]]
        );
    }

    const NPM_MODULE: &str = r#"
npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(name = "npm", pnpm_lock = "//web:pnpm-lock.yaml")
"#;

    fn pnpm_lock(packages: &[(&str, &str)], settings: &str) -> String {
        let mut out = format!("lockfileVersion: '9.0'\n\nsettings:\n  {settings}\n\npackages:\n");
        for (name, version) in packages {
            out.push_str(&format!(
                "\n  {name}@{version}:\n    resolution: {{integrity: sha512-abc}}\n"
            ));
        }
        out
    }

    #[test]
    fn npm_translate_lock_hubs_classify_pnpm_locks() {
        let hubs = crate_hubs(NPM_MODULE).expect("hubs");
        assert_eq!(
            classify("web/pnpm-lock.yaml", &hubs),
            LockKind::Pnpm(vec!["npm".to_owned()])
        );
        assert_eq!(classify("pnpm-lock.yaml", &hubs), LockKind::Unknown);
        assert!(crate_hubs(&NPM_MODULE.replace("\"//web:pnpm-lock.yaml\"", "LOCK")).is_err());
    }

    #[test]
    fn pnpm_diff_names_changed_packages_only() {
        let base = pnpm_lock(&[("left-pad", "1.0.0"), ("lodash", "4.0.0")], "a: 1");
        let head = pnpm_lock(&[("left-pad", "1.0.0"), ("lodash", "4.1.0")], "a: 1");
        let change = changed_npm_packages(&base, &head).expect("diff");
        assert!(change.npm);
        assert_eq!(change.crates, crates(&[("lodash", &["4.0.0", "4.1.0"])]));
        let settings = pnpm_lock(&[("left-pad", "1.0.0"), ("lodash", "4.0.0")], "a: 2");
        assert!(changed_npm_packages(&base, &settings).is_err());
    }

    #[test]
    fn npm_seeds_match_hub_targets_and_version_spokes() {
        let labels = [
            "@@aspect_rules_js++npm+npm//lodash:pkg",
            "@@aspect_rules_js++npm+npm//left-pad:pkg",
            "@@aspect_rules_js++npm+npm__lodash__4.1.0//:pkg",
            "@@aspect_rules_js++npm+npm__lodash__3.0.0//:pkg",
            "@@aspect_rules_js++npm+npm__at_scope_a__1.0.0//:pkg",
        ];
        let changed = crates(&[("lodash", &["4.1.0"]), ("@scope/a", &["1.0.0"])]);
        assert_eq!(
            npm_seeds(labels.iter().copied(), "npm", &changed),
            [
                "@@aspect_rules_js++npm+npm//lodash:pkg",
                "@@aspect_rules_js++npm+npm__at_scope_a__1.0.0//:pkg",
                "@@aspect_rules_js++npm+npm__lodash__4.1.0//:pkg",
            ]
        );
    }
}
