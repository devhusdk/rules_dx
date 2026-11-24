use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use dx_module_bazel::{Call, ModuleFile, Value};
use dx_path::label::repo_of;
use serde_json::{Map, Value as Json};

use super::extension_scope::{self, Names, Scope};

pub(crate) const BUILTIN_TOOLS_DEPS: &[&str] = &[
    "abseil-cpp",
    "apple_support",
    "bazel_features",
    "buildozer",
    "platforms",
    "protobuf",
    "rules_apple",
    "rules_cc",
    "rules_java",
    "rules_license",
    "rules_python",
    "rules_shell",
    "rules_swift",
    "zlib",
];

pub(crate) const BUILTIN_TOOLS: &str = "bazel_tools";

const OWNED_SECTIONS: &[&str] = &[
    "registryFileHashes",
    "selectedYankedVersions",
    "moduleExtensions",
    "facts",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LockChanges {
    pub(crate) modules: BTreeSet<String>,
    pub(crate) owned: Vec<(String, String)>,
    pub(crate) scopes: BTreeSet<Scope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegistryFile {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) file: String,
    pub(crate) hash: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleGraph {
    pub(crate) deps: BTreeMap<String, BTreeSet<String>>,
    pub(crate) registers: BTreeSet<String>,
    pub(crate) registered: BTreeMap<String, BTreeSet<Scope>>,
    pub(crate) usages: BTreeMap<String, BTreeSet<Scope>>,
    pub(crate) opaque: BTreeSet<String>,
}

pub(crate) fn lock_pins(
    lock: &str,
    updated: &BTreeMap<String, String>,
) -> Result<(), dx_error::DxError> {
    if updated.is_empty() {
        return Ok(());
    }
    let unpinned = || dx_error::DxError::failure("workspace input MODULE.bazel.lock");
    let lock = parse_lock(lock)?;
    let Some(Json::Object(hashes)) = lock.get("registryFileHashes") else {
        return Err(unpinned());
    };
    let pinned: BTreeSet<(&str, &str)> = hashes
        .iter()
        .filter(|(_, hash)| hash.as_str().is_some_and(|hash| hash != "not found"))
        .filter_map(|(url, _)| registry_url_parts(url))
        .filter(|(_, _, _, file)| *file == "MODULE.bazel")
        .map(|(_, name, version, _)| (name, version))
        .collect();
    if updated
        .iter()
        .all(|(name, version)| pinned.contains(&(name.as_str(), version.as_str())))
    {
        Ok(())
    } else {
        Err(unpinned())
    }
}

pub(crate) fn registry_url_parts(url: &str) -> Option<(&str, &str, &str, &str)> {
    let (registry, rest) = url.split_once("/modules/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [name, version, file] = parts.as_slice() else {
        return None;
    };
    let known = matches!(*file, "MODULE.bazel" | "source.json");
    (known && !name.is_empty() && !version.is_empty()).then_some((registry, name, version, file))
}

fn parse_lock(text: &str) -> Result<Map<String, Json>, dx_error::DxError> {
    match serde_json::from_str(text) {
        Ok(Json::Object(map)) => Ok(map),
        Ok(_) => Err(dx_error::DxError::infra(
            "cannot read MODULE.bazel.lock: not a JSON object".to_owned(),
        )),
        Err(error) => Err(dx_error::DxError::infra(format!(
            "cannot read MODULE.bazel.lock: {error}"
        ))),
    }
}

fn lock_changed(key: &str) -> dx_error::DxError {
    dx_error::DxError::failure(format!("MODULE.bazel.lock changed {key}"))
}

fn section<'a>(
    value: Option<&'a Json>,
    empty: &'a Map<String, Json>,
    key: &str,
) -> Result<&'a Map<String, Json>, dx_error::DxError> {
    match value {
        None => Ok(empty),
        Some(Json::Object(map)) => Ok(map),
        Some(_) => Err(lock_changed(key)),
    }
}

const EXTENSION_INPUT_DIGESTS: &[&str] = &["bzlTransitiveDigest", "usagesDigest"];

fn same_extension_result(before: Option<&Json>, after: Option<&Json>) -> bool {
    let (Some(Json::Object(before)), Some(Json::Object(after))) = (before, after) else {
        return false;
    };
    let results = |evaluations: &Map<String, Json>| -> Option<Vec<(String, Map<String, Json>)>> {
        evaluations
            .iter()
            .map(|(platform, evaluation)| {
                let mut evaluation = evaluation.as_object()?.clone();
                for digest in EXTENSION_INPUT_DIGESTS {
                    evaluation.remove(*digest);
                }
                Some((platform.clone(), evaluation))
            })
            .collect()
    };
    match (results(before), results(after)) {
        (Some(before), Some(after)) => before == after,
        _ => false,
    }
}

pub(crate) fn lock_changes(base: &str, head: &str) -> Result<LockChanges, dx_error::DxError> {
    let base = parse_lock(base)?;
    let head = parse_lock(head)?;
    let empty = Map::new();
    let mut changes = LockChanges::default();
    let keys: BTreeSet<&String> = base.keys().chain(head.keys()).collect();
    for key in keys {
        let (before, after) = (base.get(key), head.get(key));
        if !OWNED_SECTIONS.contains(&key.as_str()) {
            if before != after {
                return Err(lock_changed(key));
            }
            continue;
        }
        let (before, after) = (section(before, &empty, key)?, section(after, &empty, key)?);
        let entries: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
        for entry in entries {
            if before.get(entry) == after.get(entry) {
                continue;
            }
            if key == "moduleExtensions"
                && same_extension_result(before.get(entry), after.get(entry))
            {
                continue;
            }
            match key.as_str() {
                "registryFileHashes" => {
                    let (_, name, _, _) =
                        registry_url_parts(entry).ok_or_else(|| lock_changed(entry))?;
                    changes.modules.insert(name.to_owned());
                }
                "selectedYankedVersions" => {
                    let name = entry
                        .split_once('@')
                        .map(|(name, _)| name)
                        .filter(|name| !name.is_empty())
                        .ok_or_else(|| lock_changed(entry))?;
                    changes.owned.push((entry.clone(), name.to_owned()));
                }
                _ => {
                    let scope =
                        extension_scope::from_lock_key(entry).ok_or_else(|| lock_changed(entry))?;
                    changes.scopes.insert(scope);
                }
            }
        }
    }
    Ok(changes)
}

pub(crate) fn registry_files(lock: &str) -> Result<Vec<RegistryFile>, dx_error::DxError> {
    let lock = parse_lock(lock)?;
    let Some(Json::Object(hashes)) = lock.get("registryFileHashes") else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    for (url, hash) in hashes {
        let (Some((_, name, version, file)), Some(hash)) = (registry_url_parts(url), hash.as_str())
        else {
            continue;
        };
        if hash == "not found" {
            continue;
        }
        files.push(RegistryFile {
            name: name.to_owned(),
            version: version.to_owned(),
            file: file.to_owned(),
            hash: hash.to_owned(),
        });
    }
    let selected: BTreeSet<(String, String)> = files
        .iter()
        .filter(|file| file.file == "source.json")
        .map(|file| (file.name.clone(), file.version.clone()))
        .collect();
    let has_source: BTreeSet<&str> = selected.iter().map(|(name, _)| name.as_str()).collect();
    let keep = |file: &RegistryFile| {
        file.file == "MODULE.bazel"
            && (!has_source.contains(file.name.as_str())
                || selected.contains(&(file.name.clone(), file.version.clone())))
    };
    let kept = files.iter().filter(|file| keep(file)).cloned().collect();
    Ok(kept)
}

pub(crate) fn cached(caches: &[PathBuf], hash: &str) -> Option<String> {
    caches.iter().find_map(|cache| {
        let path = cache
            .join("content_addressable")
            .join("sha256")
            .join(hash)
            .join("file");
        let bytes = std::fs::read(path).ok()?;
        if dx_digest::sha256_hex(&bytes) != hash {
            return None;
        }
        String::from_utf8(bytes).ok()
    })
}

pub(crate) fn registers_toolchains(file: &ModuleFile) -> bool {
    file.calls().iter().any(|call| {
        matches!(
            call.name.as_str(),
            "register_toolchains" | "register_execution_platforms"
        )
    })
}

pub(crate) fn registered_repo(label: &str) -> Option<&str> {
    label
        .strip_prefix('@')
        .map(|rest| rest.trim_start_matches('@'))
        .and_then(|rest| rest.split_once("//"))
        .map(|(repo, _)| repo)
}

fn registered_scopes(module: &ModuleFile, name: &str) -> BTreeSet<Scope> {
    let files = std::slice::from_ref(module);
    let imported = super::module_diff::imported(files, name);
    let deps = Names::of(files, name).deps;
    let mut scopes = BTreeSet::new();
    for call in module.calls().iter().filter(|call| {
        matches!(
            call.name.as_str(),
            "register_toolchains" | "register_execution_platforms"
        ) && !matches!(call.arg("dev_dependency"), Some(Value::Ident(dev)) if dev == "True")
    }) {
        let labels: Vec<Option<&str>> = if call.unpacked {
            vec![None]
        } else {
            call.positional
                .iter()
                .map(|value| {
                    if let Value::Str(label) = value {
                        Some(label.value.as_str())
                    } else {
                        None
                    }
                })
                .collect()
        };
        for repo in labels
            .into_iter()
            .map(|label| label.and_then(registered_repo))
        {
            if let Some(scope) = repo.and_then(|repo| imported.get(repo)) {
                scopes.insert(scope.clone());
                continue;
            }
            if let Some(dep) = repo.and_then(|repo| deps.get(repo)) {
                scopes.insert(Scope::Module(dep.clone()));
            }
            scopes.insert(Scope::Module(name.to_owned()));
            scopes.extend(imported.values().cloned());
        }
    }
    scopes
}

pub(crate) fn module_graph(
    files: &[RegistryFile],
    read: &dyn Fn(&str) -> Option<String>,
    multiple: &BTreeSet<String>,
) -> Result<ModuleGraph, dx_error::DxError> {
    let mut graph = ModuleGraph::default();
    graph.deps.insert(
        BUILTIN_TOOLS.to_owned(),
        BUILTIN_TOOLS_DEPS
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
    );
    graph.registers.insert(BUILTIN_TOOLS.to_owned());
    graph.registered.insert(
        BUILTIN_TOOLS.to_owned(),
        BTreeSet::from([Scope::Module(BUILTIN_TOOLS.to_owned())]),
    );
    for file in files {
        let id = format!("{}@{}", file.name, file.version);
        let text = read(&file.hash).ok_or_else(|| {
            dx_error::DxError::failure(format!("no cached MODULE.bazel for {id}"))
        })?;
        let module = ModuleFile::parse(&text).map_err(|error| {
            dx_error::DxError::infra(format!("cannot read MODULE.bazel of {id}: {error}"))
        })?;
        let deps = graph.deps.entry(file.name.clone()).or_default();
        for call in module.calls_named("bazel_dep") {
            let name = call.str("name").ok_or_else(|| {
                dx_error::DxError::infra(format!(
                    "cannot read MODULE.bazel of {id}: computed bazel_dep"
                ))
            })?;
            deps.insert(name.to_owned());
        }
        if registers_toolchains(&module) {
            graph.registers.insert(file.name.clone());
        }
        let registered = registered_scopes(&module, &file.name);
        if !registered.is_empty() {
            graph.registered.insert(file.name.clone(), registered);
        }
        let mut names = Names::of(std::slice::from_ref(&module), &file.name);
        names.multiple.clone_from(multiple);
        for call in module.calls_named("use_extension").filter(
            |call| !matches!(call.arg("dev_dependency"), Some(Value::Ident(dev)) if dev == "True"),
        ) {
            let bzl = binding_label(call, "extension_bzl_file");
            let name = match call
                .positional
                .get(1)
                .or_else(|| call.arg("extension_name"))
            {
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                _ => None,
            };
            match bzl
                .zip(name)
                .and_then(|(bzl, name)| names.extension(&bzl, name))
            {
                Some(scope) => {
                    graph
                        .usages
                        .entry(file.name.clone())
                        .or_default()
                        .insert(scope);
                }
                None => {
                    graph.opaque.insert(file.name.clone());
                }
            }
        }
    }
    Ok(graph)
}

pub(crate) fn other_user(
    graph: &ModuleGraph,
    scope: &Scope,
    allowed: &BTreeSet<String>,
) -> Option<String> {
    graph
        .usages
        .iter()
        .find(|(module, used)| {
            !allowed.contains(*module) && used.iter().any(|known| known.overlaps(scope))
        })
        .map(|(module, _)| format!("module {module} also uses {}", scope.display()))
        .or_else(|| {
            graph
                .opaque
                .iter()
                .find(|module| !allowed.contains(*module))
                .map(|module| format!("module {module} uses an extension dx cannot name"))
        })
}

pub(crate) fn closure(
    changed: &BTreeSet<String>,
    deps: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut affected = changed.clone();
    loop {
        let before = affected.len();
        for (module, uses) in deps {
            if !affected.contains(module) && uses.iter().any(|dep| affected.contains(dep)) {
                affected.insert(module.clone());
            }
        }
        if affected.len() == before {
            return affected;
        }
    }
}

fn label_module(label: &str, deps: &BTreeMap<String, String>, own: &[String]) -> String {
    match repo_of(label) {
        None | Some("") => String::new(),
        Some(repo) if own.iter().any(|name| name == repo) => String::new(),
        Some(repo) => deps.get(repo).cloned().unwrap_or_else(|| repo.to_owned()),
    }
}

fn binding_label(call: &Call, key: &str) -> Option<String> {
    match call.positional.first().or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Some(literal.value.clone()),
        _ => None,
    }
}

pub(crate) fn repo_owners(files: &[ModuleFile]) -> BTreeMap<String, String> {
    let mut deps: BTreeMap<String, String> = BTreeMap::new();
    let mut own: Vec<String> = Vec::new();
    for file in files {
        for call in file.calls_named("module") {
            own.extend(
                [call.str("name"), call.str("repo_name")]
                    .into_iter()
                    .flatten()
                    .map(str::to_owned),
            );
        }
        for call in file.calls_named("bazel_dep") {
            let Some(name) = call.str("name") else {
                continue;
            };
            let repo = match call.arg("repo_name") {
                None => Some(name),
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                Some(_) => None,
            };
            if let Some(repo) = repo {
                deps.insert(repo.to_owned(), name.to_owned());
            }
        }
    }
    let mut owners = deps.clone();
    for file in files {
        let mut bindings: BTreeMap<&str, String> = BTreeMap::new();
        let mut rules: BTreeMap<&str, String> = BTreeMap::new();
        for call in file.calls() {
            let Some(binding) = call.binding.as_deref() else {
                continue;
            };
            let (map, key) = match call.name.as_str() {
                "use_extension" => (&mut bindings, "extension_bzl_file"),
                "use_repo_rule" => (&mut rules, "repo_rule_bzl_file"),
                _ => continue,
            };
            let module = binding_label(call, key)
                .map(|label| label_module(&label, &deps, &own))
                .unwrap_or_default();
            map.insert(binding, module);
        }
        for call in file.calls() {
            if call.name == "use_repo" {
                let Some(Value::Ident(binding)) = call.positional.first() else {
                    continue;
                };
                let Some(module) = bindings.get(binding.as_str()) else {
                    continue;
                };
                for value in call.positional.iter().skip(1) {
                    if let Value::Str(literal) = value {
                        owners.insert(literal.value.clone(), module.clone());
                    }
                }
                for (name, _) in &call.named {
                    owners.insert(name.clone(), module.clone());
                }
            } else if let Some(module) = rules.get(call.name.as_str())
                && let Some(name) = call.str("name")
            {
                owners.insert(name.to_owned(), module.clone());
            }
        }
    }
    owners
}

const GRAPH_EDITS: &[&str] = &["bazel_dep", "_override"];
const MODULE_EDITS: &[&str] = &["register_", "use_extension", "use_repo_rule", "include"];

fn module_file_edits(patch: &str) -> Vec<&str> {
    let mut module_file = false;
    let mut added = Vec::new();
    for line in patch.lines() {
        if let Some(target) = line.strip_prefix("+++ ") {
            module_file = target.split_whitespace().next().is_some_and(|path| {
                path.rsplit('/')
                    .next()
                    .is_some_and(|name| name.ends_with("MODULE.bazel"))
            });
        } else if module_file && let Some(line) = line.strip_prefix('+') {
            added.push(line);
        }
    }
    added
}

pub(crate) fn overrides_that_widen(
    files: &[ModuleFile],
    affected: &BTreeSet<String>,
    read: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    files.iter().flat_map(ModuleFile::calls).find_map(|call| {
        if !call.name.ends_with("_override") {
            return None;
        }
        let module = call.str("module_name").unwrap_or_default();
        let widen = |what: &str| Some(format!("MODULE.bazel {} for {module} {what}", call.name));
        if module.is_empty() || call.unpacked {
            return widen("has no literal module_name");
        }
        if matches!(
            call.name.as_str(),
            "git_override" | "archive_override" | "local_path_override"
        ) {
            return widen("hides its MODULE.bazel");
        }
        if call.arg("patch_cmds").is_some() {
            return widen("runs patch_cmds");
        }
        let patches = match call.arg("patches") {
            None => Vec::new(),
            Some(Value::List(items)) => items
                .iter()
                .map(|item| {
                    if let Value::Str(literal) = item {
                        Some(literal.value.as_str())
                    } else {
                        None
                    }
                })
                .collect(),
            Some(_) => vec![None],
        };
        for patch in patches {
            let text = patch
                .and_then(|label| dx_module_bazel::label_path(label).ok())
                .and_then(|path| read(&path));
            let Some(text) = text else {
                return widen("has an unreadable patch");
            };
            let edits = module_file_edits(&text);
            let edits_any = |keys: &[&str]| {
                edits
                    .iter()
                    .any(|line| keys.iter().any(|key| line.contains(key)))
            };
            if edits_any(GRAPH_EDITS) || (affected.contains(module) && edits_any(MODULE_EDITS)) {
                return widen("patches its MODULE.bazel");
            }
        }
        None
    })
}

fn repo_of_module(repo: &str, module: &str) -> bool {
    repo.starts_with(&format!("{module}+"))
}

fn root_owned(repo: &str) -> bool {
    repo.starts_with('+')
}

pub(crate) fn module_seeds<'a>(
    labels: impl Iterator<Item = &'a str>,
    modules: &BTreeSet<String>,
) -> Vec<String> {
    let mut seeds: Vec<String> = labels
        .filter(|label| {
            let Some((repo, _)) = label
                .strip_prefix("@@")
                .and_then(|rest| rest.split_once("//"))
            else {
                return false;
            };
            root_owned(repo) || modules.iter().any(|module| repo_of_module(repo, module))
        })
        .map(str::to_owned)
        .collect();
    seeds.sort();
    seeds.dedup();
    seeds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    fn files(text: &str) -> Vec<ModuleFile> {
        vec![ModuleFile::parse(text).expect("parses")]
    }

    #[test]
    fn lock_pins_requires_the_new_version_module_file() {
        let updated = BTreeMap::from([("leaf".to_owned(), "1.1".to_owned())]);
        let stale = lock(
            &format!(
                "\"{R}/modules/leaf/1.0/MODULE.bazel\": \"a\", \"{R}/modules/leaf/1.0/source.json\": \"b\""
            ),
            "",
        );
        let missing = lock(
            &format!("\"{R}/modules/leaf/1.1/MODULE.bazel\": \"not found\""),
            "",
        );
        let fresh = lock(
            &format!(
                "\"{R}/modules/leaf/1.1/MODULE.bazel\": \"a\", \"{R}/modules/leaf/1.2/source.json\": \"b\""
            ),
            "",
        );
        let error = Err(dx_error::DxError::failure(
            "workspace input MODULE.bazel.lock",
        ));
        assert_eq!(lock_pins(&stale, &updated), error);
        assert_eq!(lock_pins(&missing, &updated), error);
        assert_eq!(lock_pins("{}", &updated), error);
        assert_eq!(lock_pins(&fresh, &updated), Ok(()));
        assert_eq!(lock_pins(&stale, &BTreeMap::new()), Ok(()));
    }

    fn lock(hashes: &str, rest: &str) -> String {
        format!("{{\"lockFileVersion\": 24, \"registryFileHashes\": {{{hashes}}}{rest}}}")
    }

    const R: &str = "https://bcr.bazel.build";

    #[test]
    fn lock_changes_collect_modules_and_owned_keys() {
        let base = lock(
            &format!(
                "\"{R}/bazel_registry.json\": \"a\", \"{R}/modules/leaf/1.0/source.json\": \"b\""
            ),
            ", \"moduleExtensions\": {\"@@leaf+//:e.bzl%e\": {\"x\": 1}}",
        );
        let head = lock(
            &format!(
                "\"{R}/bazel_registry.json\": \"a\", \"{R}/modules/leaf/1.1/source.json\": \"c\", \"{R}/modules/dep/2.0/MODULE.bazel\": \"d\""
            ),
            ", \"moduleExtensions\": {\"@@leaf+//:e.bzl%e\": {\"x\": 2}}, \"selectedYankedVersions\": {\"old@1.0\": \"why\"}, \"facts\": {\"@@tool+//:f.bzl%f\": 1}",
        );
        let changes = lock_changes(&base, &head).expect("narrows");
        assert_eq!(changes.modules, set(&["dep", "leaf"]));
        assert_eq!(
            changes.owned,
            vec![("old@1.0".to_owned(), "old".to_owned())]
        );
        assert_eq!(
            changes.scopes,
            BTreeSet::from([
                Scope::extension("leaf+", "e"),
                Scope::extension("tool+", "f"),
            ])
        );
    }

    #[test]
    fn lock_changes_skip_extensions_whose_only_change_is_an_input_digest() {
        let extension = |digest: &str, usages: &str, repo: &str| {
            lock(
                "",
                &format!(
                    ", \"moduleExtensions\": {{\"//qt:extensions.bzl%qt\": {{\"general\": {{\"bzlTransitiveDigest\": \"{digest}\", \"usagesDigest\": \"{usages}\", \"generatedRepoSpecs\": {{\"sdk\": \"{repo}\"}}}}}}}}"
                ),
            )
        };
        let base = extension("a", "u", "r1");
        assert_eq!(
            lock_changes(&base, &extension("b", "v", "r1")),
            Ok(LockChanges::default())
        );
        let qt = LockChanges {
            scopes: BTreeSet::from([Scope::extension("", "qt")]),
            ..LockChanges::default()
        };
        assert_eq!(
            lock_changes(&base, &extension("b", "u", "r2")),
            Ok(qt.clone())
        );
        assert_eq!(lock_changes(&lock("", ""), &base), Ok(qt));
    }

    #[test]
    fn lock_changes_reject_unknown_keys() {
        let base = lock("", "");
        for (head, key) in [
            (
                lock("", ", \"moduleExtensions\": {\"//:e.bzl%e%x\": 1}"),
                "//:e.bzl%e%x",
            ),
            (lock("", ", \"factsVersions\": {}"), "factsVersions"),
            (
                lock(&format!("\"{R}/bazel_registry.json\": \"a\""), ""),
                "https://bcr.bazel.build/bazel_registry.json",
            ),
            (
                lock("", ", \"moduleExtensions\": {\"@@tool~//:f.bzl%f\": 1}"),
                "@@tool~//:f.bzl%f",
            ),
            (
                lock(&format!("\"{R}/modules/x/1/extra\": \"a\""), ""),
                "https://bcr.bazel.build/modules/x/1/extra",
            ),
        ] {
            assert_eq!(lock_changes(&base, &head), Err(lock_changed(key)));
        }
        assert!(lock_changes("{", &base).is_err());
    }

    #[test]
    fn registry_url_parts_follow_the_registry_layout() {
        assert_eq!(
            registry_url_parts(&format!("{R}/modules/leaf/1.0/MODULE.bazel")),
            Some((R, "leaf", "1.0", "MODULE.bazel"))
        );
        assert_eq!(
            registry_url_parts(&format!("{R}/modules/leaf/1.0/x.patch")),
            None
        );
        assert_eq!(
            registry_url_parts(&format!("{R}/bazel_registry.json")),
            None
        );
    }

    #[test]
    fn registry_files_keep_selected_versions_and_skip_missing_files() {
        let text = lock(
            &format!(
                "\"{R}/modules/leaf/1.0/MODULE.bazel\": \"h1\", \"{R}/modules/leaf/1.1/MODULE.bazel\": \"h2\", \"{R}/modules/leaf/1.1/source.json\": \"s\", \"{R}/modules/git/1/MODULE.bazel\": \"h3\", \"{R}/modules/git/2/MODULE.bazel\": \"h4\", \"{R}/modules/gone/1/MODULE.bazel\": \"not found\""
            ),
            "",
        );
        let found: Vec<(String, String)> = registry_files(&text)
            .expect("parses")
            .into_iter()
            .map(|file| (file.name, file.hash))
            .collect();
        let expected = [("git", "h3"), ("git", "h4"), ("leaf", "h2")];
        assert_eq!(
            found,
            expected.map(|(name, hash)| (name.to_owned(), hash.to_owned()))
        );
    }

    #[test]
    fn cached_accepts_only_matching_hashes() {
        let scratch = dx_test_scratch::scratch("dx-module-cache-").unwrap();
        let cache = scratch.path().join("cache");
        let text = "module(name = \"leaf\")\n";
        let good = dx_digest::sha256_hex(text.as_bytes());
        let bad = dx_digest::sha256_hex(b"other");
        for hash in [&good, &bad] {
            let dir = cache.join("content_addressable/sha256").join(hash);
            std::fs::create_dir_all(&dir).expect("mkdir");
            std::fs::write(dir.join("file"), text).expect("write");
        }
        let caches = [scratch.path().join("missing"), cache];
        assert_eq!(cached(&caches, &good).as_deref(), Some(text));
        assert_eq!(cached(&caches, &bad), None);
    }

    fn file(name: &str, hash: &str) -> RegistryFile {
        RegistryFile {
            name: name.to_owned(),
            version: "1".to_owned(),
            file: "MODULE.bazel".to_owned(),
            hash: hash.to_owned(),
        }
    }

    #[test]
    fn module_graph_and_closure_follow_reverse_deps() {
        let texts: BTreeMap<&str, &str> = [
            ("leaf", "module(name = \"leaf\")\ne = use_extension(\"//:e.bzl\", \"e\")\n"),
            (
                "user",
                "bazel_dep(name = \"leaf\", version = \"1\")\ne = use_extension(\"@leaf//:e.bzl\", \"e\")\nx = use_extension(\"@nobody//:x.bzl\", \"x\")\n",
            ),
            (
                "top",
                "bazel_dep(name = \"user\", version = \"1\")\nbazel_dep(name = \"leaf\", version = \"1\")\ne = use_extension(\"@leaf//:e.bzl\", \"e\", dev_dependency = True)\n",
            ),
            ("other", "register_toolchains(\"//:t\")\n"),
        ]
        .into_iter()
        .collect();
        let read = |hash: &str| texts.get(hash).map(|text| (*text).to_owned());
        let listed = [
            file("leaf", "leaf"),
            file("user", "user"),
            file("top", "top"),
            file("other", "other"),
        ];
        let graph = module_graph(&listed, &read, &BTreeSet::new()).expect("graph");
        assert_eq!(graph.registers, set(&[BUILTIN_TOOLS, "other"]));
        assert_eq!(
            graph.usages,
            BTreeMap::from([
                (
                    "leaf".to_owned(),
                    BTreeSet::from([Scope::extension("leaf+", "e")])
                ),
                (
                    "user".to_owned(),
                    BTreeSet::from([Scope::extension("leaf+", "e")])
                ),
            ])
        );
        assert_eq!(graph.opaque, set(&["user"]));
        let e = Scope::extension("leaf+", "e");
        assert_eq!(
            other_user(&graph, &e, &BTreeSet::new()).as_deref(),
            Some("module leaf also uses @@leaf++e+*")
        );
        assert_eq!(
            other_user(&graph, &e, &set(&["leaf", "user"])).as_deref(),
            None
        );
        assert_eq!(
            other_user(&graph, &Scope::extension("top+", "x"), &set(&["leaf"])).as_deref(),
            Some("module user uses an extension dx cannot name")
        );
        assert_eq!(
            closure(&set(&["leaf"]), &graph.deps),
            set(&["leaf", "top", "user"])
        );
        assert!(closure(&set(&["rules_cc"]), &graph.deps).contains(BUILTIN_TOOLS));
        assert_eq!(
            module_graph(&[file("lost", "lost")], &read, &BTreeSet::new()),
            Err(dx_error::DxError::failure(
                "no cached MODULE.bazel for lost@1"
            ))
        );
        let broken = |_: &str| Some("bazel_dep(".to_owned());
        let error = module_graph(&[file("bad", "bad")], &broken, &BTreeSet::new())
            .expect_err("parse error");
        assert!(
            error
                .message()
                .starts_with("cannot read MODULE.bazel of bad@1: "),
            "{error}"
        );
    }

    #[test]
    fn repo_owners_follow_deps_extensions_and_repo_rules() {
        let owners = repo_owners(&files(
            "module(name = \"root\")\nbazel_dep(name = \"rules_go\", version = \"1\", repo_name = \"go\")\nbazel_dep(name = \"leaf\", version = \"1\")\nbazel_dep(name = \"hidden\", version = \"1\", repo_name = None)\nsdk = use_extension(\"@go//:ext.bzl\", \"sdk\")\nuse_repo(sdk, \"go_sdk\", tools = \"go_tools\")\nmine = use_extension(\"//:ext.bzl\", \"mine\")\nuse_repo(mine, \"local\")\nhttp = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\nhttp(name = \"blob\")\n",
        ));
        let expected = [
            ("blob", "bazel_tools"),
            ("go", "rules_go"),
            ("go_sdk", "rules_go"),
            ("leaf", "leaf"),
            ("local", ""),
            ("tools", "rules_go"),
        ];
        assert_eq!(
            owners,
            expected
                .iter()
                .map(|(repo, module)| ((*repo).to_owned(), (*module).to_owned()))
                .collect()
        );
    }

    #[test]
    fn overrides_widen_only_when_they_hide_the_module_graph() {
        let patches: BTreeMap<&str, &str> = [
            ("p/plain.patch", "--- a/x.bzl\n+++ b/x.bzl\n+bazel_dep(\n"),
            (
                "p/dep.patch",
                "--- a/MODULE.bazel\n+++ b/MODULE.bazel\n+bazel_dep(name = \"x\")\n",
            ),
            (
                "p/register.patch",
                "--- a/MODULE.bazel\n+++ b/MODULE.bazel\n-register_toolchains(\"//:t\")\n+register_toolchains(\"//:t\", dev_dependency = True)\n",
            ),
        ]
        .into_iter()
        .collect();
        let read = |path: &str| patches.get(path).map(|text| (*text).to_owned());
        let widen = |text: &str, affected: &[&str]| {
            overrides_that_widen(&files(text), &set(affected), &read)
        };
        for (text, reason) in [
            (
                "git_override(module_name = \"a\", remote = \"r\")\n",
                "git_override for a hides its MODULE.bazel",
            ),
            (
                "archive_override(module_name = \"a\", urls = [])\n",
                "archive_override for a hides its MODULE.bazel",
            ),
            (
                "local_path_override(module_name = \"a\", path = \"x\")\n",
                "local_path_override for a hides its MODULE.bazel",
            ),
            (
                "single_version_override(module_name = \"a\", patch_cmds = [\"x\"])\n",
                "single_version_override for a runs patch_cmds",
            ),
            (
                "single_version_override(module_name = \"a\", patches = [\"//p:gone.patch\"])\n",
                "single_version_override for a has an unreadable patch",
            ),
            (
                "single_version_override(module_name = \"a\", patches = [\"//p:dep.patch\"])\n",
                "single_version_override for a patches its MODULE.bazel",
            ),
        ] {
            assert_eq!(widen(text, &[]), Some(format!("MODULE.bazel {reason}")));
        }
        let register = "single_version_override(module_name = \"a\", patches = [\"//p:register.patch\", \"//p:plain.patch\"])\n";
        assert_eq!(widen(register, &["b"]), None);
        assert_eq!(
            widen(register, &["a"]),
            Some("MODULE.bazel single_version_override for a patches its MODULE.bazel".to_owned())
        );
        assert_eq!(
            widen(
                "single_version_override(module_name = \"a\", version = \"1\")\n",
                &["a"]
            ),
            None
        );
    }

    #[test]
    fn module_seeds_match_canonical_prefixes_and_root_repos() {
        let labels = [
            "@@foo+//:a",
            "@@foo++ext+hub//:b",
            "@@foo_bar+//:c",
            "@@foo~//:d",
            "@@+ext+local//:e",
            "@@other+//:g",
            "//main:h",
        ];
        assert_eq!(
            module_seeds(labels.into_iter(), &set(&["foo"])),
            ["@@+ext+local//:e", "@@foo++ext+hub//:b", "@@foo+//:a",]
        );
    }
}
