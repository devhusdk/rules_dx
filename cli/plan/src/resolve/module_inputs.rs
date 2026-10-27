use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use dx_module_bazel::{Call, ModuleFile, Value};
use serde_json::{Map, Value as Json};

/// The modules Bazel's built-in `bazel_tools` depends on, from its `MODULE.tools`.
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

/// The module that stands for Bazel's built-in `bazel_tools` repository.
pub(crate) const BUILTIN_TOOLS: &str = "bazel_tools";

const OWNED_SECTIONS: &[&str] = &[
    "registryFileHashes",
    "selectedYankedVersions",
    "moduleExtensions",
    "facts",
];

/// The modules whose lock entries changed, and the changed lock keys each module owns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LockChanges {
    pub(crate) modules: BTreeSet<String>,
    pub(crate) owned: Vec<(String, String)>,
}

/// One registry file the lock pins: module, version, file name, and sha256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegistryFile {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) file: String,
    pub(crate) hash: String,
}

/// The dependency edges of every resolved module and the modules that register toolchains.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleGraph {
    pub(crate) deps: BTreeMap<String, BTreeSet<String>>,
    pub(crate) registers: BTreeSet<String>,
}

fn module_input_error() -> String {
    "workspace input MODULE.bazel".to_owned()
}

/// The `bazel_dep` names whose version changed, when nothing else in MODULE.bazel did.
pub(crate) fn bumped_deps(base: &str, head: &str) -> Result<BTreeSet<String>, String> {
    let base_file = ModuleFile::parse(base).map_err(|_| module_input_error())?;
    let head_file = ModuleFile::parse(head).map_err(|_| module_input_error())?;
    let base_versions = dep_versions(&base_file)?;
    let head_versions = dep_versions(&head_file)?;
    if base_versions.keys().ne(head_versions.keys()) {
        return Err(module_input_error());
    }
    if masked(base, &base_versions) != masked(head, &head_versions) {
        return Err(module_input_error());
    }
    Ok(base_versions
        .iter()
        .filter(|(name, version)| {
            head_versions.get(*name).map(|other| other.0.as_str()) != Some(version.0.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect())
}

type DepVersions = BTreeMap<String, (String, Option<std::ops::Range<usize>>)>;

fn dep_versions(file: &ModuleFile) -> Result<DepVersions, String> {
    let mut versions = DepVersions::new();
    for call in file.calls_named("bazel_dep") {
        if call.unpacked {
            return Err(module_input_error());
        }
        let Some(name) = call.str("name") else {
            return Err(module_input_error());
        };
        let version = match call.arg("version") {
            None => (String::new(), None),
            Some(Value::Str(literal)) => (literal.value.clone(), Some(literal.span.clone())),
            Some(_) => return Err(module_input_error()),
        };
        if versions.insert(name.to_owned(), version).is_some() {
            return Err(module_input_error());
        }
    }
    Ok(versions)
}

fn masked(text: &str, versions: &DepVersions) -> String {
    let mut spans: Vec<&std::ops::Range<usize>> = versions
        .values()
        .filter_map(|(_, span)| span.as_ref())
        .collect();
    spans.sort_by_key(|span| span.start);
    let mut out = String::new();
    let mut at = 0;
    for span in spans {
        out.push_str(text.get(at..span.start).unwrap_or_default());
        at = span.end;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}

/// The registry, module, version, and file of a `registryFileHashes` key.
pub(crate) fn registry_url_parts(url: &str) -> Option<(&str, &str, &str, &str)> {
    let (registry, rest) = url.split_once("/modules/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [name, version, file] = parts.as_slice() else {
        return None;
    };
    let known = matches!(*file, "MODULE.bazel" | "source.json");
    (known && !name.is_empty() && !version.is_empty()).then_some((registry, name, version, file))
}

/// The module that owns a `moduleExtensions` or `facts` key, from its `@@<m>+//` repo.
fn extension_owner(key: &str) -> Option<&str> {
    let repo = key.strip_prefix("@@")?.split_once("//")?.0;
    let name = repo.strip_suffix('+').or_else(|| repo.strip_suffix('~'))?;
    (!name.is_empty()).then_some(name)
}

fn parse_lock(text: &str) -> Result<Map<String, Json>, String> {
    match serde_json::from_str(text) {
        Ok(Json::Object(map)) => Ok(map),
        Ok(_) => Err("cannot read MODULE.bazel.lock: not a JSON object".to_owned()),
        Err(error) => Err(format!("cannot read MODULE.bazel.lock: {error}")),
    }
}

fn lock_changed(key: &str) -> String {
    format!("MODULE.bazel.lock changed {key}")
}

fn section<'a>(
    value: Option<&'a Json>,
    empty: &'a Map<String, Json>,
    key: &str,
) -> Result<&'a Map<String, Json>, String> {
    match value {
        None => Ok(empty),
        Some(Json::Object(map)) => Ok(map),
        Some(_) => Err(lock_changed(key)),
    }
}

/// The modules and module-owned keys that differ between two lock files.
pub(crate) fn lock_changes(base: &str, head: &str) -> Result<LockChanges, String> {
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
                    let name = extension_owner(entry).ok_or_else(|| lock_changed(entry))?;
                    changes.owned.push((entry.clone(), name.to_owned()));
                }
            }
        }
    }
    Ok(changes)
}

/// The registry files of the versions the lock selects, skipping files a registry lacks.
pub(crate) fn registry_files(lock: &str) -> Result<Vec<RegistryFile>, String> {
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

/// The text of a repository cache entry whose sha256 matches `hash`.
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

/// Whether a module file registers toolchains or execution platforms.
pub(crate) fn registers_toolchains(file: &ModuleFile) -> bool {
    file.calls().iter().any(|call| {
        matches!(
            call.name.as_str(),
            "register_toolchains" | "register_execution_platforms"
        )
    })
}

/// The dependency graph of the registry module files, with the built-in `bazel_tools` node.
pub(crate) fn module_graph(
    files: &[RegistryFile],
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<ModuleGraph, String> {
    let mut graph = ModuleGraph::default();
    graph.deps.insert(
        BUILTIN_TOOLS.to_owned(),
        BUILTIN_TOOLS_DEPS
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
    );
    graph.registers.insert(BUILTIN_TOOLS.to_owned());
    for file in files {
        let id = format!("{}@{}", file.name, file.version);
        let text = read(&file.hash).ok_or_else(|| format!("no cached MODULE.bazel for {id}"))?;
        let module = ModuleFile::parse(&text)
            .map_err(|error| format!("cannot read MODULE.bazel of {id}: {error}"))?;
        let deps = graph.deps.entry(file.name.clone()).or_default();
        for call in module.calls_named("bazel_dep") {
            let name = call
                .str("name")
                .ok_or_else(|| format!("cannot read MODULE.bazel of {id}: computed bazel_dep"))?;
            deps.insert(name.to_owned());
        }
        if registers_toolchains(&module) {
            graph.registers.insert(file.name.clone());
        }
    }
    Ok(graph)
}

/// The changed modules and every module that depends on one, directly or transitively.
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

fn label_repo(label: &str) -> Option<&str> {
    let rest = label.strip_prefix('@')?;
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    Some(rest.split_once("//").map_or(rest, |(repo, _)| repo))
}

/// The module a label's repository belongs to, `""` for the root module.
fn label_module(label: &str, deps: &BTreeMap<String, String>, own: &[String]) -> String {
    match label_repo(label) {
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

/// Each apparent repository the root module sees, mapped to the module that owns it.
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
            } else if let Some(module) = rules.get(call.name.as_str()) {
                if let Some(name) = call.str("name") {
                    owners.insert(name.to_owned(), module.clone());
                }
            }
        }
    }
    owners
}

/// Why the overrides in the module files keep a bump from narrowing, if they do.
pub(crate) fn overrides_that_widen(files: &[ModuleFile]) -> Option<String> {
    files.iter().flat_map(ModuleFile::calls).find_map(|call| {
        let module = call.str("module_name").unwrap_or_default();
        let opaque = matches!(
            call.name.as_str(),
            "git_override" | "archive_override" | "local_path_override"
        );
        let patched = call.name.ends_with("_override")
            && (call.arg("patches").is_some() || call.arg("patch_cmds").is_some());
        (opaque || patched).then(|| format!("MODULE.bazel has {} for {module}", call.name))
    })
}

fn repo_of_module(repo: &str, module: &str) -> bool {
    [format!("{module}+"), format!("{module}~")]
        .iter()
        .any(|prefix| repo.starts_with(prefix.as_str()))
}

fn root_owned(repo: &str) -> bool {
    repo.starts_with('+') || repo.starts_with("_main~")
}

/// The graph labels in repositories of the modules, and in root-owned repositories.
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
    fn bumped_deps_returns_names_whose_version_changed() {
        let base = "module(name = \"r\")\nbazel_dep(name = \"leaf\", version = \"1.0\")\nbazel_dep(name = \"other\", version = \"2.0\")\n";
        let head = "module(name = \"r\")\nbazel_dep(name = \"leaf\", version = \"1.10\")\nbazel_dep(name = \"other\", version = \"2.0\")\n";
        assert_eq!(bumped_deps(base, head), Ok(set(&["leaf"])));
        assert_eq!(bumped_deps(base, base), Ok(set(&[])));
    }

    #[test]
    fn bumped_deps_rejects_any_other_edit() {
        let base = "bazel_dep(name = \"leaf\", version = \"1.0\")\n";
        let error = Err(module_input_error());
        for head in [
            "bazel_dep(name = \"leaf\", version = \"1.1\", dev_dependency = True)\n",
            "bazel_dep(name = \"leaf\", version = \"1.1\")\nbazel_dep(name = \"x\", version = \"1\")\n",
            "bazel_dep(name = \"leaf\", version = V)\n",
            "bazel_dep(name = \"leaf\", version = \"1.0\")\nbazel_dep(name = \"leaf\", version = \"1.0\")\n",
            "bazel_dep(name = NAME, version = \"1.0\")\n",
            "bazel_dep(name = \"leaf\", version = \"1.1\")\n# note\n",
            "bazel_dep(",
        ] {
            assert_eq!(bumped_deps(base, head), error, "{head}");
        }
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
            &format!("\"{R}/bazel_registry.json\": \"a\", \"{R}/modules/leaf/1.1/source.json\": \"c\", \"{R}/modules/dep/2.0/MODULE.bazel\": \"d\""),
            ", \"moduleExtensions\": {\"@@leaf+//:e.bzl%e\": {\"x\": 2}}, \"selectedYankedVersions\": {\"old@1.0\": \"why\"}, \"facts\": {\"@@tool~//:f.bzl%f\": 1}",
        );
        let changes = lock_changes(&base, &head).expect("narrows");
        assert_eq!(changes.modules, set(&["dep", "leaf"]));
        assert_eq!(
            changes.owned,
            vec![
                ("@@tool~//:f.bzl%f".to_owned(), "tool".to_owned()),
                ("@@leaf+//:e.bzl%e".to_owned(), "leaf".to_owned()),
                ("old@1.0".to_owned(), "old".to_owned()),
            ]
        );
    }

    #[test]
    fn lock_changes_reject_root_and_unknown_keys() {
        let base = lock("", "");
        for (head, key) in [
            (
                lock("", ", \"moduleExtensions\": {\"//:e.bzl%e\": 1}"),
                "//:e.bzl%e",
            ),
            (lock("", ", \"factsVersions\": {}"), "factsVersions"),
            (
                lock(&format!("\"{R}/bazel_registry.json\": \"a\""), ""),
                "https://bcr.bazel.build/bazel_registry.json",
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
        let scratch = dx_test_scratch::scratch("dx-module-cache-");
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
            ("leaf", "module(name = \"leaf\")\n"),
            ("user", "bazel_dep(name = \"leaf\", version = \"1\")\n"),
            ("top", "bazel_dep(name = \"user\", version = \"1\")\n"),
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
        let graph = module_graph(&listed, &read).expect("graph");
        assert_eq!(graph.registers, set(&[BUILTIN_TOOLS, "other"]));
        assert_eq!(
            closure(&set(&["leaf"]), &graph.deps),
            set(&["leaf", "top", "user"])
        );
        assert!(closure(&set(&["rules_cc"]), &graph.deps).contains(BUILTIN_TOOLS));
        assert_eq!(
            module_graph(&[file("lost", "lost")], &read),
            Err("no cached MODULE.bazel for lost@1".to_owned())
        );
        let broken = |_: &str| Some("bazel_dep(".to_owned());
        let error = module_graph(&[file("bad", "bad")], &broken).expect_err("parse error");
        assert!(
            error.starts_with("cannot read MODULE.bazel of bad@1: "),
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
    fn opaque_and_patched_overrides_widen() {
        for (text, reason) in [
            (
                "git_override(module_name = \"a\", remote = \"r\")\n",
                "git_override for a",
            ),
            (
                "archive_override(module_name = \"a\", urls = [])\n",
                "archive_override for a",
            ),
            (
                "local_path_override(module_name = \"a\", path = \"x\")\n",
                "local_path_override for a",
            ),
            (
                "single_version_override(module_name = \"a\", patches = [\"//:p\"])\n",
                "single_version_override for a",
            ),
            (
                "single_version_override(module_name = \"a\", patch_cmds = [\"x\"])\n",
                "single_version_override for a",
            ),
        ] {
            assert_eq!(
                overrides_that_widen(&files(text)),
                Some(format!("MODULE.bazel has {reason}"))
            );
        }
        assert_eq!(
            overrides_that_widen(&files(
                "single_version_override(module_name = \"a\", version = \"1\")\n"
            )),
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
            "@@_main~ext~old//:f",
            "@@other+//:g",
            "//main:h",
        ];
        assert_eq!(
            module_seeds(labels.into_iter(), &set(&["foo"])),
            [
                "@@+ext+local//:e",
                "@@_main~ext~old//:f",
                "@@foo++ext+hub//:b",
                "@@foo+//:a",
                "@@foo~//:d",
            ]
        );
    }
}
