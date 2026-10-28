use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_module_bazel::{load_labels, Call, ModuleFile, Value};

fn read(workspace: &Path, rel: &str) -> Result<Option<String>, String> {
    match std::fs::read_to_string(workspace.join(rel)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {rel}: {error}")),
    }
}

fn literal_arg<'a>(call: &'a Call, key: &str) -> Result<&'a str, String> {
    match call.positional.first().or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Ok(&literal.value),
        _ => Err(format!(
            "{} at line {} has no literal file label",
            call.name, call.line
        )),
    }
}

fn main_repo(repo: &str, names: &[String]) -> bool {
    repo.is_empty() || names.iter().any(|name| name == repo)
}

fn label_path(label: &str, package: &str, names: &[String]) -> Option<(String, String)> {
    let (package, target) = if let Some(rest) = label.strip_prefix('@') {
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        let (repo, rest) = rest.split_once("//")?;
        if !main_repo(repo, names) {
            return None;
        }
        main_label(rest)
    } else if let Some(rest) = label.strip_prefix("//") {
        main_label(rest)
    } else {
        (
            package.to_owned(),
            label.strip_prefix(':').unwrap_or(label).to_owned(),
        )
    };
    if target.is_empty()
        || format!("{package}/{target}")
            .split('/')
            .any(|part| part == "..")
    {
        return None;
    }
    let path = if package.is_empty() {
        target.clone()
    } else {
        format!("{package}/{target}")
    };
    Some((path, package))
}

fn main_label(rest: &str) -> (String, String) {
    match rest.split_once(':') {
        Some((package, target)) => (package.to_owned(), target.to_owned()),
        None => (
            rest.to_owned(),
            rest.rsplit('/').next().unwrap_or(rest).to_owned(),
        ),
    }
}

fn own_names(file: &ModuleFile) -> Vec<String> {
    file.calls_named("module")
        .flat_map(|call| [call.str("name"), call.str("repo_name")])
        .flatten()
        .map(str::to_owned)
        .collect()
}

/// One main-repository file that MODULE.bazel or an included segment names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleInput {
    pub path: String,
    pub extension: Option<(String, String)>,
    pub tag: String,
    pub hub: Option<String>,
}

/// The files MODULE.bazel names, and whether every scanned value was literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleInputs {
    pub files: Vec<ModuleInput>,
    pub complete: bool,
}

struct ModuleRoots {
    names: Vec<String>,
    extensions: Vec<(String, String)>,
    toolchains: Vec<(String, String)>,
    inputs: ModuleInputs,
}

const UNSCANNED_CALLS: &[&str] = &[
    "module",
    "bazel_dep",
    "use_extension",
    "use_repo_rule",
    "use_repo",
    "inject_repo",
    "override_repo",
    "register_toolchains",
    "register_execution_platforms",
    "include",
];

fn is_main_label(text: &str, names: &[String]) -> bool {
    if text.starts_with("//") || text.starts_with(':') {
        return true;
    }
    let Some(rest) = text.strip_prefix('@') else {
        return false;
    };
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    rest.split_once("//")
        .is_some_and(|(repo, _)| main_repo(repo, names))
}

fn value_labels<'a>(value: &'a Value, labels: &mut Vec<&'a str>, complete: &mut bool) {
    match value {
        Value::Str(literal) => labels.push(&literal.value),
        Value::List(items) => {
            for item in items {
                value_labels(item, labels, complete);
            }
        }
        Value::Dict(entries) => {
            for (key, item) in entries {
                value_labels(key, labels, complete);
                value_labels(item, labels, complete);
            }
        }
        Value::Ident(name) if matches!(name.as_str(), "True" | "False" | "None") => {}
        Value::Ident(_) | Value::Computed { .. } => *complete = false,
    }
}

fn binding_arg(call: &Call, index: usize, key: &str) -> Option<String> {
    match call.positional.get(index).or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Some(literal.value.clone()),
        _ => None,
    }
}

fn scan_inputs(file: &ModuleFile, package: &str, names: &[String], inputs: &mut ModuleInputs) {
    let mut extensions: BTreeMap<&str, (String, String)> = BTreeMap::new();
    let mut rules: BTreeMap<&str, (String, String)> = BTreeMap::new();
    for call in file.calls() {
        let Some(binding) = call.binding.as_deref() else {
            continue;
        };
        let (bindings, file_key, name_key) = match call.name.as_str() {
            "use_extension" => (&mut extensions, "extension_bzl_file", "extension_name"),
            "use_repo_rule" => (&mut rules, "repo_rule_bzl_file", "repo_rule_name"),
            _ => continue,
        };
        let bzl = binding_arg(call, 0, file_key).unwrap_or_default();
        let name = binding_arg(call, 1, name_key).unwrap_or_default();
        bindings.insert(binding, (bzl, name));
    }
    for call in file.calls() {
        if UNSCANNED_CALLS.contains(&call.name.as_str()) {
            continue;
        }
        let (extension, tag) = if let Some(rule) = rules.get(call.name.as_str()) {
            (Some(rule.clone()), call.name.clone())
        } else if let Some((binding, tag)) = call.name.split_once('.') {
            match extensions.get(binding) {
                Some(extension) => (Some(extension.clone()), tag.to_owned()),
                None => continue,
            }
        } else if call.name.ends_with("_override") {
            (None, call.name.clone())
        } else {
            continue;
        };
        if call.unpacked {
            inputs.complete = false;
        }
        let mut labels = Vec::new();
        for value in call
            .positional
            .iter()
            .chain(call.named.iter().map(|(_, value)| value))
        {
            value_labels(value, &mut labels, &mut inputs.complete);
        }
        let hub = call
            .str("hub_name")
            .or_else(|| call.str("name"))
            .map(str::to_owned);
        for label in labels {
            if !is_main_label(label, names) {
                continue;
            }
            if let Some((path, _)) = label_path(label, package, names) {
                inputs.files.push(ModuleInput {
                    path,
                    extension: extension.clone(),
                    tag: tag.clone(),
                    hub: hub.clone(),
                });
            }
        }
    }
}

fn module_roots(workspace: &Path) -> Result<ModuleRoots, String> {
    let mut roots = ModuleRoots {
        names: Vec::new(),
        extensions: Vec::new(),
        toolchains: Vec::new(),
        inputs: ModuleInputs {
            files: Vec::new(),
            complete: true,
        },
    };
    let mut modules: Vec<(String, String)> = vec![("MODULE.bazel".to_owned(), String::new())];
    let mut seen_modules: BTreeSet<String> = BTreeSet::new();
    while let Some((rel, package)) = modules.pop() {
        if !seen_modules.insert(rel.clone()) {
            continue;
        }
        let Some(source) = read(workspace, &rel)? else {
            if rel == "MODULE.bazel" {
                roots.inputs.complete = false;
                return Ok(roots);
            }
            return Err(format!("{rel} is included but missing"));
        };
        let file = ModuleFile::parse(&source).map_err(|error| format!("{rel}: {error}"))?;
        if rel == "MODULE.bazel" {
            roots.names = own_names(&file);
        }
        scan_inputs(&file, &package, &roots.names, &mut roots.inputs);
        for call in file.calls() {
            if matches!(
                call.name.as_str(),
                "register_toolchains" | "register_execution_platforms"
            ) {
                if call.unpacked {
                    return Err(no_pattern(&rel, call));
                }
                for value in &call.positional {
                    let Value::Str(pattern) = value else {
                        return Err(no_pattern(&rel, call));
                    };
                    roots
                        .toolchains
                        .push((pattern.value.clone(), package.clone()));
                }
                continue;
            }
            let key = match call.name.as_str() {
                "use_extension" => "extension_bzl_file",
                "use_repo_rule" => "repo_rule_bzl_file",
                "include" => "label",
                _ => continue,
            };
            let label = literal_arg(call, key).map_err(|error| format!("{rel}: {error}"))?;
            let Some(target) = label_path(label, &package, &roots.names) else {
                if call.name == "include" {
                    return Err(format!(
                        "{rel}: include {label:?} is not in the main repository"
                    ));
                }
                continue;
            };
            if call.name == "include" {
                roots.inputs.files.push(ModuleInput {
                    path: target.0.clone(),
                    extension: None,
                    tag: "include".to_owned(),
                    hub: None,
                });
                modules.push(target);
            } else {
                roots.extensions.push(target);
            }
        }
    }
    Ok(roots)
}

/// The main-repository files that MODULE.bazel and its included segments name.
pub fn module_inputs(workspace: &Path) -> Result<ModuleInputs, String> {
    module_roots(workspace).map(|roots| roots.inputs)
}

fn no_pattern(rel: &str, call: &Call) -> String {
    format!(
        "{rel}: {} at line {} has no literal target pattern",
        call.name, call.line
    )
}

struct BazelrcRoots {
    aspects: Vec<String>,
    toolchains: Vec<String>,
    repos: BTreeSet<String>,
}

/// The repository a label names, or `None` for a label in the main repository.
fn label_repo(label: &str) -> Option<&str> {
    let rest = label.strip_prefix('@')?;
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    Some(rest.split_once("//").map_or(rest, |(repo, _)| repo))
}

/// Whether `name` is the repository `repo`, or one it owns when `repo` is canonical.
fn repo_matches(name: &str, repo: &str) -> bool {
    name == repo || ((repo.ends_with('+') || repo.ends_with('~')) && name.starts_with(repo))
}

fn bazelrc_roots(workspace: &Path) -> Result<BazelrcRoots, String> {
    let mut pending: Vec<String> = Vec::new();
    let entries =
        std::fs::read_dir(workspace).map_err(|error| format!("cannot read workspace: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read workspace: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".bazelrc") && workspace.join(&name).is_file() {
            pending.push(name);
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut roots = BazelrcRoots {
        aspects: Vec::new(),
        toolchains: Vec::new(),
        repos: BTreeSet::new(),
    };
    while let Some(rel) = pending.pop() {
        if !seen.insert(rel.clone()) {
            continue;
        }
        let Some(text) = read(workspace, &rel)? else {
            continue;
        };
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default();
            let words: Vec<&str> = line.split_whitespace().collect();
            if let [verb, path] = words.as_slice() {
                if matches!(*verb, "import" | "try-import") {
                    if let Some(rest) = path.strip_prefix("%workspace%/") {
                        pending.push(rest.to_owned());
                    }
                    continue;
                }
            }
            for word in words.iter().skip(1) {
                let word = word.strip_prefix("--").unwrap_or(word);
                let named = word
                    .split(['=', ','])
                    .map(|item| item.trim_matches(|c| c == '"' || c == '\''))
                    .filter_map(label_repo)
                    .filter(|repo| !repo.is_empty())
                    .map(str::to_owned);
                roots.repos.extend(named);
            }
            for (index, word) in words.iter().enumerate() {
                let Some(flag) = word.strip_prefix("--") else {
                    continue;
                };
                let (flag, value) = match flag.split_once('=') {
                    Some(pair) => pair,
                    None => match words.get(index + 1) {
                        Some(value) => (flag, *value),
                        None => continue,
                    },
                };
                let list = match flag {
                    "aspects" => &mut roots.aspects,
                    "extra_toolchains"
                    | "extra_execution_platforms"
                    | "platforms"
                    | "host_platform" => &mut roots.toolchains,
                    _ => continue,
                };
                list.extend(
                    value
                        .split(',')
                        .map(|item| item.trim_matches(|c| c == '"' || c == '\''))
                        .filter(|item| !item.is_empty())
                        .map(str::to_owned),
                );
            }
        }
    }
    Ok(roots)
}

fn pattern_builds(
    pattern: &str,
    package: &str,
    names: &[String],
    builds: &[(String, String)],
) -> Vec<String> {
    let rest = if let Some(rest) = pattern.strip_prefix('@') {
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        let Some((repo, rest)) = rest.split_once("//") else {
            return Vec::new();
        };
        if !main_repo(repo, names) {
            return Vec::new();
        }
        rest.to_owned()
    } else if let Some(rest) = pattern.strip_prefix("//") {
        rest.to_owned()
    } else if pattern.starts_with('-') {
        return Vec::new();
    } else if let Some(target) = pattern.strip_prefix(':') {
        format!("{package}:{target}")
    } else {
        let joined = if package.is_empty() {
            pattern.to_owned()
        } else {
            format!("{package}/{pattern}")
        };
        let mut found = pattern_builds(&format!("//{joined}"), "", names, builds);
        if !pattern.contains('/') && !pattern.contains(':') {
            found.extend(pattern_builds(&format!("//{package}"), "", names, builds));
        }
        return found;
    };
    let spec = rest.split_once(':').map_or(rest.as_str(), |(spec, _)| spec);
    if spec.split('/').any(|part| part == "..") {
        return Vec::new();
    }
    let recursive = if spec == "..." {
        Some("")
    } else {
        spec.strip_suffix("/...")
    };
    builds
        .iter()
        .filter(|(_, build_package)| match recursive {
            Some("") => true,
            Some(prefix) => {
                build_package == prefix
                    || build_package
                        .strip_prefix(prefix)
                        .is_some_and(|tail| tail.starts_with('/'))
            }
            None => build_package == spec,
        })
        .map(|(rel, _)| rel.clone())
        .collect()
}

/// The `.bazelignore` prefixes of the workspace, without `./` or trailing slashes.
pub fn read_bazelignore(workspace: &Path) -> Result<Vec<String>, String> {
    let Some(text) = read(workspace, ".bazelignore")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("./").unwrap_or(line);
        let line = line.trim_end_matches('/');
        if line.is_empty() || line == "." {
            continue;
        }
        out.push(line.to_owned());
    }
    Ok(out)
}

fn is_ignored(rel: &str, ignores: &[String]) -> bool {
    ignores.iter().any(|prefix| {
        rel == prefix
            || rel
                .strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Walks the workspace like Bazel would: honours `.bazelignore`, skips `skip` directory names
/// and never follows symlinks. `visit(name, rel, dir)` returns true to stop early.
fn walk_workspace(
    workspace: &Path,
    skip: &[&str],
    mut visit: impl FnMut(&str, &str, &str) -> bool,
) -> Result<(), String> {
    let ignores = read_bazelignore(workspace)?;
    let mut dirs: Vec<String> = vec![String::new()];
    while let Some(dir) = dirs.pop() {
        let shown = if dir.is_empty() { "." } else { dir.as_str() };
        let entries = std::fs::read_dir(workspace.join(&dir))
            .map_err(|error| format!("cannot read {shown}: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read {shown}: {error}"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if dir.is_empty() {
                name.clone()
            } else {
                format!("{dir}/{name}")
            };
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot read {rel}: {error}"))?;
            if kind.is_dir() {
                if !skip.contains(&name.as_str()) && !is_ignored(&rel, &ignores) {
                    dirs.push(rel);
                }
            } else if visit(&name, &rel, &dir) {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn build_files(workspace: &Path) -> Result<Vec<(String, String)>, String> {
    let mut found: Vec<(String, String)> = Vec::new();
    walk_workspace(workspace, &[".git"], |name, rel, dir| {
        if matches!(name, "BUILD" | "BUILD.bazel") && workspace.join(rel).is_file() {
            found.push((rel.to_owned(), dir.to_owned()));
        }
        false
    })?;
    found.sort();
    Ok(found)
}

/// Whether the workspace has a `.cu` or `.cuh` source Bazel can see. Stops at the first one.
pub fn workspace_has_cuda_sources(root: &Path) -> Result<bool, crate::AdoptError> {
    let mut found = false;
    walk_workspace(root, &[".git", ".dx"], |name, rel, _| {
        let cuda = name.ends_with(".cu") || name.ends_with(".cuh");
        found = cuda && root.join(rel).is_file();
        found
    })
    .map_err(|detail| crate::AdoptError::WorkspaceScan { detail })?;
    Ok(found)
}

struct LoadGraph<'a> {
    workspace: &'a Path,
    names: Vec<String>,
    loads: BTreeMap<String, Vec<String>>,
    external: BTreeMap<String, BTreeSet<String>>,
}

impl LoadGraph<'_> {
    fn add(&mut self, rel: &str, package: &str) -> Result<(), String> {
        let mut pending: Vec<(String, String)> = vec![(rel.to_owned(), package.to_owned())];
        while let Some((rel, package)) = pending.pop() {
            if self.loads.contains_key(&rel) {
                continue;
            }
            let Some(source) = read(self.workspace, &rel)? else {
                self.loads.insert(rel, Vec::new());
                continue;
            };
            let labels = load_labels(&source).map_err(|error| format!("{rel}: {error}"))?;
            let mut targets = Vec::new();
            for label in labels {
                if let Some(target) = label_path(&label, &package, &self.names) {
                    targets.push(target.0.clone());
                    pending.push(target);
                } else if let Some(repo) = label_repo(&label) {
                    self.external
                        .entry(rel.clone())
                        .or_default()
                        .insert(repo.to_owned());
                }
            }
            self.loads.insert(rel, targets);
        }
        Ok(())
    }

    fn closure(&self, roots: &[String]) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<&str> = roots.iter().map(String::as_str).collect();
        while let Some(rel) = pending.pop() {
            if !seen.insert(rel.to_owned()) {
                continue;
            }
            if let Some(targets) = self.loads.get(rel) {
                pending.extend(targets.iter().map(String::as_str));
            }
        }
        seen
    }
}

/// How a changed .bzl file reaches the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BzlImpact {
    Global(String),
    Packages(BTreeSet<String>),
}

struct ImpactGraph<'a> {
    graph: LoadGraph<'a>,
    aspects: Vec<String>,
    bazelrc_repos: BTreeSet<String>,
    extension_inputs: BTreeSet<String>,
    aspect_inputs: BTreeSet<String>,
    toolchain_inputs: BTreeSet<String>,
    loaders: BTreeMap<String, Vec<String>>,
    packages: BTreeMap<String, String>,
}

fn impact_graph<'a>(
    workspace: &'a Path,
    aspect_labels: &[String],
) -> Result<ImpactGraph<'a>, String> {
    let roots = module_roots(workspace)?;
    let mut graph = LoadGraph {
        workspace,
        names: roots.names.clone(),
        loads: BTreeMap::new(),
        external: BTreeMap::new(),
    };
    let mut extension_roots = Vec::new();
    for (rel, package) in &roots.extensions {
        graph.add(rel, package)?;
        extension_roots.push(rel.clone());
    }
    let mut aspect_roots = Vec::new();
    let bazelrc = bazelrc_roots(workspace)?;
    let mut aspects = aspect_labels.to_vec();
    aspects.extend(bazelrc.aspects);
    for aspect in &aspects {
        let file = aspect.split('%').next().unwrap_or(aspect);
        if let Some((rel, package)) = label_path(file, "", &roots.names) {
            graph.add(&rel, &package)?;
            aspect_roots.push(rel);
        }
    }
    let builds = build_files(workspace)?;
    for (rel, package) in &builds {
        graph.add(rel, package)?;
    }
    let mut toolchain_roots = Vec::new();
    let patterns = roots
        .toolchains
        .iter()
        .map(|(pattern, package)| (pattern.as_str(), package.as_str()))
        .chain(
            bazelrc
                .toolchains
                .iter()
                .map(|pattern| (pattern.as_str(), "")),
        );
    for (pattern, package) in patterns {
        toolchain_roots.extend(pattern_builds(pattern, package, &roots.names, &builds));
    }
    toolchain_roots.extend(implementation_roots(&mut graph, &toolchain_roots.clone())?);
    let extension_inputs = graph.closure(&extension_roots);
    let aspect_inputs = graph.closure(&aspect_roots);
    let toolchain_inputs = graph.closure(&toolchain_roots);
    let mut loaders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (rel, targets) in &graph.loads {
        for target in targets {
            loaders.entry(target.clone()).or_default().push(rel.clone());
        }
    }
    let packages = builds.into_iter().collect();
    Ok(ImpactGraph {
        graph,
        aspects,
        bazelrc_repos: bazelrc.repos,
        extension_inputs,
        aspect_inputs,
        toolchain_inputs,
        loaders,
        packages,
    })
}

/// Double-quoted string literals of `source`.
fn string_literals(source: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find('"') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('"') else { break };
        found.push(&tail[..end]);
        rest = &tail[end + 1..];
    }
    found
}

/// Labels given as `toolchain = "..."` in `source`.
fn toolchain_attr_labels(source: &str) -> Vec<&str> {
    let mut found = Vec::new();
    for (at, _) in source.match_indices("toolchain") {
        let before = source[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let tail = source[at + "toolchain".len()..].trim_start();
        let Some(tail) = tail.strip_prefix('=') else {
            continue;
        };
        let Some(tail) = tail.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = tail.find('"') {
            found.push(&tail[..end]);
        }
    }
    found
}

/// Extra roots for the rule implementations behind `toolchain()` targets: the
/// package of each workspace `toolchain =` label, and for each external one the
/// loaded .bzl files that name the repository, with the .bzl files they reference
/// through `Label(...)` (a repository rule can generate a BUILD that loads them).
fn implementation_roots(
    graph: &mut LoadGraph<'_>,
    toolchain_builds: &[String],
) -> Result<Vec<String>, String> {
    let mut roots = Vec::new();
    let mut repos: BTreeSet<String> = BTreeSet::new();
    for rel in toolchain_builds {
        let Some(source) = read(graph.workspace, rel)? else {
            continue;
        };
        let package = rel.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
        for label in toolchain_attr_labels(&source) {
            if let Some((target, _)) = label_path(label, &package, &graph.names) {
                let dir = target.rsplit_once('/').map_or("", |(dir, _)| dir);
                for name in ["BUILD.bazel", "BUILD"] {
                    let build = if dir.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{dir}/{name}")
                    };
                    if graph.workspace.join(&build).is_file() {
                        graph.add(&build, dir)?;
                        roots.push(build);
                    }
                }
            } else if let Some(repo) = label_repo(label) {
                repos.insert(repo.to_owned());
            }
        }
    }
    if repos.is_empty() {
        return Ok(roots);
    }
    let candidates: Vec<String> = graph
        .loads
        .keys()
        .filter(|rel| rel.ends_with(".bzl"))
        .cloned()
        .collect();
    for rel in candidates {
        let Some(source) = read(graph.workspace, &rel)? else {
            continue;
        };
        let literals = string_literals(&source);
        if !literals
            .iter()
            .any(|lit| repos.iter().any(|repo| repo_matches(lit, repo)))
        {
            continue;
        }
        let dir = rel.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
        roots.push(rel.clone());
        for lit in literals {
            if !lit.ends_with(".bzl") {
                continue;
            }
            if let Some((target, package)) = label_path(lit, &dir, &graph.names) {
                graph.add(&target, &package)?;
                roots.push(target);
            }
        }
    }
    Ok(roots)
}

impl ImpactGraph<'_> {
    fn loading_packages(&self, starts: Vec<&str>) -> BTreeSet<String> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending = starts;
        let mut selected = BTreeSet::new();
        while let Some(rel) = pending.pop() {
            if !seen.insert(rel) {
                continue;
            }
            if let Some(package) = self.packages.get(rel) {
                selected.insert(format!("//{package}:all"));
            }
            if let Some(from) = self.loaders.get(rel) {
                pending.extend(from.iter().map(String::as_str));
            }
        }
        selected
    }

    fn loads_repo(&self, inputs: &BTreeSet<String>, repo: &str) -> bool {
        inputs.iter().any(|rel| {
            self.graph
                .external
                .get(rel)
                .is_some_and(|repos| repos.iter().any(|name| repo_matches(name, repo)))
        })
    }
}

/// The packages that load each changed .bzl file, or why it affects the whole workspace.
pub fn bzl_impact(
    workspace: &Path,
    changed_bzl: &[String],
    aspect_labels: &[String],
) -> Result<BTreeMap<String, BzlImpact>, String> {
    let graph = impact_graph(workspace, aspect_labels)?;
    let mut impact = BTreeMap::new();
    for path in changed_bzl {
        let effect = if graph.extension_inputs.contains(path) {
            BzlImpact::Global(format!("module extension input {path}"))
        } else if graph.aspect_inputs.contains(path) {
            BzlImpact::Global(format!("aspect input {path}"))
        } else if graph.toolchain_inputs.contains(path) {
            BzlImpact::Global(format!("toolchain input {path}"))
        } else {
            BzlImpact::Packages(graph.loading_packages(vec![path.as_str()]))
        };
        impact.insert(path.clone(), effect);
    }
    Ok(impact)
}

/// The packages that load from each external repository, or why it affects the whole workspace.
pub fn repo_impact(
    workspace: &Path,
    repos: &[String],
    aspect_labels: &[String],
) -> Result<BTreeMap<String, BzlImpact>, String> {
    let graph = impact_graph(workspace, aspect_labels)?;
    let mut impact = BTreeMap::new();
    for repo in repos {
        let effect = if graph.loads_repo(&graph.extension_inputs, repo) {
            BzlImpact::Global(format!("module extension input loads @{repo}"))
        } else if graph.loads_repo(&graph.aspect_inputs, repo)
            || graph
                .aspects
                .iter()
                .any(|label| label_repo(label).is_some_and(|name| repo_matches(name, repo)))
        {
            BzlImpact::Global(format!("aspect loads @{repo}"))
        } else if graph.loads_repo(&graph.toolchain_inputs, repo) {
            BzlImpact::Global(format!("toolchain input loads @{repo}"))
        } else if graph
            .bazelrc_repos
            .iter()
            .any(|name| repo_matches(name, repo))
        {
            BzlImpact::Global(format!("bazelrc names @{repo}"))
        } else {
            let starts = graph
                .graph
                .external
                .iter()
                .filter(|(_, names)| names.iter().any(|name| repo_matches(name, repo)))
                .map(|(rel, _)| rel.as_str())
                .collect();
            BzlImpact::Packages(graph.loading_packages(starts))
        };
        impact.insert(repo.clone(), effect);
    }
    Ok(impact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::write;

    fn impact(root: &Path, changed: &[&str], aspects: &[&str]) -> Vec<(String, BzlImpact)> {
        let changed: Vec<String> = changed.iter().map(ToString::to_string).collect();
        let aspects: Vec<String> = aspects.iter().map(ToString::to_string).collect();
        bzl_impact(root, &changed, &aspects)
            .expect("load graph")
            .into_iter()
            .collect()
    }

    fn global(path: &str, reason: &str) -> (String, BzlImpact) {
        (
            path.to_owned(),
            BzlImpact::Global(format!("{reason} {path}")),
        )
    }

    fn packages(path: &str, packages: &[&str]) -> (String, BzlImpact) {
        (
            path.to_owned(),
            BzlImpact::Packages(packages.iter().map(ToString::to_string).collect()),
        )
    }

    #[test]
    fn extension_files_name_direct_extensions_and_repo_rules() {
        let scratch = dx_test_scratch::scratch("dx-ext-direct-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools/ext:x.bzl\", \"x\")\nr = use_repo_rule(repo_rule_bzl_file = \"@@//tools:repo.bzl\", repo_rule_name = \"r\")\ny = use_extension(\":root.bzl\", \"y\")\n",
        );
        write(root, "tools/ext/x.bzl", "X = 1\n");
        write(root, "tools/repo.bzl", "R = 1\n");
        write(root, "root.bzl", "Y = 1\n");
        write(root, "other.bzl", "O = 1\n");
        assert_eq!(
            impact(
                root,
                &["other.bzl", "root.bzl", "tools/ext/x.bzl", "tools/repo.bzl"],
                &[]
            ),
            [
                packages("other.bzl", &[]),
                global("root.bzl", "module extension input"),
                global("tools/ext/x.bzl", "module extension input"),
                global("tools/repo.bzl", "module extension input"),
            ]
        );
    }

    #[test]
    fn extension_files_follow_helper_loads() {
        let scratch = dx_test_scratch::scratch("dx-ext-helper-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\", repo_name = \"own\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\n",
        );
        write(
            root,
            "tools/ext.bzl",
            "load(\":helper.bzl\", \"h\")\nload(\"@demo//lib:shared.bzl\", \"s\")\nload(\"@own//lib:own.bzl\", \"o\")\nload(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\n",
        );
        write(
            root,
            "tools/helper.bzl",
            "load(\"//lib:deep.bzl\", \"d\")\n",
        );
        write(root, "lib/shared.bzl", "S = 1\n");
        write(root, "lib/own.bzl", "O = 1\n");
        write(root, "lib/deep.bzl", "D = 1\n");
        write(root, "lib/unused.bzl", "U = 1\n");
        assert_eq!(
            impact(
                root,
                &[
                    "lib/deep.bzl",
                    "lib/own.bzl",
                    "lib/shared.bzl",
                    "lib/unused.bzl",
                    "tools/helper.bzl"
                ],
                &[]
            ),
            [
                global("lib/deep.bzl", "module extension input"),
                global("lib/own.bzl", "module extension input"),
                global("lib/shared.bzl", "module extension input"),
                packages("lib/unused.bzl", &[]),
                global("tools/helper.bzl", "module extension input"),
            ]
        );
    }

    #[test]
    fn extension_files_read_included_module_files() {
        let scratch = dx_test_scratch::scratch("dx-ext-include-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\ninclude(\"//deps:go.MODULE.bazel\")\n",
        );
        write(
            root,
            "deps/go.MODULE.bazel",
            "go = use_extension(\"//deps:go.bzl\", \"go\")\n",
        );
        write(root, "deps/go.bzl", "G = 1\n");
        assert_eq!(
            impact(root, &["deps/go.bzl"], &[]),
            [global("deps/go.bzl", "module extension input")]
        );
    }

    #[test]
    fn extension_files_skip_external_extensions_and_missing_module() {
        let scratch = dx_test_scratch::scratch("dx-ext-external-");
        let root = scratch.path();
        write(root, "rust/extensions.bzl", "R = 1\n");
        assert_eq!(
            impact(root, &["rust/extensions.bzl"], &[]),
            [packages("rust/extensions.bzl", &[])]
        );
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\n",
        );
        assert_eq!(
            impact(root, &["rust/extensions.bzl"], &[]),
            [packages("rust/extensions.bzl", &[])]
        );
    }

    #[test]
    fn extension_files_fail_on_parse_errors() {
        let scratch = dx_test_scratch::scratch("dx-ext-error-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\n",
        );
        write(root, "tools/ext.bzl", "load(\":a.bzl\"\n");
        let changed = ["tools/a.bzl".to_owned()];
        let error = bzl_impact(root, &changed, &[]).expect_err("parse error");
        assert!(error.starts_with("tools/ext.bzl: "), "{error}");
        write(root, "MODULE.bazel", "module(name = \n");
        let error = bzl_impact(root, &changed, &[]).expect_err("module parse error");
        assert!(error.starts_with("MODULE.bazel: "), "{error}");
    }

    fn input_paths(root: &Path) -> (Vec<String>, bool) {
        let inputs = module_inputs(root).expect("module inputs");
        let mut paths: Vec<String> = inputs.files.into_iter().map(|file| file.path).collect();
        paths.sort();
        (paths, inputs.complete)
    }

    #[test]
    fn module_inputs_collect_tag_repo_rule_override_and_include_labels() {
        let scratch = dx_test_scratch::scratch("dx-module-inputs-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            concat!(
                "module(name = \"demo\")\n",
                "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n",
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n",
                "npm.npm_translate_lock(name = \"npm\", pnpm_lock = \"//:pnpm-lock.yaml\", data = [\"@demo//web:package.json\", \"@other//:x.json\"], dev = True)\n",
                "pip = use_extension(\"@rules_python//python/extensions:pip.bzl\", \"pip\")\n",
                "pip.parse(hub_name = \"pypi\", python_version = \"3.11\", requirements_by_platform = {\"//py:r.txt\": \"linux_*\"})\n",
                "archive = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\n",
                "archive(name = \"zlib\", build_file = \"@@//third_party:zlib.BUILD\", urls = [\"https://example.com/z.tgz\"])\n",
                "archive_override(module_name = \"rules_go\", patches = [\"@//patches:go.patch\"])\n",
                "register_toolchains(\"//toolchains:all\")\n",
                "use_repo(npm, \"npm\")\n",
                "include(\"//deps:go.MODULE.bazel\")\n",
            ),
        );
        write(
            root,
            "deps/go.MODULE.bazel",
            "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \":go.mod\")\n",
        );
        assert_eq!(
            input_paths(root),
            (
                [
                    "deps/go.MODULE.bazel",
                    "deps/go.mod",
                    "patches/go.patch",
                    "pnpm-lock.yaml",
                    "py/r.txt",
                    "third_party/zlib.BUILD",
                    "web/package.json",
                ]
                .map(str::to_owned)
                .to_vec(),
                true
            )
        );
        let inputs = module_inputs(root).expect("module inputs");
        let lock = inputs
            .files
            .iter()
            .find(|file| file.path == "pnpm-lock.yaml")
            .expect("pnpm lock");
        assert_eq!(
            lock,
            &ModuleInput {
                path: "pnpm-lock.yaml".to_owned(),
                extension: Some((
                    "@aspect_rules_js//npm:extensions.bzl".to_owned(),
                    "npm".to_owned()
                )),
                tag: "npm_translate_lock".to_owned(),
                hub: Some("npm".to_owned()),
            }
        );
        let requirements = inputs
            .files
            .iter()
            .find(|file| file.path == "py/r.txt")
            .expect("requirements");
        assert_eq!(requirements.hub.as_deref(), Some("pypi"));
        let build_file = inputs
            .files
            .iter()
            .find(|file| file.path == "third_party/zlib.BUILD")
            .expect("build file");
        assert_eq!(build_file.tag, "archive");
    }

    #[test]
    fn module_inputs_are_incomplete_for_computed_values_or_missing_module() {
        let scratch = dx_test_scratch::scratch("dx-module-inputs-computed-");
        let root = scratch.path();
        assert_eq!(input_paths(root), (Vec::new(), false));
        for tag in [
            "npm.npm_translate_lock(pnpm_lock = LOCK)",
            "npm.npm_translate_lock(pnpm_lock = \"//:\" + \"pnpm-lock.yaml\")",
            "npm.npm_translate_lock(**KW)",
        ] {
            write(
                root,
                "MODULE.bazel",
                &format!(
                    "module(name = \"demo\")\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n{tag}\n"
                ),
            );
            assert_eq!(input_paths(root), (Vec::new(), false), "{tag}");
        }
        write(root, "MODULE.bazel", "module(name = \"demo\")\nbazel_dep(name = \"x\", version = VERSION)\nother.tag(lock = \"//:x.lock\")\n");
        assert_eq!(input_paths(root), (Vec::new(), true));
    }

    #[test]
    fn module_inputs_fail_on_parse_errors_and_missing_includes() {
        let scratch = dx_test_scratch::scratch("dx-module-inputs-error-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \n");
        let error = module_inputs(root).expect_err("parse error");
        assert!(error.starts_with("MODULE.bazel: "), "{error}");
        write(
            root,
            "MODULE.bazel",
            "include(\"//deps:gone.MODULE.bazel\")\n",
        );
        assert_eq!(
            module_inputs(root),
            Err("deps/gone.MODULE.bazel is included but missing".to_owned())
        );
    }

    #[test]
    fn load_graph_selects_direct_and_transitive_loaders() {
        let scratch = dx_test_scratch::scratch("dx-load-transitive-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "tools/BUILD.bazel", "");
        write(root, "tools/a.bzl", "load(\":b.bzl\", \"b\")\n");
        write(root, "tools/b.bzl", "B = 1\n");
        write(root, "app/BUILD.bazel", "load(\"//tools:a.bzl\", \"a\")\n");
        write(root, "lib/BUILD", "load(\"//tools:b.bzl\", \"b\")\n");
        write(root, "BUILD.bazel", "load(\"//tools:a.bzl\", \"a\")\n");
        assert_eq!(
            impact(root, &["tools/a.bzl", "tools/b.bzl"], &[]),
            [
                packages("tools/a.bzl", &["//:all", "//app:all"]),
                packages("tools/b.bzl", &["//:all", "//app:all", "//lib:all"]),
            ]
        );
    }

    #[test]
    fn load_graph_selects_loaders_of_scl_files() {
        let scratch = dx_test_scratch::scratch("dx-load-scl-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "tools/BUILD.bazel", "");
        write(root, "tools/a.bzl", "load(\":x.scl\", \"x\")\n");
        write(root, "tools/x.scl", "X = 1\n");
        write(root, "app/BUILD.bazel", "load(\"//tools:a.bzl\", \"a\")\n");
        write(root, "lib/BUILD", "load(\"//tools:x.scl\", \"x\")\n");
        assert_eq!(
            impact(root, &["tools/x.scl"], &[]),
            [packages("tools/x.scl", &["//app:all", "//lib:all"])]
        );
    }

    #[test]
    fn relative_loads_in_a_bzl_resolve_against_its_label_package() {
        let scratch = dx_test_scratch::scratch("dx-load-relative-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "tools/BUILD.bazel", "");
        write(root, "tools/sub/a.bzl", "load(\":helper.bzl\", \"h\")\n");
        write(root, "tools/helper.bzl", "H = 1\n");
        write(root, "tools/sub/helper.bzl", "H = 2\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//tools:sub/a.bzl\", \"a\")\n",
        );
        assert_eq!(
            impact(root, &["tools/helper.bzl", "tools/sub/helper.bzl"], &[]),
            [
                packages("tools/helper.bzl", &["//app:all"]),
                packages("tools/sub/helper.bzl", &[]),
            ]
        );
    }

    #[test]
    fn walk_skips_ignored_dirs_and_symlinks() {
        let scratch = dx_test_scratch::scratch("dx-load-skip-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, ".bazelignore", "# comment\n./node_modules/\n");
        write(root, "defs.bzl", "D = 1\n");
        write(
            root,
            "node_modules/x/BUILD.bazel",
            "load(\"//:defs.bzl\", \"d\")\n",
        );
        write(root, "real/BUILD.bazel", "load(\"//:defs.bzl\", \"d\")\n");
        write(root, "nested/MODULE.bazel", "module(name = \"nested\")\n");
        write(
            root,
            "nested/BUILD.bazel",
            "load(\"@demo//:defs.bzl\", \"d\")\n",
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real"), root.join("bazel-out")).expect("symlink");
        assert_eq!(
            impact(root, &["defs.bzl"], &[]),
            [packages("defs.bzl", &["//nested:all", "//real:all"])]
        );
    }

    #[test]
    fn cuda_sources_are_detected_outside_ignored_dirs() {
        let scratch = dx_test_scratch::scratch("dx-cuda-detect-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "a.cc", "");
        assert!(!super::workspace_has_cuda_sources(root).expect("scan"));
        write(root, ".bazelignore", "vendor\n");
        write(root, "vendor/k.cu", "");
        write(root, ".dx/k.cu", "");
        write(root, ".git/k.cu", "");
        assert!(!super::workspace_has_cuda_sources(root).expect("scan"));
        #[cfg(unix)]
        {
            let outside = dx_test_scratch::scratch("dx-cuda-outside-");
            write(outside.path(), "k.cu", "");
            std::os::unix::fs::symlink(outside.path(), root.join("bazel-out")).expect("symlink");
            assert!(!super::workspace_has_cuda_sources(root).expect("scan"));
            outside.close().expect("cleanup");
        }
        write(root, "k/a.cu", "");
        assert!(super::workspace_has_cuda_sources(root).expect("scan"));
    }

    #[test]
    fn cuh_alone_counts_as_cuda() {
        let scratch = dx_test_scratch::scratch("dx-cuda-cuh-");
        let root = scratch.path();
        write(root, "include/k.cuh", "");
        assert!(super::workspace_has_cuda_sources(root).expect("scan"));
    }

    #[test]
    fn aspects_from_bazelrc_and_caller_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-aspects-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            ".bazelrc",
            "import %workspace%/tools/preset.bazelrc\ntry-import %workspace%/missing.bazelrc\n",
        );
        write(
            root,
            "tools/preset.bazelrc",
            "build --aspects=//rc:aspect.bzl%rc_aspect,@rules_x//:x.bzl%x # lint\n",
        );
        write(root, "rc/aspect.bzl", "load(\":rc_helper.bzl\", \"h\")\n");
        write(root, "rc/rc_helper.bzl", "H = 1\n");
        write(root, "quality/aspects.bzl", "A = 1\n");
        write(root, "plain.bzl", "P = 1\n");
        assert_eq!(
            impact(
                root,
                &["plain.bzl", "quality/aspects.bzl", "rc/rc_helper.bzl"],
                &["@demo//quality:aspects.bzl%lint_aspect"]
            ),
            [
                packages("plain.bzl", &[]),
                global("quality/aspects.bzl", "aspect input"),
                global("rc/rc_helper.bzl", "aspect input"),
            ]
        );
    }

    #[test]
    fn toolchain_packages_from_module_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchains-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\n    \"//tc:all\",\n    dev_dependency = True,\n)\nregister_execution_platforms(\":plat\")\n",
        );
        write(root, "tc/BUILD.bazel", "load(\"//rules:tc.bzl\", \"t\")\n");
        write(root, "rules/tc.bzl", "load(\":helper.bzl\", \"h\")\n");
        write(root, "rules/helper.bzl", "H = 1\n");
        write(root, "BUILD.bazel", "load(\"//rules:plat.bzl\", \"p\")\n");
        write(root, "rules/plat.bzl", "P = 1\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//rules:app.bzl\", \"a\")\n",
        );
        write(root, "rules/app.bzl", "A = 1\n");
        assert_eq!(
            impact(
                root,
                &[
                    "rules/app.bzl",
                    "rules/helper.bzl",
                    "rules/plat.bzl",
                    "rules/tc.bzl"
                ],
                &[]
            ),
            [
                packages("rules/app.bzl", &["//app:all"]),
                global("rules/helper.bzl", "toolchain input"),
                global("rules/plat.bzl", "toolchain input"),
                global("rules/tc.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn toolchain_implementation_bzl_widens() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-impl-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\"//tc:all\")\n",
        );
        write(
            root,
            "tc/BUILD.bazel",
            "toolchain(\n    name = \"a\",\n    toolchain = \"@hub//:a\",\n    toolchain_type = \"//m:type\",\n)\ntoolchain(\n    name = \"b\",\n    toolchain = \"//impl:b\",\n)\n",
        );
        write(root, "impl/BUILD.bazel", "load(\"//impl:b.bzl\", \"b\")\n");
        write(root, "impl/b.bzl", "B = 1\n");
        write(
            root,
            "m/repos.bzl",
            "HUB = \"hub\"\nTEXT = str(Label(\":tool.bzl\"))\n",
        );
        write(root, "m/tool.bzl", "T = 1\n");
        write(
            root,
            "m/BUILD.bazel",
            "load(\":repos.bzl\", \"HUB\")\nload(\":tool.bzl\", \"T\")\n",
        );
        write(root, "m/other.bzl", "O = 1\n");
        write(
            root,
            "m/user/BUILD.bazel",
            "load(\"//m:other.bzl\", \"O\")\n",
        );
        assert_eq!(
            impact(root, &["impl/b.bzl", "m/tool.bzl", "m/other.bzl"], &[]),
            [
                global("impl/b.bzl", "toolchain input"),
                packages("m/other.bzl", &["//m/user:all"]),
                global("m/tool.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn toolchain_patterns_from_includes_and_recursive_patterns() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-include-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\ninclude(\"//deps:tc.MODULE.bazel\")\n",
        );
        write(
            root,
            "deps/tc.MODULE.bazel",
            "register_toolchains(\"//tcs/...\", \"@other//:all\", \"-//tcs/skip:all\")\n",
        );
        write(root, ".bazelignore", "tcs/ignored\n");
        write(root, "tcs/a/BUILD.bazel", "load(\"//lib:a.bzl\", \"a\")\n");
        write(root, "tcs/a/b/BUILD", "load(\"//lib:b.bzl\", \"b\")\n");
        write(
            root,
            "tcs/ignored/BUILD.bazel",
            "load(\"//lib:ignored.bzl\", \"i\")\n",
        );
        write(root, "tcsx/BUILD.bazel", "load(\"//lib:x.bzl\", \"x\")\n");
        write(root, "lib/a.bzl", "A = 1\n");
        write(root, "lib/b.bzl", "B = 1\n");
        write(root, "lib/ignored.bzl", "I = 1\n");
        write(root, "lib/x.bzl", "X = 1\n");
        assert_eq!(
            impact(
                root,
                &["lib/a.bzl", "lib/b.bzl", "lib/ignored.bzl", "lib/x.bzl"],
                &[]
            ),
            [
                global("lib/a.bzl", "toolchain input"),
                global("lib/b.bzl", "toolchain input"),
                packages("lib/ignored.bzl", &[]),
                packages("lib/x.bzl", &["//tcsx:all"]),
            ]
        );
    }

    #[test]
    fn toolchains_from_bazelrc_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-rc-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            ".bazelrc",
            "import %workspace%/tools/preset.bazelrc\n",
        );
        write(
            root,
            "tools/preset.bazelrc",
            "common:windows --extra_execution_platforms=//plat:win,@platforms//host\nbuild --extra_toolchains //tc:x\nbuild --host_platform=//host # host\n",
        );
        write(
            root,
            "plat/BUILD.bazel",
            "load(\"//lib:plat.bzl\", \"p\")\n",
        );
        write(root, "tc/BUILD.bazel", "load(\"//lib:tc.bzl\", \"t\")\n");
        write(
            root,
            "host/BUILD.bazel",
            "load(\"//lib:host.bzl\", \"h\")\n",
        );
        write(root, "app/BUILD.bazel", "load(\"//lib:app.bzl\", \"a\")\n");
        write(root, "lib/plat.bzl", "P = 1\n");
        write(root, "lib/tc.bzl", "T = 1\n");
        write(root, "lib/host.bzl", "H = 1\n");
        write(root, "lib/app.bzl", "A = 1\n");
        assert_eq!(
            impact(
                root,
                &["lib/app.bzl", "lib/host.bzl", "lib/plat.bzl", "lib/tc.bzl"],
                &[]
            ),
            [
                packages("lib/app.bzl", &["//app:all"]),
                global("lib/host.bzl", "toolchain input"),
                global("lib/plat.bzl", "toolchain input"),
                global("lib/tc.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn computed_toolchain_pattern_fails() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-computed-");
        let root = scratch.path();
        let changed = ["defs.bzl".to_owned()];
        for call in [
            "register_toolchains(*TCS)",
            "register_toolchains(NAME)",
            "register_execution_platforms(\"//p:all\", NAME)",
        ] {
            write(
                root,
                "MODULE.bazel",
                &format!("module(name = \"demo\")\n{call}\n"),
            );
            let error = bzl_impact(root, &changed, &[]).expect_err("computed pattern");
            let name = call.split('(').next().unwrap_or_default();
            assert!(
                error.starts_with(&format!(
                    "MODULE.bazel: {name} at line 2 has no literal target pattern"
                )),
                "{error}"
            );
        }
    }

    #[test]
    fn deleted_bzl_still_matches_its_loaders() {
        let scratch = dx_test_scratch::scratch("dx-load-deleted-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//tools:gone.bzl\", \"g\")\n",
        );
        assert_eq!(
            impact(root, &["tools/gone.bzl"], &[]),
            [packages("tools/gone.bzl", &["//app:all"])]
        );
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\"//tc:all\")\n",
        );
        write(
            root,
            "tc/BUILD.bazel",
            "load(\"//tools:tc_gone.bzl\", \"t\")\n",
        );
        assert_eq!(
            impact(root, &["tools/gone.bzl", "tools/tc_gone.bzl"], &[]),
            [
                packages("tools/gone.bzl", &["//app:all"]),
                global("tools/tc_gone.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn build_parse_errors_fail() {
        let scratch = dx_test_scratch::scratch("dx-load-build-error-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "app/BUILD.bazel", "load(\"//:defs.bzl\"\n");
        let error = bzl_impact(root, &["defs.bzl".to_owned()], &[]).expect_err("parse error");
        assert!(error.starts_with("app/BUILD.bazel: "), "{error}");
    }

    fn repo_effects(root: &Path, repos: &[&str], aspects: &[&str]) -> Vec<(String, BzlImpact)> {
        let repos: Vec<String> = repos.iter().map(ToString::to_string).collect();
        let aspects: Vec<String> = aspects.iter().map(ToString::to_string).collect();
        repo_impact(root, &repos, &aspects)
            .expect("load graph")
            .into_iter()
            .collect()
    }

    fn repo_global(repo: &str, reason: &str) -> (String, BzlImpact) {
        (
            repo.to_owned(),
            BzlImpact::Global(format!("{reason} @{repo}")),
        )
    }

    #[test]
    fn repo_impact_selects_direct_and_transitive_loaders() {
        let scratch = dx_test_scratch::scratch("dx-repo-loaders-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "a/BUILD.bazel", "load(\"@leaf//:defs.bzl\", \"x\")\n");
        write(root, "b/BUILD.bazel", "load(\"//lib:wrap.bzl\", \"w\")\n");
        write(
            root,
            "lib/wrap.bzl",
            "load(\"@@leaf+//:defs.bzl\", \"x\")\n",
        );
        write(
            root,
            "c/BUILD.bazel",
            "load(\"@leaf_extra//:defs.bzl\", \"x\")\n",
        );
        write(
            root,
            "d/BUILD.bazel",
            "load(\"@@leaf+_x+y//:defs.bzl\", \"x\")\n",
        );
        assert_eq!(
            repo_effects(root, &["leaf", "leaf+", "unused"], &[]),
            [
                packages("leaf", &["//a:all"]),
                packages("leaf+", &["//b:all", "//d:all"]),
                packages("unused", &[]),
            ]
        );
    }

    #[test]
    fn repo_impact_widens_for_extension_aspect_toolchain_and_bazelrc() {
        let scratch = dx_test_scratch::scratch("dx-repo-global-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\nregister_toolchains(\"//tc:all\")\n",
        );
        write(root, "tools/ext.bzl", "load(\"@ext//:defs.bzl\", \"e\")\n");
        write(root, "tc/BUILD.bazel", "load(\"@tc//:defs.bzl\", \"t\")\n");
        write(root, "asp/aspect.bzl", "load(\"@asp//:defs.bzl\", \"a\")\n");
        write(
            root,
            ".bazelrc",
            "build --platforms=@plat//:p\nbuild:ci --@flag//:f=1\n",
        );
        assert_eq!(
            repo_effects(
                root,
                &["asp", "ext", "flag", "own", "plat", "tc"],
                &["//asp:aspect.bzl%a", "@own//:a.bzl%a"]
            ),
            [
                repo_global("asp", "aspect loads"),
                repo_global("ext", "module extension input loads"),
                repo_global("flag", "bazelrc names"),
                repo_global("own", "aspect loads"),
                repo_global("plat", "bazelrc names"),
                repo_global("tc", "toolchain input loads"),
            ]
        );
    }
}
