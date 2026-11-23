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

/// The `<kind>=<path>` arguments; `all_languages_src`, `core_aspects` and `core_probe_src` are the
/// inputs.
const KINDS: [&str; 11] = [
    "all_languages.bzl",
    "all_languages_src",
    "conformance_quality.bzl",
    "core_aspects",
    "core_probe.bzl",
    "core_probe_src",
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
const TABLE_LOADS_BEGIN: &str = "# BEGIN GENERATED FAMILY TABLE LOADS\n";
const TABLE_LOADS_END: &str = "# END GENERATED FAMILY TABLE LOADS\n";
const TABLES_BEGIN: &str = "# BEGIN GENERATED FAMILY TABLES\n";
const TABLES_END: &str = "# END GENERATED FAMILY TABLES\n";

/// The family tables every quality file exports, and the union each one becomes.
const TOOLS: (&str, &str) = ("TOOLS", " | ");
const SHARD_SPECS: (&str, &str) = ("SHARD_SPECS", " | ");
const HOOKS: (&str, &str) = ("HOOKS", " | ");
const HOOK_ORDER: (&str, &str) = ("HOOK_ORDER", " + ");

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
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// The keys a core aspect manifest row carries about its file family.
const CORE_FAMILY_KEYS: [&str; 3] = ["owner", "classes", "defaults"];

/// The file families of the core aspect manifest, keyed by `family`, with each row's owner, classes
/// and defaults.
fn core_families(core: &Value) -> Result<Map<String, Value>, String> {
    let rows = core["quality"]
        .as_array()
        .ok_or("core_aspects quality is not a list")?;
    let mut families = Map::new();
    for row in rows {
        let Some(family) = row.get("family") else {
            let names_aspects = ["aspects", "probes"]
                .iter()
                .any(|key| row.get(*key).is_some_and(|value| !value.is_null()));
            if !names_aspects {
                return Err(format!(
                    "core_aspects row {} names no family, aspects or probes",
                    row["bzl"]
                ));
            }
            continue;
        };
        let family = str_at(family, "core_aspects family")?;
        let mut entry = Map::new();
        for key in CORE_FAMILY_KEYS {
            if let Some(value) = row.get(key) {
                entry.insert(key.to_owned(), value.clone());
            }
        }
        if !entry.contains_key("owner") || !entry.contains_key("classes") {
            return Err(format!(
                "core_aspects family {family} needs an owner and classes"
            ));
        }
        if families
            .insert(family.to_owned(), Value::Object(entry))
            .is_some()
        {
            return Err(format!("core_aspects family {family} is listed twice"));
        }
    }
    Ok(families)
}

/// `NAME = json.decode("""...""")` for a value with no escapes or triple quotes.
fn json_decode_line(name: &str, value: Value) -> Result<String, String> {
    let body = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    if body.contains('\\') || body.contains("\"\"\"") {
        return Err("langs.bzl cannot embed escapes or triple quotes".to_owned());
    }
    Ok(format!("{name} = json.decode(\"\"\"{body}\"\"\")\n"))
}

fn langs_bzl(manifests: &BTreeMap<String, Value>, core: &Value) -> Result<String, String> {
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
    let langs = json_decode_line("LANGS", Value::Object(langs))?;
    let core = json_decode_line("CORE_FAMILIES", Value::Object(core_families(core)?))?;
    Ok(format!(
        "\"\"\"{REFRESH}\"\"\"\n\nvisibility([\"//core/...\", \"//ruby/...\", \"//tools/...\"])\n\n{langs}\n{core}"
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

/// `rows` sorted by language name.
fn by_name(mut rows: Vec<(String, Value)>) -> Vec<Value> {
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, row)| row).collect()
}

/// `{"languages": rows}` with rows sorted by language name.
fn languages_table(rows: Vec<(String, Value)>) -> Result<String, String> {
    pretty(&serde_json::json!({ "languages": by_name(rows) }))
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

/// The `quality` and `docs` rows of `core/quality/core_aspects.json`, each as a manifest with that
/// one entry.
fn core_manifests(core: &Value) -> Result<Vec<Value>, String> {
    let mut found = Vec::new();
    for key in ["quality", "docs"] {
        let rows = core[key]
            .as_array()
            .ok_or_else(|| format!("core_aspects {key} is not a list"))?;
        found.extend(rows.iter().map(|row| serde_json::json!({ key: row })));
    }
    Ok(found)
}

fn families_languages_json(
    manifests: &BTreeMap<String, Value>,
    core: &Value,
) -> Result<String, String> {
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
    pretty(&serde_json::json!({ "core": core, "languages": by_name(rows) }))
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

/// Whether `name` is a shard aspect. Others, like `android_sdk_aspect`, only provide for the shards
/// after them.
fn is_shard(name: &str) -> bool {
    name.starts_with("real_")
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

/// The quality files whose rows name shard aspects, so they export the family tables. Probe-only
/// files, like `core_probe.bzl`, merge those tables and export none.
fn table_files<'a>(
    manifests: impl IntoIterator<Item = &'a Value>,
) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    for manifest in manifests {
        let Some(quality) = manifest.get("quality") else {
            continue;
        };
        if quality
            .get("aspects")
            .is_some_and(|aspects| !aspects.is_null())
        {
            found.insert(str_at(&quality["bzl"], "quality.bzl")?.to_owned());
        }
    }
    Ok(found)
}

/// `_<FILE>_` for a quality file label, such as `_CC_` for `cc/private/quality:cc.bzl`.
fn table_prefix(label: &str) -> Result<String, String> {
    let stem = label
        .rsplit(['/', ':'])
        .next()
        .and_then(|file| file.strip_suffix(".bzl"))
        .ok_or_else(|| format!("{label}: quality.bzl is not a .bzl file"))?;
    Ok(format!("_{}_", stem.to_uppercase()))
}

/// Each table file's load symbols, `(local, exported)`, for `tables`.
fn table_symbols(
    files: &BTreeSet<String>,
    tables: &[(&str, &str)],
) -> Result<BTreeMap<String, Vec<(String, String)>>, String> {
    let mut found = BTreeMap::new();
    let mut owners: BTreeMap<String, &str> = BTreeMap::new();
    for label in files {
        let prefix = table_prefix(label)?;
        if let Some(other) = owners.insert(prefix.clone(), label) {
            return Err(format!(
                "{other} and {label} share the table prefix {prefix}"
            ));
        }
        let symbols = tables
            .iter()
            .map(|(table, _)| (format!("{prefix}{table}"), (*table).to_owned()))
            .collect();
        found.insert(label.clone(), symbols);
    }
    Ok(found)
}

/// One `_FAMILY_<TABLE> = ...` line per table, joining every file's table in label order.
fn table_unions(files: &BTreeSet<String>, tables: &[(&str, &str)]) -> Result<String, String> {
    let mut text = String::new();
    for (table, join) in tables {
        let parts = files
            .iter()
            .map(|label| table_prefix(label).map(|prefix| format!("{prefix}{table}")))
            .collect::<Result<Vec<_>, _>>()?;
        if parts.is_empty() {
            return Err(format!("no quality file exports {table}"));
        }
        writeln!(text, "_FAMILY_{table} = {}", parts.join(join)).map_err(|e| e.to_string())?;
    }
    Ok(text)
}

/// `load(...)` text for `symbols`, `(local, exported)`, sorted by local name as buildifier keeps them.
fn load_line(label: &str, mut symbols: Vec<(String, String)>) -> String {
    symbols.sort();
    let symbols: Vec<String> = symbols
        .iter()
        .map(|(local, exported)| format!("{local} = \"{exported}\""))
        .collect();
    format!("load(\"//{label}\", {})\n", symbols.join(", "))
}

/// Rewrites the generated blocks of `core_probe.bzl`: the core rows' family tables and their unions.
fn core_probe_bzl(core: &Value, current: &str) -> Result<String, String> {
    let tables = [TOOLS, SHARD_SPECS, HOOK_ORDER, HOOKS];
    let files = table_files(&core_manifests(core)?)?;
    let mut loads = String::new();
    for (label, symbols) in table_symbols(&files, &tables)? {
        loads.push_str(&load_line(&label, symbols));
    }
    let text = splice(current, TABLE_LOADS_BEGIN, TABLE_LOADS_END, &loads)?;
    splice(
        &text,
        TABLES_BEGIN,
        TABLES_END,
        &table_unions(&files, &tables)?,
    )
}

fn all_languages_bzl(
    manifests: &BTreeMap<String, Value>,
    core: &Value,
    current: &str,
) -> Result<String, String> {
    let core = core_manifests(core)?;
    let tables = [TOOLS, SHARD_SPECS, HOOKS];
    let files = table_files(manifests.values().chain(&core))?;
    let mut table_loads = table_symbols(&files, &tables)?;
    let mut aspects = BTreeSet::new();
    let mut shard_aspects = BTreeSet::new();
    for manifest in manifests.values().chain(&core) {
        aspects.extend(lang_aspects(manifest)?);
        if let Some(groups @ Value::Object(_)) =
            manifest.get("quality").and_then(|q| q.get("aspects"))
        {
            shard_aspects.extend(aspect_names(groups)?.into_iter().filter(|n| is_shard(n)));
        }
    }
    let mut by_label: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (label, name) in &aspects {
        by_label.entry(label).or_default().push(name);
    }
    let mut loads = String::new();
    for (label, names) in &by_label {
        let mut symbols = table_loads.remove(*label).unwrap_or_default();
        symbols.extend(names.iter().map(|n| (format!("_{n}"), (*n).to_owned())));
        loads.push_str(&load_line(label, symbols));
    }
    if let Some(label) = table_loads.keys().next() {
        return Err(format!("{label} names no aspect"));
    }
    let exports: Vec<String> = aspects.iter().map(|(_, n)| format!("{n} = _{n}")).collect();
    let exports = format!("{}\n", exports.join("\n\n"));
    let mut shards = String::from("_SHARD_ASPECTS = {\n");
    for name in &shard_aspects {
        let shard = name
            .strip_suffix("_aspect")
            .ok_or_else(|| format!("{name}: aspect names end in _aspect"))?;
        writeln!(shards, "    \"{shard}\": _{name},").map_err(|e| e.to_string())?;
    }
    shards.push_str("}\n");
    let text = splice(current, LOADS_BEGIN, LOADS_END, &loads)?;
    let text = splice(&text, EXPORTS_BEGIN, EXPORTS_END, &exports)?;
    let text = splice(
        &text,
        TABLES_BEGIN,
        TABLES_END,
        &table_unions(&files, &tables)?,
    )?;
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
            let names: Vec<String> = aspect_names(names)?
                .into_iter()
                .filter(|n| is_shard(n))
                .collect();
            if names.is_empty() {
                continue;
            }
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
    core: &Value,
    named: &BTreeMap<&str, PathBuf>,
) -> Result<String, String> {
    match kind {
        "all_languages.bzl" => {
            let src = named
                .get("all_languages_src")
                .ok_or("all_languages.bzl needs all_languages_src=<path>")?;
            let current =
                std::fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
            all_languages_bzl(manifests, core, &current)
        }
        "conformance_quality.bzl" => conformance_quality_bzl(manifests),
        "core_probe.bzl" => {
            let src = named
                .get("core_probe_src")
                .ok_or("core_probe.bzl needs core_probe_src=<path>")?;
            let current =
                std::fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
            core_probe_bzl(core, &current)
        }
        "ecosystems_languages.json" => ecosystems_languages_json(manifests),
        "families_languages.json" => families_languages_json(manifests, core),
        "langs.bzl" => langs_bzl(manifests, core),
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
    let core = read_manifest(
        named
            .get("core_aspects")
            .ok_or("needs core_aspects=<core_aspects.json>")?,
    )?;
    for (kind, path) in &named {
        if matches!(
            *kind,
            "all_languages_src" | "core_aspects" | "core_probe_src"
        ) {
            continue;
        }
        let text = render(kind, &manifests, &core, &named)?;
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

    fn no_core() -> Value {
        serde_json::json!({"quality": [], "docs": []})
    }

    #[test]
    fn langs_bzl_drops_the_schema_key() {
        let text = langs_bzl(&sample(), &no_core()).unwrap();
        assert!(text.contains("\"py\": {"), "{text}");
        assert!(!text.contains("$schema"), "{text}");
        assert!(text.contains("bazel run //core/modules:update_lang_projections"));
    }

    #[test]
    fn langs_bzl_writes_the_core_families() {
        let core = serde_json::json!({"quality": [
            {"bzl": "core/quality:families/core.bzl", "family": "core", "owner": "other",
             "classes": ["github_workflow"], "file_extensions": [".yml"],
             "aspects": {"lint": ["real_lint_aspect"]}},
            {"bzl": "core/quality:families/core.bzl", "family": "toml", "owner": "other",
             "classes": ["toml"], "defaults": {"lint": ["taplo"]}},
            {"bzl": "core/quality:families/core_probe.bzl", "probes": {"lint": ["real_core_lint_probe_aspect"]}},
        ], "docs": []});
        let families = core_families(&core).unwrap();
        assert_eq!(
            Value::Object(families),
            serde_json::json!({
                "core": {"owner": "other", "classes": ["github_workflow"]},
                "toml": {"owner": "other", "classes": ["toml"], "defaults": {"lint": ["taplo"]}},
            })
        );
        let text = langs_bzl(&sample(), &core).unwrap();
        assert!(
            text.contains("\nCORE_FAMILIES = json.decode(\"\"\"{\n  \"core\": {"),
            "{text}"
        );
        assert!(!text.contains("file_extensions"), "{text}");
    }

    #[test]
    fn core_families_need_an_owner_and_classes_once() {
        let row = |family: &str, owner: Option<&str>| {
            let mut row = serde_json::json!({"bzl": "core/quality:families/core.bzl", "family": family, "classes": ["x"]});
            if let Some(owner) = owner {
                row["owner"] = Value::from(owner);
            }
            row
        };
        let core = |rows: Vec<Value>| serde_json::json!({"quality": rows, "docs": []});
        assert_eq!(
            core_families(&core(vec![row("x", None)])).unwrap_err(),
            "core_aspects family x needs an owner and classes"
        );
        assert_eq!(
            core_families(&core(vec![
                row("x", Some("other")),
                row("x", Some("other"))
            ]))
            .unwrap_err(),
            "core_aspects family x is listed twice"
        );
        let empty = serde_json::json!({"bzl": "core/quality:families/core.bzl"});
        assert_eq!(
            core_families(&core(vec![empty])).unwrap_err(),
            "core_aspects row \"core/quality:families/core.bzl\" names no family, aspects or probes"
        );
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
        let families = families_languages_json(&sample(), &no_core()).unwrap();
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
            "\"\"\"Hand header.\"\"\"\n\n{LOADS_BEGIN}stale\n{LOADS_END}\nkeep = 1\n\n{EXPORTS_BEGIN}{EXPORTS_END}\n{TABLES_BEGIN}{TABLES_END}\n{SHARDS_BEGIN}{SHARDS_END}"
        );
        let text = all_languages_bzl(&sample(), &no_core(), &current).unwrap();
        assert!(text.starts_with("\"\"\"Hand header.\"\"\""), "{text}");
        assert!(text.contains("keep = 1"), "{text}");
        assert!(!text.contains("stale"), "{text}");
        assert!(
            text.contains("load(\"//py/private/quality:py.bzl\", _PY_HOOKS = \"HOOKS\", _PY_SHARD_SPECS = \"SHARD_SPECS\", _PY_TOOLS = \"TOOLS\", _real_py_format_aspect = \"real_py_format_aspect\", _real_py_lint_aspect = \"real_py_lint_aspect\", _real_py_lint_probe_aspect = \"real_py_lint_probe_aspect\")"),
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
    fn helper_aspects_get_no_shard_and_no_conformance_subject() {
        let mut manifests = BTreeMap::new();
        manifests.insert(
            "sdk".to_owned(),
            serde_json::json!({
                "name": "sdk",
                "quality": {
                    "bzl": "sdk/private/quality:sdk.bzl",
                    "file_extensions": [],
                    "licenses": ["sdk-eula"],
                    "aspects": {"lint": ["sdk_aspect"], "typecheck": ["sdk_aspect"]}
                }
            }),
        );
        let current = format!(
            "{LOADS_BEGIN}{LOADS_END}{EXPORTS_BEGIN}{EXPORTS_END}{TABLES_BEGIN}{TABLES_END}{SHARDS_BEGIN}{SHARDS_END}"
        );
        let text = all_languages_bzl(&manifests, &no_core(), &current).unwrap();
        assert!(text.contains("sdk_aspect = _sdk_aspect"), "{text}");
        assert!(!text.contains("\"sdk\": _sdk_aspect"), "{text}");
        let conformance = conformance_quality_bzl(&manifests).unwrap();
        assert!(!conformance.contains("sdk"), "{conformance}");
    }

    #[test]
    fn core_rows_join_the_generated_blocks() {
        let core = serde_json::json!({
            "quality": [
                {"bzl": "core/quality:families/core.bzl", "aspects": {"lint": ["real_lint_aspect"]}},
                {"bzl": "core/quality:families/core_probe.bzl", "probes": {"lint": ["real_core_lint_probe_aspect"]}},
            ],
            "docs": [{"bzl": "tools/docs:aspect.bzl", "aspects": ["real_docs_aspect"]}],
        });
        let current = format!(
            "{LOADS_BEGIN}{LOADS_END}{EXPORTS_BEGIN}{EXPORTS_END}{TABLES_BEGIN}{TABLES_END}{SHARDS_BEGIN}{SHARDS_END}"
        );
        let text = all_languages_bzl(&sample(), &core, &current).unwrap();
        for expected in [
            "load(\"//core/quality:families/core.bzl\", _CORE_HOOKS = \"HOOKS\", _CORE_SHARD_SPECS = \"SHARD_SPECS\", _CORE_TOOLS = \"TOOLS\", _real_lint_aspect = \"real_lint_aspect\")",
            "_FAMILY_TOOLS = _CORE_TOOLS | _PY_TOOLS\n",
            "_FAMILY_HOOKS = _CORE_HOOKS | _PY_HOOKS\n",
            "load(\"//core/quality:families/core_probe.bzl\", _real_core_lint_probe_aspect = \"real_core_lint_probe_aspect\")",
            "load(\"//tools/docs:aspect.bzl\", _real_docs_aspect = \"real_docs_aspect\")",
            "real_core_lint_probe_aspect = _real_core_lint_probe_aspect",
            "real_docs_aspect = _real_docs_aspect",
            "_SHARD_ASPECTS = {\n",
            "    \"real_lint\": _real_lint_aspect,",
            "    \"real_py_lint\": _real_py_lint_aspect,",
        ] {
            assert!(text.contains(expected), "{expected} not in {text}");
        }
        assert!(!text.contains("\"real_core_lint_probe\""), "{text}");
        let families = families_languages_json(&sample(), &core).unwrap();
        let families: Value = serde_json::from_str(&families).unwrap();
        assert_eq!(families["core"], core);
        assert_eq!(families["languages"][0]["name"], "py");
    }

    #[test]
    fn core_probe_bzl_merges_the_core_rows_that_name_aspects() {
        let core = serde_json::json!({
            "quality": [
                {"bzl": "core/quality:families/core.bzl", "aspects": {"lint": ["real_lint_aspect"]}},
                {"bzl": "core/quality:families/core_probe.bzl", "probes": {"lint": ["real_core_lint_probe_aspect"]}},
                {"bzl": "core/quality:families/yaml.bzl", "aspects": {"format": ["real_yaml_format_aspect"]}},
            ],
            "docs": [{"bzl": "tools/docs:aspect.bzl", "aspects": ["real_docs_aspect"]}],
        });
        let current = format!(
            "\"\"\"Probe.\"\"\"\n\n{TABLE_LOADS_BEGIN}stale\n{TABLE_LOADS_END}\nkeep = 1\n\n{TABLES_BEGIN}stale\n{TABLES_END}"
        );
        let text = core_probe_bzl(&core, &current).unwrap();
        assert_eq!(
            text,
            format!(
                "\"\"\"Probe.\"\"\"\n\n{TABLE_LOADS_BEGIN}\
                 load(\"//core/quality:families/core.bzl\", _CORE_HOOKS = \"HOOKS\", _CORE_HOOK_ORDER = \"HOOK_ORDER\", _CORE_SHARD_SPECS = \"SHARD_SPECS\", _CORE_TOOLS = \"TOOLS\")\n\
                 load(\"//core/quality:families/yaml.bzl\", _YAML_HOOKS = \"HOOKS\", _YAML_HOOK_ORDER = \"HOOK_ORDER\", _YAML_SHARD_SPECS = \"SHARD_SPECS\", _YAML_TOOLS = \"TOOLS\")\n\
                 {TABLE_LOADS_END}\nkeep = 1\n\n{TABLES_BEGIN}\
                 _FAMILY_TOOLS = _CORE_TOOLS | _YAML_TOOLS\n\
                 _FAMILY_SHARD_SPECS = _CORE_SHARD_SPECS | _YAML_SHARD_SPECS\n\
                 _FAMILY_HOOK_ORDER = _CORE_HOOK_ORDER + _YAML_HOOK_ORDER\n\
                 _FAMILY_HOOKS = _CORE_HOOKS | _YAML_HOOKS\n\
                 {TABLES_END}"
            )
        );
    }

    #[test]
    fn table_files_with_one_stem_fail() {
        let files: BTreeSet<String> = ["a/private/quality:x.bzl", "b/private/quality:x.bzl"]
            .map(str::to_owned)
            .into();
        let error = table_symbols(&files, &[TOOLS]).unwrap_err();
        assert!(error.contains("share the table prefix _X_"), "{error}");
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
