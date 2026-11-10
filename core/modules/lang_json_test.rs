//! Every language dir named in families.json has a lang.json that satisfies lang.schema.json.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const NON_LANGUAGE: [&str; 31] = [
    "appimage",
    "bcr_test_module",
    "bundle",
    "ci",
    "cli",
    "components",
    "config",
    "core",
    "containers",
    "deploy",
    "docs",
    "dx",
    "env",
    "examples",
    "families",
    "gazelle",
    "ios",
    "macos",
    "packaging",
    "patches",
    "platforms",
    "provisioning",
    "rules",
    "sdk",
    "signing",
    "testing",
    "tests",
    "third_party",
    "toolchain",
    "toolchains",
    "tools",
];

fn language_dirs(root: &Path, families: &Value) -> BTreeSet<PathBuf> {
    let mut dirs = BTreeSet::new();
    for family in families["families"].as_object().expect("families").values() {
        let base = family["path"].as_str();
        for dir in family["dirs"].as_array().expect("dirs") {
            let dir = dir.as_str().expect("dir");
            if NON_LANGUAGE.contains(&dir) {
                continue;
            }
            if let Some(base) = base {
                let nested = PathBuf::from(base).join(dir);
                if root.join(&nested).is_dir() {
                    dirs.insert(nested);
                }
            }
            if root.join(dir).is_dir() {
                dirs.insert(PathBuf::from(dir));
            }
        }
    }
    dirs
}

fn type_problem(path: &str, value: &Value, schema: &Value) -> Option<String> {
    let wanted = schema["type"].as_str()?;
    let ok = match wanted {
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "boolean" => value.is_boolean(),
        _ => true,
    };
    (!ok).then(|| format!("{path}: expected {wanted}"))
}

fn value_problems(path: &str, value: &Value, schema: &Value, defs: &Value) -> Vec<String> {
    let schema = match schema["$ref"].as_str() {
        Some(reference) => &defs[reference.trim_start_matches("#/$defs/")],
        None => schema,
    };
    if let Some(problem) = type_problem(path, value, schema) {
        return vec![problem];
    }
    let mut problems = Vec::new();
    if let Some(allowed) = schema["enum"].as_array()
        && !allowed.contains(value)
    {
        problems.push(format!("{path}: {value} is not allowed"));
    }
    match value {
        Value::String(text) => {
            let min = schema["minLength"].as_u64().unwrap_or(0) as usize;
            if text.len() < min {
                problems.push(format!("{path}: empty string"));
            }
        }
        Value::Array(items) => {
            let seen: BTreeSet<String> = items.iter().map(Value::to_string).collect();
            if schema["uniqueItems"] == true && seen.len() != items.len() {
                problems.push(format!("{path}: duplicate items"));
            }
            for item in items {
                problems.extend(value_problems(path, item, &schema["items"], defs));
            }
        }
        Value::Object(object) => {
            let Some(properties) = schema["properties"].as_object() else {
                return problems;
            };
            let child = |key: &str| match path {
                "" => key.to_owned(),
                _ => format!("{path}.{key}"),
            };
            for key in schema["required"].as_array().into_iter().flatten() {
                let key = key.as_str().expect("required key");
                if !object.contains_key(key) {
                    problems.push(format!("{}: missing", child(key)));
                }
            }
            for (key, value) in object {
                match properties.get(key) {
                    Some(property) => {
                        problems.extend(value_problems(&child(key), value, property, defs));
                    }
                    None => problems.push(format!("{}: not in the schema", child(key))),
                }
            }
        }
        _ => {}
    }
    problems
}

fn schema_problems(manifest: &Value, schema: &Value) -> Vec<String> {
    if !manifest.is_object() {
        return vec!["not an object".to_owned()];
    }
    value_problems("", manifest, schema, &schema["$defs"])
}

fn problems(root: &Path, dir: &Path, schema: &Value) -> Vec<String> {
    let file = root.join(dir).join("lang.json");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return vec![format!("{}: lang.json missing", dir.display())];
    };
    let manifest: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => return vec![format!("{}: {error}", file.display())],
    };
    let mut found: Vec<String> = schema_problems(&manifest, schema)
        .into_iter()
        .map(|problem| format!("{}: {problem}", dir.display()))
        .collect();
    let name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
    if manifest["name"] != name {
        found.push(format!("{}: name must be {name}", dir.display()));
    }
    found
}

#[test]
fn every_language_dir_has_a_valid_lang_json() {
    let root = dx_testing::rules_dx_source();
    let families: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("families.json");
    let schema: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/lang.schema.json"))
            .expect("lang.schema.json");
    let dirs = language_dirs(&root, &families);
    assert!(dirs.len() > 40, "found only {} language dirs", dirs.len());
    let found: Vec<String> = dirs
        .iter()
        .flat_map(|dir| problems(&root, dir, &schema))
        .collect();
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn schema_rejects_a_bad_manifest() {
    let schema: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/lang.schema.json"))
            .expect("schema");
    let bad =
        serde_json::json!({"name": "x", "extra": 1, "platforms": ["beos"], "tools": ["a", "a"]});
    let found = schema_problems(&bad, &schema).join("\n");
    assert!(found.contains("title: missing"), "{found}");
    assert!(found.contains("extra: not in the schema"), "{found}");
    assert!(found.contains("is not allowed"), "{found}");
    assert!(found.contains("duplicate items"), "{found}");
}

#[test]
fn schema_rejects_a_bad_setup() {
    let schema: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/lang.schema.json"))
            .expect("schema");
    let bad = serde_json::json!({"setup": {
        "extra": [],
        "flags": [""],
        "licenses": [{"id": "x"}],
        "hubs": [{"ecosystem": "", "manifest": "m", "hub": 1, "version": "1"}],
    }});
    let found = schema_problems(&bad, &schema);
    for want in [
        "setup.extra: not in the schema",
        "setup.flags: empty string",
        "setup.licenses.platforms: missing",
        "setup.hubs.extension: missing",
        "setup.hubs.ecosystem: empty string",
        "setup.hubs.hub: expected string",
        "setup.hubs.version: not in the schema",
    ] {
        assert!(found.iter().any(|problem| problem == want), "{want}: {found:?}");
    }
}

#[test]
fn schema_rejects_a_hub_missing_any_key() {
    let schema: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/lang.schema.json"))
            .expect("schema");
    let full = serde_json::json!({
        "ecosystem": "cargo",
        "manifest": "Cargo.toml",
        "lock": "Cargo.lock",
        "state": null,
        "extension": "@rules_dx//rust:extensions.bzl",
        "tag": "from_cargo",
        "hub": "crates",
        "resolver": null,
        "imports": "name",
    });
    let whole = serde_json::json!({"setup": {"hubs": [full.clone()]}});
    let found = schema_problems(&whole, &schema);
    assert!(found.iter().all(|problem| !problem.starts_with("setup")), "{found:?}");
    for key in full.as_object().expect("entry").keys() {
        let mut entry = full.clone();
        entry.as_object_mut().expect("entry").remove(key);
        let bad = serde_json::json!({"setup": {"hubs": [entry]}});
        let found = schema_problems(&bad, &schema);
        let want = format!("setup.hubs.{key}: missing");
        assert!(found.contains(&want), "{want}: {found:?}");
    }
}

#[test]
fn every_hub_names_an_extension_its_own_language_exports() {
    let root = dx_testing::rules_dx_source();
    let families: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("families.json");
    let mut hubs = 0;
    let mut found = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        for hub in manifest["setup"]["hubs"].as_array().into_iter().flatten() {
            hubs += 1;
            let ecosystem = hub["ecosystem"].as_str().expect("ecosystem");
            let want = format!("@rules_dx//{}:extensions.bzl", dir.display());
            if hub["extension"] != want.as_str() {
                found.push(format!("{}: {ecosystem} extension must be {want}", dir.display()));
            }
            let exports = std::fs::read_to_string(root.join(&dir).join("extensions.bzl"))
                .unwrap_or_default();
            if !exports.lines().any(|line| line.starts_with(&format!("{ecosystem} = "))) {
                found.push(format!("{}/extensions.bzl exports no {ecosystem}", dir.display()));
            }
            if hub["lock"].is_null() != hub["tag"].is_null() {
                found.push(format!("{}: {ecosystem} needs lock and tag together", dir.display()));
            }
        }
    }
    assert!(hubs >= 5, "found only {hubs} hubs");
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn ecosystems_are_dx_ids_and_every_id_is_used() {
    let root = dx_testing::rules_dx_source();
    let families: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("families.json");
    let table: Value =
        serde_json::from_str(&dx_testing::read_runfiles("cli/ecosystems/ecosystems.json"))
            .expect("ecosystems.json");
    let ids: BTreeSet<String> = table["ecosystems"]
        .as_array()
        .expect("ecosystems")
        .iter()
        .map(|row| row["name"].as_str().expect("name").to_owned())
        .collect();
    let mut used = BTreeSet::new();
    let mut unknown = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        for id in manifest["ecosystems"].as_array().expect("ecosystems") {
            let id = id.as_str().expect("id");
            if !ids.contains(id) {
                unknown.push(format!("{}: {id} is not in ecosystems.json", dir.display()));
            }
            used.insert(id.to_owned());
        }
    }
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
    let unused: Vec<&String> = ids
        .iter()
        .filter(|id| id.as_str() != "bazel" && !used.contains(*id))
        .collect();
    assert!(unused.is_empty(), "no lang.json lists {unused:?}");
}

#[test]
fn languages_json_lists_every_depcheck_language() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let mut rows = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        if manifest["depcheck"].is_null() {
            continue;
        }
        rows.push((
            manifest["name"].as_str().expect("name").to_owned(),
            serde_json::json!({
                "name": manifest["name"],
                "depcheck": manifest["depcheck"],
                "semver": manifest["semver"],
                "ecosystems": manifest["ecosystems"],
            }),
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let rows: Vec<Value> = rows.into_iter().map(|(_, row)| row).collect();
    let text = serde_json::to_string_pretty(&serde_json::json!({"languages": rows}))
        .expect("serializes")
        + "\n";
    let scratch = dx_testing::mkscratch("languages_json").expect("scratch");
    let actual = scratch.join("languages.json");
    dx_testing::write_text(&actual, &text);
    let expected = dx_testing::resolve_runfiles("cli/ecosystems/languages.json");
    dx_testing::snapshot_diff(&expected, &actual, Some("cli/ecosystems/languages.json"))
        .expect("cli/ecosystems/languages.json is generated from every lang.json");
}

#[test]
fn families_languages_json_lists_every_quality_docs_and_setup_entry() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let mut rows = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        let mut row = serde_json::Map::new();
        row.insert("name".to_owned(), manifest["name"].clone());
        for key in ["quality", "docs", "setup"] {
            if !manifest[key].is_null() {
                row.insert(key.to_owned(), manifest[key].clone());
            }
        }
        if row.len() > 1 {
            rows.push((manifest["name"].as_str().expect("name").to_owned(), Value::Object(row)));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let rows: Vec<Value> = rows.into_iter().map(|(_, row)| row).collect();
    let text = serde_json::to_string_pretty(&serde_json::json!({"languages": rows}))
        .expect("serializes")
        + "\n";
    let scratch = dx_testing::mkscratch("families_languages_json").expect("scratch");
    let actual = scratch.join("languages.json");
    dx_testing::write_text(&actual, &text);
    let expected = dx_testing::resolve_runfiles("cli/families/languages.json");
    dx_testing::snapshot_diff(&expected, &actual, Some("cli/families/languages.json"))
        .expect("cli/families/languages.json is generated from every lang.json");
}

fn bzl_label_path(label: &str) -> String {
    let (package, file) = label.split_once(':').expect("quality.bzl is package:file");
    format!("{package}/{file}")
}

#[test]
fn every_quality_bzl_exists_and_feeds_families_test() {
    let root = dx_testing::rules_dx_source();
    let families: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("families.json");
    let build = std::fs::read_to_string(root.join("core/modules/BUILD.bazel")).expect("BUILD.bazel");
    let start = build.find("name = \"families_test\"").expect("families_test");
    let end = build.find("name = \"lang_json_test\"").expect("lang_json_test");
    let data = &build[start..end];
    let mut problems = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        let Some(label) = manifest["quality"]["bzl"].as_str() else {
            continue;
        };
        let path = bzl_label_path(label);
        if !root.join(&path).is_file() {
            problems.push(format!("{}: {path} does not exist", dir.display()));
        }
        let (package, file) = label.split_once(':').expect("label");
        let data_label = format!("\"//{package}:{file}\"");
        if !data.contains(&data_label) {
            problems.push(format!(
                "{}: {data_label} is not in families_test data in core/modules/BUILD.bazel",
                dir.display()
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn langs_bzl_is_generated_from_every_lang_json() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let mut langs = serde_json::Map::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let mut manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        manifest.as_object_mut().expect("object").remove("$schema");
        let name = manifest["name"].as_str().expect("name").to_owned();
        langs.insert(name, manifest);
    }
    let body = serde_json::to_string_pretty(&Value::Object(langs)).expect("serializes");
    assert!(!body.contains('\\') && !body.contains("\"\"\""), "langs.bzl cannot embed escapes");
    let text = format!(
        "\"\"\"Generated from every <lang>/lang.json by //core/modules:lang_json_test; refresh with UPDATE_EXPECT=1.\"\"\"\n\nLANGS = json.decode(\"\"\"{body}\"\"\")\n"
    );
    let scratch = dx_testing::mkscratch("langs_bzl").expect("scratch");
    let actual = scratch.join("langs.bzl");
    dx_testing::write_text(&actual, &text);
    let expected = dx_testing::resolve_runfiles("core/modules/langs.bzl");
    dx_testing::snapshot_diff(&expected, &actual, Some("core/modules/langs.bzl"))
        .expect("core/modules/langs.bzl is generated from every lang.json");
}

fn lang_aspects(manifest: &Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (section, label_key) in [("aspects", "quality"), ("probes", "quality"), ("aspects", "docs")] {
        let Some(table) = manifest.get(label_key) else {
            continue;
        };
        let label = table["bzl"].as_str().expect("bzl").to_owned();
        let names: Vec<&Value> = match &table[section] {
            Value::Array(items) => items.iter().collect(),
            Value::Object(groups) => groups
                .values()
                .flat_map(|group| group.as_array().expect("group").iter())
                .collect(),
            _ => Vec::new(),
        };
        for name in names {
            found.push((label.clone(), name.as_str().expect("aspect").to_owned()));
        }
    }
    found
}

fn loaded_names(text: &str) -> BTreeSet<(String, String)> {
    let mut loaded = BTreeSet::new();
    for line in text.lines().filter(|line| line.starts_with("load(")) {
        let mut quoted = line.split('"').skip(1).step_by(2);
        let label = quoted.next().expect("load label");
        let (package, file) = label.trim_start_matches("//").split_once(':').expect("label");
        for name in quoted {
            loaded.insert((format!("{package}:{file}"), name.to_owned()));
        }
    }
    loaded
}

#[test]
fn every_lang_json_aspect_is_loaded_from_its_bzl() {
    let root = dx_testing::rules_dx_source();
    let families: Value =
        serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
            .expect("families.json");
    let listed: BTreeSet<&str> = families["families"]
        .as_object()
        .expect("families")
        .values()
        .flat_map(|family| family["aspects"].as_array().expect("aspects").iter())
        .map(|name| name.as_str().expect("aspect"))
        .collect();
    let loaded = loaded_names(&dx_testing::read_runfiles(
        "core/quality/testdata/all_languages.bzl",
    ));
    let mut problems = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        for (label, name) in lang_aspects(&manifest) {
            let (package, file) = label.split_once(':').expect("label");
            if !loaded.contains(&(format!("{package}:{file}"), name.clone())) {
                problems.push(format!(
                    "{}: {name} is not loaded from //{package}:{file} in core/quality/testdata/all_languages.bzl",
                    dir.display()
                ));
            }
            if !listed.contains(name.as_str()) {
                problems.push(format!(
                    "{}: {name} is not in any families.json aspects list",
                    dir.display()
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

const LOADS_BEGIN: &str = "# BEGIN GENERATED LANG ASPECT LOADS\n";
const LOADS_END: &str = "# END GENERATED LANG ASPECT LOADS\n";
const EXPORTS_BEGIN: &str = "# BEGIN GENERATED LANG ASPECT EXPORTS\n";
const EXPORTS_END: &str = "# END GENERATED LANG ASPECT EXPORTS\n";

fn every_lang_aspect(root: &Path, families: &Value) -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for dir in language_dirs(root, families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        found.extend(lang_aspects(&manifest));
    }
    found
}

fn splice(text: &str, begin: &str, end: &str, body: &str) -> String {
    let start = text.find(begin).unwrap_or_else(|| panic!("missing {begin:?}")) + begin.len();
    let stop = text.find(end).unwrap_or_else(|| panic!("missing {end:?}"));
    format!("{}{body}{}", &text[..start], &text[stop..])
}

#[test]
fn aspect_registration_is_generated_from_every_lang_json() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let aspects = every_lang_aspect(&root, &families);
    let mut by_label: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for (label, name) in &aspects {
        by_label.entry(label).or_default().push(name);
    }
    let loads: String = by_label
        .iter()
        .map(|(label, names)| {
            let symbols: Vec<String> = names.iter().map(|n| format!("_{n} = \"{n}\"")).collect();
            format!("load(\"//{label}\", {})\n", symbols.join(", "))
        })
        .collect();
    let exports: Vec<String> = aspects.iter().map(|(_, n)| format!("{n} = _{n}")).collect();
    let exports = format!("{}\n", exports.join("\n\n"));
    let current = dx_testing::read_runfiles("core/quality/testdata/all_languages.bzl");
    let text = splice(&current, LOADS_BEGIN, LOADS_END, &loads);
    let text = splice(&text, EXPORTS_BEGIN, EXPORTS_END, &exports);
    let scratch = dx_testing::mkscratch("all_languages_bzl").expect("scratch");
    let actual = scratch.join("all_languages.bzl");
    dx_testing::write_text(&actual, &text);
    let expected = dx_testing::resolve_runfiles("core/quality/testdata/all_languages.bzl");
    dx_testing::snapshot_diff(
        &expected,
        &actual,
        Some("core/quality/testdata/all_languages.bzl"),
    )
    .expect("all_languages.bzl aspect registration is generated from every lang.json");
}

#[test]
fn families_aspects_named_for_a_language_are_in_its_lang_json() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let listed: BTreeSet<&str> = families["families"]
        .as_object()
        .expect("families")
        .values()
        .flat_map(|family| family["aspects"].as_array().expect("aspects").iter())
        .map(|name| name.as_str().expect("aspect"))
        .collect();
    let mut problems = Vec::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        let own: BTreeSet<String> = lang_aspects(&manifest).into_iter().map(|(_, n)| n).collect();
        let prefix = format!("real_{}_", manifest["name"].as_str().expect("name"));
        for name in listed.iter().filter(|name| name.starts_with(&prefix)) {
            if !own.contains(*name) {
                problems.push(format!(
                    "{}: families.json lists {name} but lang.json does not",
                    dir.display()
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn gazelle_lock_registry_is_generated_from_every_lang_json() {
    let root = dx_testing::rules_dx_source();
    let families: Value = serde_json::from_str(&dx_testing::read_runfiles("core/modules/families.json"))
        .expect("families.json");
    let mut entries = std::collections::BTreeMap::new();
    for dir in language_dirs(&root, &families) {
        let text = std::fs::read_to_string(root.join(&dir).join("lang.json")).expect("lang.json");
        let manifest: Value = serde_json::from_str(&text).expect("lang.json parses");
        for hub in manifest["setup"]["hubs"].as_array().into_iter().flatten() {
            if hub["imports"] != "provides" || hub["lock"].is_null() {
                continue;
            }
            let ecosystem = hub["ecosystem"].as_str().expect("ecosystem").to_owned();
            entries.insert(
                ecosystem.clone(),
                serde_json::json!({
                    "ecosystem": ecosystem,
                    "manifest": hub["manifest"],
                    "lock": hub["lock"],
                    "hub": hub["hub"],
                }),
            );
        }
    }
    let list: Vec<Value> = entries.into_values().collect();
    let text = serde_json::to_string_pretty(&list).expect("serializes") + "\n";
    let scratch = dx_testing::mkscratch("gazelle_lock_registry").expect("scratch");
    let actual = scratch.join("lock_registry.json");
    dx_testing::write_text(&actual, &text);
    let expected = dx_testing::resolve_runfiles("gazelle/common/lock_registry.json");
    dx_testing::snapshot_diff(&expected, &actual, Some("gazelle/common/lock_registry.json"))
        .expect("gazelle/common/lock_registry.json is generated from every lang.json");
}
