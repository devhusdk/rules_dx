//! Every dir, dependency, extension, toolchain, Gazelle language and aspect of the root module belongs to exactly one family in families.json.

use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{Call, ModuleFile, Value};

const KEYS: [&str; 8] = [
    "dirs",
    "bazel_deps",
    "extensions",
    "repo_rules",
    "toolchains",
    "gazelle",
    "aspects",
    "file_extensions",
];

/// The items of one kind, as family name to sorted entries.
type Kind = BTreeMap<String, BTreeSet<String>>;

fn is_dev(call: &Call) -> bool {
    matches!(call.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True")
}

fn literal(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Str(text)) => Some(&text.value),
        _ => None,
    }
}

fn bzl_and_name(call: &Call) -> Option<String> {
    let bzl = literal(
        call.positional
            .first()
            .or_else(|| call.arg("extension_bzl_file")),
    )?;
    let name = literal(
        call.positional
            .get(1)
            .or_else(|| call.arg("extension_name")),
    )?;
    Some(format!("{bzl}%{name}"))
}

fn module_items(module: &ModuleFile) -> BTreeMap<&'static str, BTreeSet<String>> {
    let mut bazel_deps = BTreeSet::new();
    let mut extensions = BTreeSet::new();
    let mut toolchains = BTreeSet::new();
    let mut rules = BTreeMap::new();
    for call in module.calls() {
        match call.name.as_str() {
            "bazel_dep" if !is_dev(call) => {
                bazel_deps.extend(call.str("name").map(str::to_owned));
            }
            "use_extension" if !is_dev(call) => {
                extensions.extend(bzl_and_name(call));
            }
            "register_toolchains" if !is_dev(call) => {
                for value in &call.positional {
                    toolchains.extend(literal(Some(value)).map(str::to_owned));
                }
            }
            "use_repo_rule" => {
                if let (Some(binding), Some(rule)) = (&call.binding, bzl_and_name(call)) {
                    rules.insert(binding.clone(), (rule, false));
                }
            }
            _ => {}
        }
    }
    for call in module.calls() {
        if let Some((_, used)) = rules.get_mut(&call.name) {
            *used |= !is_dev(call);
        }
    }
    let repo_rules = rules
        .into_values()
        .filter(|(_, used)| *used)
        .map(|(rule, _)| rule)
        .collect();
    BTreeMap::from([
        ("bazel_deps", bazel_deps),
        ("extensions", extensions),
        ("repo_rules", repo_rules),
        ("toolchains", toolchains),
    ])
}

/// The `//gazelle/<x>` labels of the `dx_gazelle.language` tags.
fn gazelle_languages(module: &ModuleFile) -> BTreeSet<String> {
    module
        .calls_named("dx_gazelle.language")
        .filter(|call| !is_dev(call))
        .filter_map(|call| call.str("library").map(str::to_owned))
        .collect()
}

fn aspect_names(aspects: &str) -> BTreeSet<String> {
    aspects
        .lines()
        .filter_map(|line| {
            line.split_once(" = ")?
                .0
                .strip_prefix("real_")
                .map(|_| line)
        })
        .filter_map(|line| line.split_once(" = ").map(|(name, _)| name))
        .filter(|name| {
            name.ends_with("_aspect")
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
        .map(str::to_owned)
        .collect()
}

fn root_dirs(root: &std::path::Path) -> BTreeSet<String> {
    std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("read_dir {root:?}: {error}"))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && !name.starts_with("bazel-"))
        .collect()
}

fn families(json: &str) -> BTreeMap<&'static str, Kind> {
    let value: serde_json::Value = serde_json::from_str(json).expect("families.json");
    let mut kinds: BTreeMap<&'static str, Kind> =
        KEYS.iter().map(|key| (*key, Kind::new())).collect();
    for (family, entry) in value["families"].as_object().expect("families object") {
        let entry = entry.as_object().expect("family object");
        for key in entry.keys() {
            assert!(
                key == "module" || key == "path" || KEYS.contains(&key.as_str()),
                "family {family:?} has unknown key {key:?}"
            );
        }
        for key in KEYS {
            let items = entry
                .get(key)
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("family {family:?} has no {key} array"));
            let items = items
                .iter()
                .map(|item| item.as_str().expect("string item").to_owned())
                .collect();
            kinds
                .get_mut(key)
                .expect("known key")
                .insert(family.clone(), items);
        }
    }
    kinds
}

/// Every way `kind` disagrees with the `actual` items; empty when each is in exactly one family.
fn violations(label: &str, kind: &Kind, actual: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    for item in actual {
        let owners: Vec<&str> = kind
            .iter()
            .filter(|(_, items)| items.contains(item))
            .map(|(family, _)| family.as_str())
            .collect();
        match owners.len() {
            1 => {}
            0 => problems.push(format!("{label} {item:?} is in no family")),
            _ => problems.push(format!(
                "{label} {item:?} is in several families: {owners:?}"
            )),
        }
    }
    for (family, items) in kind {
        for item in items.difference(actual) {
            problems.push(format!(
                "family {family:?} lists {label} {item:?} which does not exist"
            ));
        }
    }
    problems
}

fn extension_problems(kinds: &BTreeMap<&'static str, Kind>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for (family, items) in &kinds["file_extensions"] {
        for item in items {
            if !item.starts_with('.') {
                problems.push(format!(
                    "family {family:?} file extension {item:?} must start with a dot"
                ));
            }
            if !seen.insert(item) {
                problems.push(format!("file extension {item:?} is listed twice"));
            }
        }
    }
    problems
}

/// Problems with the optional `module` field: it names the published `rules_dx_<family>`
/// module, so it is unique and never set on `core`.
fn module_problems(json: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("families.json");
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    for (family, entry) in value["families"].as_object().expect("families object") {
        let Some(module) = entry.get("module") else {
            continue;
        };
        let Some(module) = module.as_str() else {
            problems.push(format!(
                "family {family:?} has a module that is not a string"
            ));
            continue;
        };
        if family == "core" {
            problems.push("core has a module; it ships in rules_dx itself".to_owned());
        }
        if !module.starts_with("rules_dx_") {
            problems.push(format!("module {module:?} does not start with rules_dx_"));
        }
        if !seen.insert(module) {
            problems.push(format!("module {module:?} is used twice"));
        }
    }
    problems
}

/// Family name to the checkout directory holding its own `MODULE.bazel`; families without a `path` are in the root module.
fn family_paths(json: &str) -> BTreeMap<String, String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("families.json");
    value["families"]
        .as_object()
        .expect("families object")
        .iter()
        .filter_map(|(family, entry)| {
            Some((family.clone(), entry.get("path")?.as_str()?.to_owned()))
        })
        .collect()
}

/// `kinds` without the entries of moved families, except aspects and file extensions, which are not module items.
fn root_kinds(
    kinds: &BTreeMap<&'static str, Kind>,
    paths: &BTreeMap<String, String>,
) -> BTreeMap<&'static str, Kind> {
    let mut kinds = kinds.clone();
    for (key, kind) in &mut kinds {
        if *key != "aspects" && *key != "file_extensions" {
            kind.retain(|family, _| !paths.contains_key(family));
        }
    }
    kinds
}

/// Problems of one moved family: its listed items against what its own module and dirs hold.
fn moved_problems(
    family: &str,
    kinds: &BTreeMap<&'static str, Kind>,
    actual: &BTreeMap<&'static str, BTreeSet<String>>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (key, items) in actual {
        let own: Kind = kinds[key]
            .iter()
            .filter(|(name, _)| name.as_str() == family)
            .map(|(name, items)| (name.clone(), items.clone()))
            .collect();
        problems.extend(violations(key, &own, items));
    }
    problems
}

#[test]
fn modules_are_unique_and_named_rules_dx() {
    let problems = module_problems(&dx_testing::read_runfiles("core/modules/families.json"));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn module_problems_are_reported() {
    let json = r#"{"families": {
        "core": {"module": "rules_dx_core"},
        "go": {"module": "rules_dx_x"},
        "rust": {"module": "rules_dx_x"},
        "lua": {"module": "lua"}
    }}"#;
    assert_eq!(module_problems(json).len(), 3);
}

#[test]
fn every_item_is_in_exactly_one_family() {
    let root = dx_testing::rules_dx_source();
    let module =
        ModuleFile::parse(&dx_testing::read_runfiles("MODULE.bazel")).expect("MODULE.bazel");
    let kinds = families(&dx_testing::read_runfiles("core/modules/families.json"));
    let mut actual = module_items(&module);
    actual.insert("dirs", root_dirs(&root));
    actual.insert("gazelle", gazelle_languages(&module));
    actual.insert(
        "aspects",
        aspect_names(&dx_testing::read_runfiles(
            "core/quality/testdata/all_languages.bzl",
        )),
    );
    let paths = family_paths(&dx_testing::read_runfiles("core/modules/families.json"));
    let mut problems = extension_problems(&kinds);
    for (key, items) in &actual {
        problems.extend(violations(key, &root_kinds(&kinds, &paths)[key], items));
    }
    for (family, path) in &paths {
        let dir = root.join(path);
        let text = std::fs::read_to_string(dir.join("MODULE.bazel"))
            .unwrap_or_else(|error| panic!("{path}/MODULE.bazel: {error}"));
        let module = ModuleFile::parse(&text).expect("family MODULE.bazel");
        let mut own = module_items(&module);
        own.insert("dirs", root_dirs(&dir));
        own.insert("gazelle", gazelle_languages(&module));
        problems.extend(moved_problems(family, &kinds, &own));
    }
    assert!(
        problems.is_empty(),
        "families.json is out of date (checkout {root:?}):\n{}",
        problems.join("\n")
    );
}

/// Names of the families other than `core` that add nothing to a consumer's module graph.
fn weightless_families(kinds: &BTreeMap<&'static str, Kind>) -> Vec<String> {
    kinds["dirs"]
        .keys()
        .filter(|family| family.as_str() != "core")
        .filter(|family| {
            ["bazel_deps", "extensions", "toolchains", "repo_rules"]
                .iter()
                .all(|key| kinds[key][*family].is_empty())
        })
        .cloned()
        .collect()
}

#[test]
fn every_family_but_core_adds_to_the_module_graph() {
    let kinds = families(&dx_testing::read_runfiles("core/modules/families.json"));
    assert_eq!(
        weightless_families(&kinds),
        Vec::<String>::new(),
        "fold these into another family"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(entries: &[(&str, &[&str])]) -> Kind {
        entries
            .iter()
            .map(|(family, items)| {
                (
                    (*family).to_owned(),
                    items.iter().map(|i| (*i).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|i| (*i).to_owned()).collect()
    }

    #[test]
    fn agreeing_items_pass() {
        let kind = kind(&[("core", &["a"]), ("go", &["b"])]);
        assert!(violations("dir", &kind, &set(&["a", "b"])).is_empty());
    }

    #[test]
    fn missing_item_fails() {
        let kind = kind(&[("core", &["a"])]);
        assert_eq!(violations("dir", &kind, &set(&["a", "b"])).len(), 1);
    }

    #[test]
    fn duplicated_item_fails() {
        let kind = kind(&[("core", &["a"]), ("go", &["a"])]);
        assert_eq!(violations("dir", &kind, &set(&["a"])).len(), 1);
    }

    #[test]
    fn stale_entry_fails() {
        let kind = kind(&[("core", &["a", "gone"])]);
        assert_eq!(violations("dir", &kind, &set(&["a"])).len(), 1);
    }

    #[test]
    fn repeated_or_dotless_file_extension_fails() {
        let mut kinds = BTreeMap::new();
        kinds.insert(
            "file_extensions",
            kind(&[("core", &[".a", "b"]), ("go", &[".a"])]),
        );
        assert_eq!(extension_problems(&kinds).len(), 2);
    }

    #[test]
    fn family_without_deps_is_weightless() {
        let mut kinds = BTreeMap::new();
        for key in ["bazel_deps", "extensions", "toolchains", "repo_rules"] {
            kinds.insert(key, kind(&[("core", &[]), ("go", &[]), ("rust", &[])]));
        }
        kinds.insert("dirs", kind(&[("core", &[]), ("go", &[]), ("rust", &[])]));
        kinds
            .get_mut("toolchains")
            .unwrap()
            .insert("rust".into(), set(&["t"]));
        assert_eq!(weightless_families(&kinds), vec!["go".to_owned()]);
    }

    #[test]
    fn moved_family_agrees_with_its_own_module() {
        let kinds = BTreeMap::from([("bazel_deps", kind(&[("android", &["rules_android"])]))]);
        let actual = BTreeMap::from([("bazel_deps", set(&["rules_android"]))]);
        assert!(moved_problems("android", &kinds, &actual).is_empty());
    }

    #[test]
    fn moved_family_item_left_in_root_fails() {
        let kinds = BTreeMap::from([("bazel_deps", kind(&[("android", &["rules_android"])]))]);
        let paths = BTreeMap::from([("android".to_owned(), "families/android".to_owned())]);
        let root = root_kinds(&kinds, &paths);
        let actual = set(&["rules_android"]);
        assert_eq!(
            violations("bazel_deps", &root["bazel_deps"], &actual).len(),
            1
        );
    }

    #[test]
    fn gazelle_languages_reads_labels() {
        let source = "dx_gazelle.language(library = \"//gazelle/go\")\n";
        let module = ModuleFile::parse(source).expect("module");
        assert_eq!(gazelle_languages(&module), set(&["//gazelle/go"]));
    }
}

/// Non-exempt packages of the family at `dir` with their BUILD.bazel text, as paths below `dir`.
fn checked_packages(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let mut subdirs = Vec::new();
        for entry in std::fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("read_dir {current:?}: {error}"))
            .filter_map(Result::ok)
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && !name.starts_with('.') && !name.starts_with("bazel-") {
                subdirs.push(entry.path());
            }
        }
        let build = current.join("BUILD.bazel");
        if let Ok(text) = std::fs::read_to_string(&build) {
            let exempt = text.contains("# gazelle:dx_rust_corpus false")
                || text.starts_with("# Generated by dx")
                || current
                    .components()
                    .any(|part| part.as_os_str() == "fixtures")
                || (current != dir && current.join("MODULE.bazel").exists());
            let relative = current.strip_prefix(dir).unwrap_or(&current);
            if !exempt {
                found.push((relative.display().to_string(), text));
            }
        }
        pending.extend(subdirs);
    }
    found.sort();
    found
}

/// Packages of the family at `dir` whose BUILD.bazel has no `:corpus_starlark`, as paths below `dir`.
fn without_corpus(dir: &std::path::Path) -> Vec<String> {
    checked_packages(dir)
        .into_iter()
        .filter(|(_, text)| !text.contains("corpus_starlark"))
        .map(|(package, _)| package)
        .collect()
}

#[test]
fn moved_families_have_corpus_starlark_in_every_package() {
    let root = dx_testing::rules_dx_source();
    let paths = family_paths(&dx_testing::read_runfiles("core/modules/families.json"));
    for (family, path) in &paths {
        let dir = root.join(path);
        let missing = without_corpus(&dir);
        assert!(
            missing.is_empty(),
            "families/{family} packages without :corpus_starlark (or `# gazelle:dx_rust_corpus false`): {missing:?}"
        );
    }
}

const FAMILY_FILES: [&str; 44] = [
    "ansible",
    "beam",
    "cc",
    "bicep",
    "cloudformation",
    "cmake",
    "core",
    "cue",
    "d",
    "dart",
    "dockerfile",
    "dotnet",
    "fortran",
    "generation",
    "go",
    "haskell",
    "html_template",
    "jsonnet",
    "kubernetes",
    "latex",
    "lua",
    "make",
    "nix",
    "oci_image",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "python",
    "julia",
    "jvm",
    "ruby",
    "pkl",
    "rego",
    "rust",
    "shell",
    "sql",
    "swift",
    "terraform",
    "text",
    "typst",
    "xml",
    "yaml",
    "zig",
];

/// Family files that sit in their language directory instead of `core/quality/families`.
const LANGUAGE_FAMILY_FILES: [&str; 8] = [
    "ansible",
    "bicep",
    "cloudformation",
    "dockerfile",
    "kubernetes",
    "rego",
    "rust",
    "terraform",
];

fn family_file(family: &str) -> String {
    if LANGUAGE_FAMILY_FILES.contains(&family) {
        format!("{family}/private/quality/{family}.bzl")
    } else {
        format!("core/quality/families/{family}.bzl")
    }
}

fn family_load(family: &str) -> String {
    if LANGUAGE_FAMILY_FILES.contains(&family) {
        format!("load(\"//{family}/private/quality:{family}.bzl\"")
    } else {
        format!("load(\"//core/quality:families/{family}.bzl\"")
    }
}

#[test]
fn family_files_define_aspects_listed_in_families_json() {
    let json: serde_json::Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("json");
    let listed: BTreeSet<&str> = json["families"]
        .as_object()
        .expect("families")
        .values()
        .flat_map(|entry| entry["aspects"].as_array().expect("aspects"))
        .map(|name| name.as_str().expect("aspect name"))
        .collect();
    for family in FAMILY_FILES {
        let source = dx_testing::read_runfiles(&family_file(family));
        for other in FAMILY_FILES {
            assert!(
                !source.contains(&family_load(other)),
                "{family}.bzl loads another family file"
            );
        }
        let defined = source
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .filter(|(name, value)| {
                name.ends_with("_aspect") && !name.starts_with('_') && !value.starts_with('_')
            })
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        assert!(!defined.is_empty(), "{family}.bzl defines no aspect");
        for name in defined {
            assert!(listed.contains(name), "{name} is not in families.json");
        }
    }
}

#[test]
fn core_family_file_loads_no_family_rule_set() {
    let source = dx_testing::read_runfiles("core/quality/families/core.bzl");
    let foreign = source
        .lines()
        .filter(|line| line.starts_with("load(\"@") && !line.starts_with("load(\"@bazel_skylib"))
        .collect::<Vec<_>>();
    assert!(foreign.is_empty(), "core.bzl loads {foreign:?}");
}

/// The repos one family file may load from (files of rule-less languages count as core): the family's `bazel_deps`, the repos its extensions `use_repo` (a moved family also gets core's, since its aspect file stays in core), and `bazel_skylib`.
fn family_repos(module: &ModuleFile, json: &str, family: &str) -> BTreeSet<String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("families.json");
    let families = &value["families"];
    let entry = if families.get(family).is_some() {
        &families[family]
    } else {
        &families["core"]
    };
    let listed = |key: &str| -> BTreeSet<String> {
        entry[key]
            .as_array()
            .unwrap_or_else(|| panic!("family {family:?} has no {key} array"))
            .iter()
            .map(|item| item.as_str().expect("string item").to_owned())
            .collect()
    };
    let mut repos = listed("bazel_deps");
    let mut extensions = listed("extensions");
    if entry.get("path").is_some() {
        let core = |key: &str| {
            families["core"][key]
                .as_array()
                .unwrap_or_else(|| panic!("core has no {key} array"))
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        repos.extend(core("bazel_deps"));
        extensions.extend(core("extensions"));
    }
    let bindings: BTreeSet<&str> = module
        .calls()
        .iter()
        .filter(|call| call.name == "use_extension")
        .filter(|call| bzl_and_name(call).is_some_and(|ext| extensions.contains(&ext)))
        .filter_map(|call| call.binding.as_deref())
        .collect();
    for call in module.calls().iter().filter(|call| call.name == "use_repo") {
        let Some(Value::Ident(binding)) = call.positional.first() else {
            continue;
        };
        if !bindings.contains(binding.as_str()) {
            continue;
        }
        repos.extend(
            call.positional[1..]
                .iter()
                .filter_map(|v| literal(Some(v)))
                .map(str::to_owned),
        );
        repos.extend(call.named.iter().map(|(alias, _)| alias.clone()));
    }
    repos.insert("bazel_skylib".to_owned());
    repos
}

#[test]
fn family_repos_follow_deps_and_extension_use_repo() {
    let module = ModuleFile::parse(
        r#"
bazel_dep(name = "rules_x", version = "1")
ext = use_extension("//x:e.bzl", "x")
use_repo(ext, "hub", alias = "real_hub")
other = use_extension("//y:e.bzl", "y")
use_repo(other, "foreign")
"#,
    )
    .expect("module");
    let json = r#"{"families": {"x": {
        "bazel_deps": ["rules_x"],
        "extensions": ["//x:e.bzl%x"]
    }}}"#;
    let repos: Vec<String> = family_repos(&module, json, "x").into_iter().collect();
    assert_eq!(repos, ["alias", "bazel_skylib", "hub", "rules_x"]);
}

#[test]
fn family_files_load_only_their_own_repos() {
    let json = dx_testing::read_runfiles("core/modules/families.json");
    let module =
        ModuleFile::parse(&dx_testing::read_runfiles("MODULE.bazel")).expect("MODULE.bazel");
    let mut problems = Vec::new();
    for family in FAMILY_FILES {
        let allowed = family_repos(&module, &json, family);
        let source = dx_testing::read_runfiles(&family_file(family));
        for line in source.lines().filter(|line| line.starts_with("load(\"@")) {
            let repo = line["load(\"@".len()..]
                .split("//")
                .next()
                .unwrap_or_default();
            if !allowed.contains(repo) {
                problems.push(format!("{family}.bzl loads @{repo}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The source-check targets every moved family declares in its own `BUILD.bazel`.
const FAMILY_CHECK_TARGETS: [&str; 3] =
    ["corpus_starlark", "corpus_markdown", "starlark_docs_test"];

fn missing_check_targets(family: &str, build_file: Option<&str>) -> Vec<String> {
    let Some(text) = build_file else {
        return vec![format!("{family}: no BUILD.bazel at the family root")];
    };
    FAMILY_CHECK_TARGETS
        .iter()
        .filter(|name| !text.contains(&format!("name = \"{name}\"")))
        .map(|name| format!("{family}: BUILD.bazel has no {name} target"))
        .collect()
}

#[test]
fn moved_families_declare_the_source_check_targets() {
    let root = dx_testing::rules_dx_source();
    let paths = family_paths(&dx_testing::read_runfiles("core/modules/families.json"));
    let mut problems = Vec::new();
    for (family, path) in &paths {
        let dir = root.join(path);
        let text = std::fs::read_to_string(dir.join("BUILD.bazel")).ok();
        let has_packages = checked_packages(&dir)
            .iter()
            .any(|(package, _)| !package.is_empty());
        problems.extend(
            missing_check_targets(family, text.as_deref())
                .into_iter()
                .filter(|problem| has_packages || !problem.contains("starlark_docs_test")),
        );
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_family_without_a_build_file_or_target_is_reported() {
    assert_eq!(
        missing_check_targets("x", None),
        ["x: no BUILD.bazel at the family root"]
    );
    assert_eq!(
        missing_check_targets("x", Some("")),
        [
            "x: BUILD.bazel has no corpus_starlark target",
            "x: BUILD.bazel has no corpus_markdown target",
            "x: BUILD.bazel has no starlark_docs_test target"
        ]
    );
    assert_eq!(
        missing_check_targets("x", Some("name = \"corpus_starlark\"")),
        [
            "x: BUILD.bazel has no corpus_markdown target",
            "x: BUILD.bazel has no starlark_docs_test target"
        ]
    );
    let both = FAMILY_CHECK_TARGETS
        .iter()
        .map(|name| format!("name = \"{name}\""))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(missing_check_targets("x", Some(&both)).is_empty());
}

/// The comma-separated packages the family root `build_file` lists under the env key `name`.
fn listed_packages(build_file: &str, name: &str) -> Vec<String> {
    let key = format!("\"{name}\":");
    let Some((_, rest)) = build_file.split_once(&key) else {
        return Vec::new();
    };
    let value = rest.trim_start().strip_prefix('"').unwrap_or("");
    let value = value.split('"').next().unwrap_or("");
    value
        .split(',')
        .filter(|package| !package.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Problems with the package lists in a family root `build_file`, given its non-exempt `packages`.
fn package_list_problems(family: &str, build_file: &str, packages: &[String]) -> Vec<String> {
    let docs = listed_packages(build_file, "STARLARK_DOCS_PACKAGES");
    let mut problems = Vec::new();
    for package in packages.iter().filter(|package| !package.is_empty()) {
        if !docs.contains(package) {
            problems.push(format!(
                "{family}: package {package} is not in STARLARK_DOCS_PACKAGES"
            ));
        }
    }
    for package in &docs {
        if !build_file.contains(&format!("\"//{package}:corpus_starlark\"")) {
            problems.push(format!(
                "{family}: //{package}:corpus_starlark is listed but missing from data"
            ));
        }
    }
    problems
}

#[test]
fn moved_families_list_every_package_in_the_source_checks() {
    let root = dx_testing::rules_dx_source();
    let paths = family_paths(&dx_testing::read_runfiles("core/modules/families.json"));
    let mut problems = Vec::new();
    for (family, path) in &paths {
        let dir = root.join(path);
        let Ok(build_file) = std::fs::read_to_string(dir.join("BUILD.bazel")) else {
            continue;
        };
        let packages: Vec<String> = checked_packages(&dir)
            .into_iter()
            .map(|(package, _)| package)
            .collect();
        problems.extend(package_list_problems(family, &build_file, &packages));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn an_unlisted_package_or_missing_data_label_is_reported() {
    let build = concat!(
        "data = [\"//rules:corpus_starlark\"],\n",
        "env = {\"STARLARK_DOCS_PACKAGES\": \"rules,sdk\"},\n",
    );
    let packages = ["", "rules", "sdk", "extra"].map(str::to_owned);
    assert_eq!(
        package_list_problems("x", build, &packages),
        [
            "x: package extra is not in STARLARK_DOCS_PACKAGES",
            "x: //sdk:corpus_starlark is listed but missing from data",
        ]
    );
}
