//! Renders the checked-in projections of every lang.json.
//!
//! Usage: lang_projections <kind>=<path>... -- <lang.json>...

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value};

const NON_LANGUAGE: [&str; 1] = ["appimage"];

const REFRESH: &str = "Generated from every <lang>/lang.json by //core/modules:lang_projections; refresh with `bazel run //core/modules:update_lang_projections`.";

/// The `<kind>=<path>` arguments; `all_languages_src` is the only input.
const KINDS: [&str; 8] = [
    "all_languages.bzl",
    "all_languages_src",
    "conformance_quality.bzl",
    "ecosystems_languages.json",
    "families_languages.json",
    "langs.bzl",
    "lock_registry.bzl",
    "lock_registry.json",
];

const LOADS_BEGIN: &str = "# BEGIN GENERATED LANG ASPECT LOADS\n";
const LOADS_END: &str = "# END GENERATED LANG ASPECT LOADS\n";
const EXPORTS_BEGIN: &str = "# BEGIN GENERATED LANG ASPECT EXPORTS\n";
const EXPORTS_END: &str = "# END GENERATED LANG ASPECT EXPORTS\n";
const SHARDS_BEGIN: &str = "# BEGIN GENERATED LANG SHARD ASPECTS\n";
const SHARDS_END: &str = "# END GENERATED LANG SHARD ASPECTS\n";

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

fn str_at<'a>(value: &'a Value, what: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{what} is not a string"))
}

fn pretty(value: &Value) -> Result<String, String> {
    Ok(serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n")
}

fn langs_bzl(manifests: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut langs = Map::new();
    for manifest in manifests.values() {
        let mut manifest = manifest.clone();
        let object = manifest
            .as_object_mut()
            .ok_or("lang.json is not an object")?;
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
        let hubs = manifest
            .get("setup")
            .and_then(|setup| setup.get("hubs"))
            .and_then(Value::as_array);
        for hub in hubs.into_iter().flatten() {
            if hub["lock"].is_null() {
                continue;
            }
            let ecosystem = str_at(&hub["ecosystem"], "setup.hubs ecosystem")?.to_owned();
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

fn lock_registry_bzl(entries: &[Value]) -> Result<String, String> {
    let mut text = format!(
        "\"\"\"{REFRESH}\"\"\"\n\nvisibility([\"//core/...\", \"//deploy/...\", \"//elixir/...\", \"//julia/...\", \"//python/...\", \"//tools/...\"])\n\nLOCK_REGISTRY = {{\n"
    );
    for entry in entries {
        writeln!(text, "    {}: {{", entry["ecosystem"]).map_err(|e| e.to_string())?;
        for key in ["hub", "imports", "lock", "manifest", "state"] {
            let value = if entry[key].is_null() {
                "None".to_owned()
            } else {
                entry[key].to_string()
            };
            writeln!(text, "        \"{key}\": {value},").map_err(|e| e.to_string())?;
        }
        text.push_str("    },\n");
    }
    text.push_str("}\n");
    Ok(text)
}

fn lock_registry_json(entries: &[Value]) -> Result<String, String> {
    let mut entries = entries.to_vec();
    for entry in &mut entries {
        if let Some(object) = entry.as_object_mut() {
            object.remove("state");
        }
    }
    pretty(&Value::Array(entries))
}

/// `{"languages": rows}` with rows sorted by language name.
fn languages_table(mut rows: Vec<(String, Value)>) -> Result<String, String> {
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let rows: Vec<Value> = rows.into_iter().map(|(_, row)| row).collect();
    pretty(&serde_json::json!({ "languages": rows }))
}

fn ecosystems_languages_json(manifests: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut rows = Vec::new();
    for manifest in manifests.values() {
        if manifest["depcheck"].is_null() {
            continue;
        }
        rows.push((
            str_at(&manifest["name"], "name")?.to_owned(),
            serde_json::json!({
                "name": manifest["name"],
                "depcheck": manifest["depcheck"],
                "semver": manifest["semver"],
                "ecosystems": manifest["ecosystems"],
            }),
        ));
    }
    languages_table(rows)
}

fn families_languages_json(manifests: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut rows = Vec::new();
    for manifest in manifests.values() {
        let mut row = Map::new();
        row.insert("name".to_owned(), manifest["name"].clone());
        for key in ["quality", "docs", "setup", "commands"] {
            if !manifest[key].is_null() {
                row.insert(key.to_owned(), manifest[key].clone());
            }
        }
        if row.len() > 1 {
            rows.push((
                str_at(&manifest["name"], "name")?.to_owned(),
                Value::Object(row),
            ));
        }
    }
    languages_table(rows)
}

fn aspect_names(value: &Value) -> Result<Vec<String>, String> {
    let items: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(groups) => groups
            .values()
            .flat_map(|group| group.as_array().into_iter().flatten())
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Vec::new(),
    };
    items
        .into_iter()
        .map(|name| str_at(name, "aspect").map(str::to_owned))
        .collect()
}

/// Every (bzl label, aspect) pair a lang.json names under quality and docs.
fn lang_aspects(manifest: &Value) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    for (section, table_key) in [
        ("aspects", "quality"),
        ("probes", "quality"),
        ("aspects", "docs"),
    ] {
        let Some(table) = manifest.get(table_key) else {
            continue;
        };
        let label = str_at(&table["bzl"], "bzl")?;
        for name in aspect_names(&table[section])? {
            found.push((label.to_owned(), name));
        }
    }
    Ok(found)
}

fn splice(text: &str, begin: &str, end: &str, body: &str) -> Result<String, String> {
    let start = text
        .find(begin)
        .ok_or_else(|| format!("missing {begin:?}"))?
        + begin.len();
    let stop = text.find(end).ok_or_else(|| format!("missing {end:?}"))?;
    match (text.get(..start), text.get(stop..)) {
        (Some(head), Some(tail)) if start <= stop => Ok(format!("{head}{body}{tail}")),
        _ => Err(format!("{begin:?} must come before {end:?}")),
    }
}

fn all_languages_bzl(manifests: &BTreeMap<String, Value>, current: &str) -> Result<String, String> {
    let mut aspects = BTreeSet::new();
    let mut shard_aspects = BTreeSet::new();
    for manifest in manifests.values() {
        aspects.extend(lang_aspects(manifest)?);
        if let Some(groups @ Value::Object(_)) =
            manifest.get("quality").and_then(|q| q.get("aspects"))
        {
            shard_aspects.extend(aspect_names(groups)?);
        }
    }
    let mut by_label: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (label, name) in &aspects {
        by_label.entry(label).or_default().push(name);
    }
    let mut loads = String::new();
    for (label, names) in &by_label {
        let symbols: Vec<String> = names.iter().map(|n| format!("_{n} = \"{n}\"")).collect();
        writeln!(loads, "load(\"//{label}\", {})", symbols.join(", "))
            .map_err(|e| e.to_string())?;
    }
    let exports: Vec<String> = aspects.iter().map(|(_, n)| format!("{n} = _{n}")).collect();
    let exports = format!("{}\n", exports.join("\n\n"));
    let mut shards = String::from("_LANG_SHARD_ASPECTS = {\n");
    for name in &shard_aspects {
        let shard = name
            .strip_suffix("_aspect")
            .ok_or_else(|| format!("{name}: aspect names end in _aspect"))?;
        writeln!(shards, "    \"{shard}\": _{name},").map_err(|e| e.to_string())?;
    }
    shards.push_str("}\n");
    let text = splice(current, LOADS_BEGIN, LOADS_END, &loads)?;
    let text = splice(&text, EXPORTS_BEGIN, EXPORTS_END, &exports)?;
    splice(&text, SHARDS_BEGIN, SHARDS_END, &shards)
}

fn conformance_quality_bzl(manifests: &BTreeMap<String, Value>) -> Result<String, String> {
    // Keyed by (package, file) so the loads sort by package first, as buildifier keeps them.
    let mut loads: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut subjects = Vec::new();
    for (lang, manifest) in manifests {
        let quality = &manifest["quality"];
        let mut kinds = Vec::new();
        for kind in ["format", "lint"] {
            let Some(names) = quality
                .get("aspects")
                .and_then(|aspects| aspects.get(kind))
                .filter(|names| names.is_array())
            else {
                continue;
            };
            let names = aspect_names(names)?;
            let label = str_at(&quality["bzl"], "quality.bzl")?;
            let (package, file) = label
                .split_once(':')
                .ok_or_else(|| format!("{label}: quality.bzl is not package:file"))?;
            loads
                .entry((package.to_owned(), file.to_owned()))
                .or_default()
                .extend(names.iter().cloned());
            kinds.push((kind, names));
        }
        if !kinds.is_empty() {
            subjects.push((lang, kinds));
        }
    }
    loads
        .entry((
            "core/quality/testdata".to_owned(),
            "config_subject.bzl".to_owned(),
        ))
        .or_default()
        .insert("config_subject_rule".to_owned());
    let mut text = format!("\"\"\"{REFRESH}\"\"\"\n\n");
    for ((package, file), names) in &loads {
        let names: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
        writeln!(text, "load(\"//{package}:{file}\", {})", names.join(", "))
            .map_err(|e| e.to_string())?;
    }
    for (lang, kinds) in &subjects {
        for (kind, names) in kinds {
            write!(
                text,
                "\n{lang}_{kind}_subject = config_subject_rule([{}], [OutputGroupInfo])\n",
                names.join(", ")
            )
            .map_err(|e| e.to_string())?;
        }
    }
    text.push_str("\nQUALITY_SUBJECTS = {\n");
    for (lang, kinds) in &subjects {
        writeln!(text, "    \"{lang}\": {{").map_err(|e| e.to_string())?;
        for (kind, _) in kinds {
            writeln!(text, "        \"{kind}\": {lang}_{kind}_subject,")
                .map_err(|e| e.to_string())?;
        }
        text.push_str("    },\n");
    }
    text.push_str("}\n");
    Ok(text)
}

/// Splits `<kind>=<path>... -- <lang.json>...` into named paths and inputs.
fn parse(args: &[OsString]) -> Result<(BTreeMap<&str, PathBuf>, Vec<PathBuf>), String> {
    let mut named = BTreeMap::new();
    let mut rest = args.iter();
    for arg in rest.by_ref() {
        let arg = arg
            .to_str()
            .ok_or_else(|| format!("{}: not UTF-8", arg.to_string_lossy()))?;
        if arg == "--" {
            break;
        }
        let (kind, path) = arg
            .split_once('=')
            .and_then(|(kind, path)| KINDS.iter().find(|k| **k == kind).map(|k| (*k, path)))
            .ok_or_else(|| {
                format!(
                    "{arg}: expected <kind>=<path>, kind one of {}",
                    KINDS.join(", ")
                )
            })?;
        if named.insert(kind, PathBuf::from(path)).is_some() {
            return Err(format!("{kind} given twice"));
        }
    }
    Ok((named, rest.map(PathBuf::from).collect()))
}

fn render(
    kind: &str,
    manifests: &BTreeMap<String, Value>,
    named: &BTreeMap<&str, PathBuf>,
) -> Result<String, String> {
    match kind {
        "all_languages.bzl" => {
            let src = named
                .get("all_languages_src")
                .ok_or("all_languages.bzl needs all_languages_src=<path>")?;
            let current =
                std::fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
            all_languages_bzl(manifests, &current)
        }
        "conformance_quality.bzl" => conformance_quality_bzl(manifests),
        "ecosystems_languages.json" => ecosystems_languages_json(manifests),
        "families_languages.json" => families_languages_json(manifests),
        "langs.bzl" => langs_bzl(manifests),
        "lock_registry.bzl" => lock_registry_bzl(&lock_registry_entries(manifests)?),
        "lock_registry.json" => lock_registry_json(&lock_registry_entries(manifests)?),
        _ => Err(format!("{kind}: not an output")),
    }
}

fn run(args: &[OsString]) -> Result<(), String> {
    let (named, inputs) = parse(args)?;
    if inputs.is_empty() {
        return Err("usage: lang_projections <kind>=<path>... -- <lang.json>...".to_owned());
    }
    let manifests = manifests(&inputs)?;
    for (kind, path) in &named {
        if *kind == "all_languages_src" {
            continue;
        }
        let text = render(kind, &manifests, &named)?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
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
                "depcheck": "python",
                "semver": "pep440",
                "ecosystems": ["pip"],
                "quality": {
                    "bzl": "py/private/quality:py.bzl",
                    "aspects": {"format": ["real_py_format_aspect"], "lint": ["real_py_lint_aspect"]},
                    "probes": {"lint": ["real_py_lint_probe_aspect"]},
                },
                "docs": {"bzl": "py/private/docs:docs.bzl", "aspects": ["real_py_docs_aspect"]},
                "commands": {"python_interpreter": "@rules_dx//py:python"},
                "setup": {"hubs": [
                    {"ecosystem": "pip", "manifest": "req.in", "lock": "req.txt", "hub": "pypi", "imports": "provides", "state": null},
                    {"ecosystem": "nolock", "manifest": "x", "lock": null, "hub": "h", "imports": "provides", "state": null},
                ]},
            }),
        );
        found.insert("bare".to_owned(), serde_json::json!({"name": "bare"}));
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
        let bzl = lock_registry_bzl(&entries).unwrap();
        assert!(
            bzl.contains("    \"pip\": {\n        \"hub\": \"pypi\","),
            "{bzl}"
        );
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

    #[test]
    fn languages_tables_keep_only_languages_with_entries() {
        let ecosystems = ecosystems_languages_json(&sample()).unwrap();
        assert!(
            ecosystems.contains("\"depcheck\": \"python\""),
            "{ecosystems}"
        );
        assert!(!ecosystems.contains("bare"), "{ecosystems}");
        let families = families_languages_json(&sample()).unwrap();
        assert!(families.contains("\"quality\": {"), "{families}");
        assert!(
            families.contains("\"python_interpreter\": \"@rules_dx//py:python\""),
            "{families}"
        );
        assert!(!families.contains("bare"), "{families}");
        assert!(!families.contains("depcheck"), "{families}");
    }

    #[test]
    fn all_languages_bzl_rewrites_only_the_generated_blocks() {
        let current = format!(
            "\"\"\"Hand header.\"\"\"\n\n{LOADS_BEGIN}stale\n{LOADS_END}\nkeep = 1\n\n{EXPORTS_BEGIN}{EXPORTS_END}\n{SHARDS_BEGIN}{SHARDS_END}"
        );
        let text = all_languages_bzl(&sample(), &current).unwrap();
        assert!(text.starts_with("\"\"\"Hand header.\"\"\""), "{text}");
        assert!(text.contains("keep = 1"), "{text}");
        assert!(!text.contains("stale"), "{text}");
        assert!(
            text.contains("load(\"//py/private/quality:py.bzl\", _real_py_format_aspect = \"real_py_format_aspect\", _real_py_lint_aspect = \"real_py_lint_aspect\", _real_py_lint_probe_aspect = \"real_py_lint_probe_aspect\")"),
            "{text}"
        );
        assert!(
            text.contains("real_py_docs_aspect = _real_py_docs_aspect"),
            "{text}"
        );
        assert!(
            text.contains("    \"real_py_lint\": _real_py_lint_aspect,"),
            "{text}"
        );
        assert!(!text.contains("\"real_py_lint_probe\""), "{text}");
    }

    #[test]
    fn splice_fails_without_markers() {
        assert!(splice("no markers", LOADS_BEGIN, LOADS_END, "").is_err());
    }

    #[test]
    fn conformance_quality_bzl_lists_format_and_lint_subjects() {
        let text = conformance_quality_bzl(&sample()).unwrap();
        assert!(
            text.contains("bazel run //core/modules:update_lang_projections"),
            "{text}"
        );
        assert!(
            text.contains(
                "py_lint_subject = config_subject_rule([real_py_lint_aspect], [OutputGroupInfo])"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "load(\"//core/quality/testdata:config_subject.bzl\", \"config_subject_rule\")"
            ),
            "{text}"
        );
        assert!(text.contains("    \"py\": {\n        \"format\": py_format_subject,\n        \"lint\": py_lint_subject,\n    },"), "{text}");
        assert!(!text.contains("bare"), "{text}");
    }

    #[test]
    fn parse_rejects_unknown_kinds() {
        let args: Vec<OsString> = ["nope=x", "--", "a/lang.json"].map(OsString::from).to_vec();
        assert!(parse(&args).is_err());
        let args: Vec<OsString> = ["langs.bzl=out", "--", "a/lang.json"]
            .map(OsString::from)
            .to_vec();
        let (named, inputs) = parse(&args).unwrap();
        assert_eq!(named.get("langs.bzl"), Some(&PathBuf::from("out")));
        assert_eq!(inputs, vec![PathBuf::from("a/lang.json")]);
    }
}
