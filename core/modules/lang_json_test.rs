//! Every language dir named in families.json has a lang.json that satisfies lang.schema.json.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const NON_LANGUAGE: [&str; 32] = [
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
    "quality",
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

fn item_problems(path: &str, items: &[Value], schema: &Value, defs: &Value) -> Vec<String> {
    let schema = match schema["$ref"].as_str() {
        Some(reference) => &defs[reference.trim_start_matches("#/$defs/")],
        None => schema,
    };
    let mut problems: Vec<String> = type_problem(path, &Value::Array(items.to_vec()), schema)
        .into_iter()
        .collect();
    let seen: BTreeSet<String> = items.iter().map(Value::to_string).collect();
    if schema["uniqueItems"] == true && seen.len() != items.len() {
        problems.push(format!("{path}: duplicate items"));
    }
    let item_schema = &schema["items"];
    for item in items {
        if let Some(allowed) = item_schema["enum"].as_array() {
            if !allowed.contains(item) {
                problems.push(format!("{path}: {item} is not allowed"));
            }
        }
        match item.as_str() {
            Some(text) => {
                let min = item_schema["minLength"].as_u64().unwrap_or(0) as usize;
                if text.len() < min {
                    problems.push(format!("{path}: empty string"));
                }
            }
            None => problems.push(format!("{path}: item {item} is not a string")),
        }
    }
    problems
}

fn schema_problems(manifest: &Value, schema: &Value) -> Vec<String> {
    let Some(object) = manifest.as_object() else {
        return vec!["not an object".to_owned()];
    };
    let properties = schema["properties"].as_object().expect("properties");
    let mut problems = Vec::new();
    for key in schema["required"].as_array().expect("required") {
        let key = key.as_str().expect("required key");
        if !object.contains_key(key) {
            problems.push(format!("{key}: missing"));
        }
    }
    for (key, value) in object {
        let Some(property) = properties.get(key) else {
            problems.push(format!("{key}: not in the schema"));
            continue;
        };
        if let Some(items) = value.as_array() {
            problems.extend(item_problems(key, items, property, &schema["$defs"]));
        } else if let Some(problem) = type_problem(key, value, property) {
            problems.push(problem);
        } else if property["minLength"].as_u64().is_some() && value.as_str() == Some("") {
            problems.push(format!("{key}: empty string"));
        }
    }
    problems
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
