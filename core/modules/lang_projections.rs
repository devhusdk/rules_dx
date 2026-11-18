//! Renders langs.bzl, lock_registry.bzl and gazelle's lock_registry.json from every lang.json.
//!
//! Usage: lang_projections <langs.bzl> <lock_registry.bzl> <lock_registry.json> <lang.json>...

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value};

const NON_LANGUAGE: [&str; 1] = ["appimage"];

const REFRESH: &str = "Generated from every <lang>/lang.json by //core/modules:lang_projections; refresh with `bazel run //core/modules:update_lang_projections`.";

fn read_manifest(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Language manifests keyed by their directory name, skipping non-language dirs.
fn manifests(paths: &[PathBuf]) -> Result<BTreeMap<String, Value>, String> {
    let mut found = BTreeMap::new();
    for path in paths {
        let dir = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{}: no language directory", path.display()))?;
        if NON_LANGUAGE.contains(&dir) {
            continue;
        }
        found.insert(dir.to_owned(), read_manifest(path)?);
    }
    Ok(found)
}

fn langs_bzl(manifests: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut langs = Map::new();
    for manifest in manifests.values() {
        let mut manifest = manifest.clone();
        let object = manifest.as_object_mut().ok_or("lang.json is not an object")?;
        object.remove("$schema");
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or("lang.json has no name")?
            .to_owned();
        langs.insert(name, manifest);
    }
    let body = serde_json::to_string_pretty(&Value::Object(langs)).map_err(|e| e.to_string())?;
    if body.contains('\\') || body.contains("\"\"\"") {
        return Err("langs.bzl cannot embed escapes or triple quotes".to_owned());
    }
    Ok(format!(
        "\"\"\"{REFRESH}\"\"\"\n\nvisibility([\"//core/...\", \"//ruby/...\", \"//tools/...\"])\n\nLANGS = json.decode(\"\"\"{body}\"\"\")\n"
    ))
}

fn lock_registry_entries(manifests: &BTreeMap<String, Value>) -> Result<Vec<Value>, String> {
    let mut entries = BTreeMap::new();
    for manifest in manifests.values() {
        for hub in manifest["setup"]["hubs"].as_array().into_iter().flatten() {
            if hub["lock"].is_null() {
                continue;
            }
            let ecosystem = hub["ecosystem"]
                .as_str()
                .ok_or("setup.hubs entry has no ecosystem")?
                .to_owned();
            entries.insert(
                ecosystem.clone(),
                serde_json::json!({
                    "ecosystem": ecosystem,
                    "manifest": hub["manifest"],
                    "lock": hub["lock"],
                    "hub": hub["hub"],
                    "imports": hub["imports"],
                    "state": hub["state"],
                }),
            );
        }
    }
    Ok(entries.into_values().collect())
}

fn lock_registry_bzl(entries: &[Value]) -> String {
    let mut text = format!(
        "\"\"\"{REFRESH}\"\"\"\n\nvisibility([\"//core/...\", \"//deploy/...\", \"//elixir/...\", \"//julia/...\", \"//python/...\", \"//tools/...\"])\n\nLOCK_REGISTRY = {{\n"
    );
    for entry in entries {
        writeln!(text, "    {}: {{", entry["ecosystem"]).expect("write to String");
        for key in ["hub", "imports", "lock", "manifest", "state"] {
            let value = if entry[key].is_null() {
                "None".to_owned()
            } else {
                entry[key].to_string()
            };
            writeln!(text, "        \"{key}\": {value},").expect("write to String");
        }
        text.push_str("    },\n");
    }
    text.push_str("}\n");
    text
}

fn lock_registry_json(entries: &[Value]) -> Result<String, String> {
    let mut entries = entries.to_vec();
    for entry in &mut entries {
        if let Some(object) = entry.as_object_mut() {
            object.remove("state");
        }
    }
    Ok(serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())? + "\n")
}

fn run(args: &[PathBuf]) -> Result<(), String> {
    let [langs_out, registry_bzl_out, registry_json_out, inputs @ ..] = args else {
        return Err(
            "usage: lang_projections <langs.bzl> <lock_registry.bzl> <lock_registry.json> <lang.json>..."
                .to_owned(),
        );
    };
    let manifests = manifests(inputs)?;
    let entries = lock_registry_entries(&manifests)?;
    for (path, text) in [
        (langs_out, langs_bzl(&manifests)?),
        (registry_bzl_out, lock_registry_bzl(&entries)),
        (registry_json_out, lock_registry_json(&entries)?),
    ] {
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lang_projections: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BTreeMap<String, Value> {
        let mut found = BTreeMap::new();
        found.insert(
            "py".to_owned(),
            serde_json::json!({
                "$schema": "../core/modules/lang.schema.json",
                "name": "py",
                "setup": {"hubs": [
                    {"ecosystem": "pip", "manifest": "req.in", "lock": "req.txt", "hub": "pypi", "imports": "provides", "state": null},
                    {"ecosystem": "nolock", "manifest": "x", "lock": null, "hub": "h", "imports": "provides", "state": null},
                ]},
            }),
        );
        found
    }

    #[test]
    fn langs_bzl_drops_the_schema_key() {
        let text = langs_bzl(&sample()).unwrap();
        assert!(text.contains("\"py\": {"), "{text}");
        assert!(!text.contains("$schema"), "{text}");
        assert!(text.contains("bazel run //core/modules:update_lang_projections"));
    }

    #[test]
    fn lock_registry_skips_hubs_without_a_lock() {
        let entries = lock_registry_entries(&sample()).unwrap();
        assert_eq!(entries.len(), 1);
        let bzl = lock_registry_bzl(&entries);
        assert!(bzl.contains("    \"pip\": {\n        \"hub\": \"pypi\","), "{bzl}");
        assert!(bzl.contains("\"state\": None,"), "{bzl}");
        let json = lock_registry_json(&entries).unwrap();
        assert!(!json.contains("state"), "{json}");
        assert!(!json.contains("nolock"), "{json}");
    }

    #[test]
    fn manifests_skip_non_language_dirs() {
        let found = manifests(&[PathBuf::from("appimage/lang.json")]).unwrap();
        assert!(found.is_empty());
    }
}
