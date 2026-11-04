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

/// The `//gazelle/<x>` labels of the dispatch binary's languages.
fn gazelle_languages(build: &str) -> BTreeSet<String> {
    let start = build
        .find("languages = [")
        .expect("gazelle_binary languages");
    let list = &build[start..];
    let list = &list[..list.find(']').expect("end of languages")];
    list.split('"')
        .filter(|text| text.starts_with("//gazelle/"))
        .map(str::to_owned)
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
                KEYS.contains(&key.as_str()),
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

#[test]
fn every_item_is_in_exactly_one_family() {
    let root = dx_testing::rules_dx_source();
    let module =
        ModuleFile::parse(&dx_testing::read_runfiles("MODULE.bazel")).expect("MODULE.bazel");
    let kinds = families(&dx_testing::read_runfiles("modules/families.json"));
    let mut actual = module_items(&module);
    actual.insert("dirs", root_dirs(&root));
    actual.insert(
        "gazelle",
        gazelle_languages(&dx_testing::read_runfiles("gazelle/dispatch/BUILD.bazel")),
    );
    actual.insert(
        "aspects",
        aspect_names(&dx_testing::read_runfiles("quality/real_aspects.bzl")),
    );
    let mut problems = extension_problems(&kinds);
    for (key, items) in &actual {
        problems.extend(violations(key, &kinds[key], items));
    }
    assert!(
        problems.is_empty(),
        "families.json is out of date (checkout {root:?}):\n{}",
        problems.join("\n")
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
    fn gazelle_languages_reads_labels() {
        let build = "gazelle_binary(\n languages = [\n \"//gazelle/go\",\n \":dispatch\",\n ],\n)";
        assert_eq!(gazelle_languages(build), set(&["//gazelle/go"]));
    }
}
