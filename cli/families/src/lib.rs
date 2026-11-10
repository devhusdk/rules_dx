//! The language families of `core/modules/families.json`.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::Deserialize;

const TABLE: &str = include_str!("../../../core/modules/families.json");

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

/// The `quality` entry of one `<lang>/lang.json`.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct QualityFiles {
    /// The aspect file, like `quality:families/go.bzl`.
    pub bzl: String,
    pub file_extensions: Vec<String>,
    /// Aspect names defined in `bzl`, by command.
    pub aspects: std::collections::BTreeMap<String, Vec<String>>,
    /// Probe aspect names defined in `bzl`, by command.
    pub probes: std::collections::BTreeMap<String, Vec<String>>,
}

impl QualityFiles {
    /// The `bzl%name` labels of the aspects (or probes) of `command`.
    pub fn labels(&self, command: &str, probes: bool) -> Vec<String> {
        let table = if probes { &self.probes } else { &self.aspects };
        table
            .get(command)
            .into_iter()
            .flatten()
            .map(|name| format!("{}%{name}", self.bzl))
            .collect()
    }
}

const LANGUAGE_MANIFESTS: &[&str] = &[
    include_str!("../../../api_spec/lang.json"),
    include_str!("../../../astro/lang.json"),
    include_str!("../../../avro/lang.json"),
    include_str!("../../../capnproto/lang.json"),
    include_str!("../../../cc/lang.json"),
    include_str!("../../../d/lang.json"),
    include_str!("../../../dart/lang.json"),
    include_str!("../../../dotnet/lang.json"),
    include_str!("../../../elixir/lang.json"),
    include_str!("../../../erlang/lang.json"),
    include_str!("../../../flatbuffers/lang.json"),
    include_str!("../../../fortran/lang.json"),
    include_str!("../../../go/lang.json"),
    include_str!("../../../haskell/lang.json"),
    include_str!("../../../java/lang.json"),
    include_str!("../../../javascript/lang.json"),
    include_str!("../../../julia/lang.json"),
    include_str!("../../../kotlin/lang.json"),
    include_str!("../../../lua/lang.json"),
    include_str!("../../../mdx/lang.json"),
    include_str!("../../../msix/lang.json"),
    include_str!("../../../ocaml/lang.json"),
    include_str!("../../../perl/lang.json"),
    include_str!("../../../php/lang.json"),
    include_str!("../../../pkl/lang.json"),
    include_str!("../../../powershell/lang.json"),
    include_str!("../../../python/lang.json"),
    include_str!("../../../qt/lang.json"),
    include_str!("../../../ruby/lang.json"),
    include_str!("../../../rust/lang.json"),
    include_str!("../../../scala/lang.json"),
    include_str!("../../../svelte/lang.json"),
    include_str!("../../../swift/lang.json"),
    include_str!("../../../thrift/lang.json"),
    include_str!("../../../typescript/lang.json"),
    include_str!("../../../vue/lang.json"),
    include_str!("../../../zig/lang.json"),
];

#[derive(Deserialize)]
struct Manifest {
    quality: Option<QualityFiles>,
}

/// Merges the `quality` entries of `lang.json` texts by aspect file, sorted by file. A text
/// without a `quality` entry is skipped.
pub fn parse_quality(texts: &[&str]) -> Result<Vec<QualityFiles>, serde_json::Error> {
    let mut merged: std::collections::BTreeMap<String, QualityFiles> = Default::default();
    for text in texts {
        let manifest: Manifest = serde_json::from_str(text)?;
        if let Some(quality) = manifest.quality {
            let entry = merged.entry(quality.bzl.clone()).or_default();
            entry.bzl = quality.bzl;
            entry.file_extensions.extend(quality.file_extensions);
            for (target, source) in [
                (&mut entry.aspects, quality.aspects),
                (&mut entry.probes, quality.probes),
            ] {
                for (command, names) in source {
                    target.entry(command).or_default().extend(names);
                }
            }
        }
    }
    Ok(merged
        .into_values()
        .map(|mut files| {
            let extensions: BTreeSet<String> = files.file_extensions.drain(..).collect();
            files.file_extensions = extensions.into_iter().collect();
            files
        })
        .collect())
}

/// The aspect files that load a language rule set, with the file extensions that make a
/// workspace need them. Every other aspect file is needed by every workspace.
pub fn quality_languages() -> &'static [QualityFiles] {
    static ALL: OnceLock<Vec<QualityFiles>> = OnceLock::new();
    ALL.get_or_init(|| parse_quality(LANGUAGE_MANIFESTS).unwrap_or_default())
}

#[cfg(test)]
mod tests;
