#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{ModuleError, ModuleFile, compare_module_versions};
use std::cmp::Ordering;

pub const OVERRIDES_ENV: &str = "DX_CONSUMER_OVERRIDES";

pub const TESTED_KEYWORD: &str = "tested";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pins {
    pub floor: String,
    pub tested: String,
}

pub type Table = BTreeMap<String, Pins>;

pub type Overrides = BTreeMap<String, String>;

pub fn parse_table(json: &str) -> Result<Table, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("table JSON: {error}"))?;
    let object = value.as_object().ok_or("table JSON is not an object")?;
    let mut table = Table::new();
    for (module, pins) in object {
        let pin = |key: &str| {
            pins.get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("table entry {module:?} has no string {key:?}"))
        };
        table.insert(
            module.clone(),
            Pins {
                floor: pin("floor")?,
                tested: pin("tested")?,
            },
        );
    }
    Ok(table)
}

fn safe(token: &str) -> bool {
    !token.is_empty()
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
}

pub fn parse_pairs(spec: &str) -> Result<Overrides, String> {
    let mut overrides = Overrides::new();
    for item in spec
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let Some((module, version)) = item.split_once('=') else {
            return Err(format!(
                "{OVERRIDES_ENV} item {item:?} is not module=version"
            ));
        };
        let (module, version) = (module.trim(), version.trim());
        if !safe(module) || !safe(version) {
            return Err(format!(
                "{OVERRIDES_ENV} item {item:?} needs a module name and a version of letters, digits and . _ + -"
            ));
        }
        if overrides
            .insert(module.to_owned(), version.to_owned())
            .is_some()
        {
            return Err(format!("{OVERRIDES_ENV} names {module:?} twice"));
        }
    }
    Ok(overrides)
}

pub fn parse_overrides(spec: &str, table: &Table) -> Result<Overrides, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(Overrides::new());
    }
    if spec == TESTED_KEYWORD {
        return Ok(table
            .iter()
            .map(|(module, pins)| (module.clone(), pins.tested.clone()))
            .collect());
    }
    let mut overrides = Overrides::new();
    for item in spec.split(',') {
        let item = item.trim();
        let Some((module, version)) = item.split_once('=') else {
            return Err(format!(
                "{OVERRIDES_ENV} item {item:?} is not module=version"
            ));
        };
        let (module, version) = (module.trim(), version.trim());
        if !safe(module) || !safe(version) {
            return Err(format!(
                "{OVERRIDES_ENV} item {item:?} needs a module name and a version of letters, digits and . _ + -"
            ));
        }
        if !table.contains_key(module) {
            return Err(format!(
                "{OVERRIDES_ENV} names {module:?}, which is not in CONSUMER_BAZEL_DEPS"
            ));
        }
        if overrides
            .insert(module.to_owned(), version.to_owned())
            .is_some()
        {
            return Err(format!("{OVERRIDES_ENV} names {module:?} twice"));
        }
    }
    Ok(overrides)
}

#[must_use]
pub fn render_overrides(overrides: &Overrides) -> String {
    let mut text = String::new();
    for (module, version) in overrides {
        text.push_str(&format!(
            "single_version_override(\n    module_name = \"{module}\",\n    version = \"{version}\",\n)\n"
        ));
    }
    text
}

pub fn apply_overrides(module_bazel: &str, overrides: &Overrides) -> Result<String, String> {
    let mut text = module_bazel.to_owned();
    for (module, version) in overrides {
        let file = ModuleFile::parse(&text).map_err(|error| error.to_string())?;
        match file.set_bazel_dep_version(module, version) {
            Ok(edited) => text = edited,
            Err(ModuleError::NotFound { .. }) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    if !overrides.is_empty() {
        text = format!("{}\n{}", text.trim_end(), render_overrides(overrides));
    }
    Ok(text)
}

fn collect<'a>(node: &'a serde_json::Value, found: &mut Vec<&'a serde_json::Value>) {
    found.push(node);
    if let Some(children) = node
        .get("dependencies")
        .and_then(serde_json::Value::as_array)
    {
        for child in children {
            collect(child, found);
        }
    }
}

pub fn check_graph(graph: &str, table: &Table, overrides: &Overrides) -> Vec<String> {
    let root: serde_json::Value = match serde_json::from_str(graph) {
        Ok(root) => root,
        Err(error) => return vec![format!("mod graph output is not JSON: {error}")],
    };
    let mut nodes = Vec::new();
    collect(&root, &mut nodes);
    let mut problems = Vec::new();
    for (module, pins) in table {
        let expected = overrides.get(module).unwrap_or(&pins.floor);
        let mut resolved = BTreeSet::new();
        let mut requesters = BTreeSet::new();
        for node in nodes
            .iter()
            .filter(|node| node.get("name").and_then(serde_json::Value::as_str) == Some(module))
        {
            if let Some(version) = node.get("version").and_then(serde_json::Value::as_str) {
                resolved.insert(version.to_owned());
                if let Some(by) = node
                    .get("resolvedRequestedBy")
                    .and_then(serde_json::Value::as_array)
                {
                    requesters.extend(by.iter().filter_map(|v| v.as_str()).map(str::to_owned));
                }
            }
        }
        let Some(version) = resolved.iter().next() else {
            problems.push(format!("{module} is not in the module graph"));
            continue;
        };
        if resolved.len() > 1 {
            problems.push(format!(
                "{module} resolves to several versions {resolved:?}"
            ));
        } else if version != expected {
            let direction = match compare_module_versions(version, expected) {
                Ordering::Greater => "above",
                Ordering::Less | Ordering::Equal => "below",
            };
            let by = if requesters.is_empty() {
                "unknown modules".to_owned()
            } else {
                requesters.into_iter().collect::<Vec<_>>().join(", ")
            };
            problems.push(format!(
                "{module} resolves to {version}, {direction} the expected {expected}; requested at that version by {by}"
            ));
        }
    }
    problems
}

#[cfg(test)]
mod tests;
