//! The language families of `modules/families.json`.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::Deserialize;

const TABLE: &str = include_str!("../../../modules/families.json");

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct Family {
    #[serde(skip)]
    pub name: String,
    /// The `rules_dx_<family>` module that carries the family, once it is published.
    pub module: Option<String>,
    pub bazel_deps: Vec<String>,
    pub file_extensions: Vec<String>,
    pub aspects: Vec<String>,
}

#[derive(Deserialize)]
struct Table {
    families: std::collections::BTreeMap<String, Family>,
}

/// Parses a `families.json` text, sorted by family name.
pub fn parse(text: &str) -> Result<Vec<Family>, serde_json::Error> {
    let table: Table = serde_json::from_str(text)?;
    Ok(table
        .families
        .into_iter()
        .map(|(name, family)| Family { name, ..family })
        .collect())
}

/// The families of the shipped table.
pub fn families() -> &'static [Family] {
    static ALL: OnceLock<Vec<Family>> = OnceLock::new();
    ALL.get_or_init(|| parse(TABLE).unwrap_or_default())
}

/// The family that owns `ext` (with its leading dot, like `.py`), ignoring ASCII case.
pub fn family_for_extension(ext: &str) -> Option<&'static Family> {
    families().iter().find(|family| {
        family
            .file_extensions
            .iter()
            .any(|own| own.eq_ignore_ascii_case(ext))
    })
}

/// The families a workspace with these `bazel_dep` names has. A family without a `module`
/// always counts.
pub fn installed(dep_names: &BTreeSet<String>) -> Vec<&'static Family> {
    families()
        .iter()
        .filter(|family| {
            family
                .module
                .as_ref()
                .is_none_or(|module| dep_names.contains(module))
        })
        .collect()
}

#[cfg(test)]
mod tests;
