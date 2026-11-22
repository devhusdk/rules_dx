#![deny(clippy::indexing_slicing, clippy::string_slice)]

//! The quality and docs aspect files and the dependency hubs of the `<lang>/lang.json` manifests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Deserialize;

/// The `quality` entry of one `<lang>/lang.json`.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct QualityFiles {
    /// The aspect file, like `go/private/quality:go.bzl`.
    pub bzl: String,
    pub file_extensions: Vec<String>,
    /// Exact file names, like `CMakeLists.txt`.
    pub file_names: Vec<String>,
    /// License ids whose acceptance in the consumer's `MODULE.bazel` asks for the aspect file, as
    /// the `license:<id>` marks of `wanted`.
    pub licenses: Vec<String>,
    /// Aspect names defined in `bzl`, by command.
    pub aspects: BTreeMap<String, Vec<String>>,
    /// Probe aspect names defined in `bzl`, by command.
    pub probes: BTreeMap<String, Vec<String>>,
}

impl QualityFiles {
    /// Whether a workspace with the extensions, file names and `license:<id>` marks of `marks`
    /// needs the aspect file.
    #[must_use]
    pub fn wanted(&self, marks: &BTreeSet<String>) -> bool {
        self.file_extensions
            .iter()
            .chain(&self.file_names)
            .any(|mark| marks.contains(mark))
            || self
                .licenses
                .iter()
                .any(|id| marks.contains(&license_mark(id)))
    }

    /// The `bzl%name` labels of the aspects (or probes) of `command`.
    #[must_use]
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

/// The mark of an accepted license id, like `license:android-sdk-license`.
#[must_use]
pub fn license_mark(id: &str) -> String {
    format!("license:{id}")
}

/// The `docs` entry of one `<lang>/lang.json`.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct DocsFiles {
    /// The aspect file, like `tools/docs/go:aspect.bzl`.
    pub bzl: String,
    pub file_extensions: Vec<String>,
    /// Docs aspect names defined in `bzl`.
    pub aspects: Vec<String>,
}

impl DocsFiles {
    /// The `bzl%name` labels of the docs aspects.
    #[must_use]
    pub fn labels(&self) -> Vec<String> {
        self.aspects
            .iter()
            .map(|name| format!("{}%{name}", self.bzl))
            .collect()
    }
}

/// One `setup.hubs` entry of a `<lang>/lang.json`: the stanza `dx init` writes for a manifest.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct Hub {
    /// The ecosystem id, which is also the extension name in `extension`.
    pub ecosystem: String,
    /// The manifest file name.
    pub manifest: String,
    /// The lock file name next to the manifest. `None` for a hub with no lock and no tag.
    pub lock: Option<String>,
    /// The extension file label, like `@rules_dx//rust:extensions.bzl`.
    pub extension: String,
    /// The tag that takes the lock label.
    pub tag: Option<String>,
    /// The default hub repository name.
    pub hub: String,
    /// The state file a resolver writes for itself.
    pub state: Option<String>,
    /// The label of the runnable target that writes the lock.
    pub resolver: Option<String>,
    /// How Gazelle maps an import: `name` or `provides`.
    pub imports: String,
}

/// One `setup.licenses` entry of a `<lang>/lang.json`.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct License {
    /// The id a `dx_licenses.accept` tag lists.
    pub id: String,
    /// The host platforms, like `linux_x86_64`, that need the id.
    pub platforms: Vec<String>,
}

/// The `setup.licenses` of one language, with the files that mark the language present.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct LanguageLicenses {
    /// The `lang.json` name.
    pub language: String,
    /// The `quality` file extensions and file names of the language.
    pub marks: Vec<String>,
    pub licenses: Vec<License>,
}

/// The files that mark one `lang.json` language with a `quality` entry present.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct LanguageMarks {
    /// The `lang.json` name.
    pub language: String,
    /// The `dx.toml` family id: `quality.family`, else the name.
    pub family: String,
    /// The `quality` file extensions and file names of the language.
    pub marks: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Setup {
    hubs: Vec<Hub>,
    licenses: Vec<License>,
    flags: Vec<String>,
}

const LANGUAGES: &str = include_str!("../languages.json");

#[derive(Deserialize)]
struct Languages {
    #[serde(default)]
    core: Core,
    languages: Vec<Manifest>,
}

/// The aspect files of the core families, from `core/quality/core_aspects.json`.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Core {
    quality: Vec<QualityFiles>,
    docs: Vec<DocsFiles>,
}

fn shipped_table() -> Option<Languages> {
    serde_json::from_str::<Languages>(LANGUAGES).ok()
}

fn shipped() -> Vec<Manifest> {
    shipped_table()
        .map(|table| table.languages)
        .unwrap_or_default()
}

fn shipped_core() -> &'static Core {
    static CORE: OnceLock<Core> = OnceLock::new();
    CORE.get_or_init(|| shipped_table().map(|table| table.core).unwrap_or_default())
}

/// The quality aspect files of the core families, which run in every workspace, in file order.
#[must_use]
pub fn core_quality() -> &'static [QualityFiles] {
    &shipped_core().quality
}

/// The docs aspect files of the core, which run in every workspace, in file order.
#[must_use]
pub fn core_docs() -> &'static [DocsFiles] {
    &shipped_core().docs
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    commands: BTreeMap<String, String>,
    quality: Option<ManifestQuality>,
    docs: Option<DocsFiles>,
    setup: Option<Setup>,
}

/// The `quality` entry of one manifest, with the family id that only the single language knows.
#[derive(Deserialize)]
struct ManifestQuality {
    /// The `dx.toml` family id when it differs from the language name, like `capnp`.
    #[serde(default)]
    family: Option<String>,
    #[serde(flatten)]
    files: QualityFiles,
}

/// The `rules_dx` target `dx generate` runs.
pub const GENERATE: &str = "@rules_dx//dx:generate";
/// The `rules_dx` target `dx generate --check` runs.
pub const GENERATE_CHECK: &str = "@rules_dx//dx:generate_check";
/// The `rules_dx` target `dx security verify` runs.
pub const COSIGN: &str = "@rules_dx//deploy/private/release:cosign";
/// The `rules_dx` target `dx audit` runs for secrets.
pub const GITLEAKS: &str = "@rules_dx//core/quality/artifacts:gitleaks";
/// The `rules_dx` target `dx audit` runs for images.
pub const TRIVY: &str = "@rules_dx//core/quality/artifacts:trivy";
/// The `rules_dx` target `dx upgrade` runs.
pub const DX: &str = "@rules_dx//:dx";
/// The `rules_dx` target `dx sync` builds for the environment plans.
pub const ENV: &str = "@rules_dx//dx:env";
/// The `rules_dx` target `dx sync` builds for the codegen plans.
pub const CODEGEN: &str = "@rules_dx//dx:codegen";
/// The `rules_dx` target `dx docs` builds for the site scope tools.
pub const DOCS_SCOPE_TOOLS: &str = "@rules_dx//tools/docs/site:scope_tools";

/// The `rules_dx` targets dx commands run for every language.
pub const COMMAND_TARGETS: &[&str] = &[
    GENERATE,
    GENERATE_CHECK,
    COSIGN,
    GITLEAKS,
    TRIVY,
    DX,
    ENV,
    CODEGEN,
    DOCS_SCOPE_TOOLS,
];

fn commands() -> &'static BTreeMap<String, BTreeMap<String, String>> {
    static ALL: OnceLock<BTreeMap<String, BTreeMap<String, String>>> = OnceLock::new();
    ALL.get_or_init(|| {
        shipped()
            .into_iter()
            .filter(|manifest| !manifest.commands.is_empty())
            .map(|manifest| (manifest.name, manifest.commands))
            .collect()
    })
}

/// The `commands` label `lang.json` of `lang` names for `tool`.
#[must_use]
pub fn command(lang: &str, tool: &str) -> Option<&'static str> {
    commands()
        .get(lang)
        .and_then(|table| table.get(tool))
        .map(String::as_str)
}

/// The `commands` labels of `lang`, sorted by tool name.
#[must_use]
pub fn language_commands(lang: &str) -> Vec<&'static str> {
    commands()
        .get(lang)
        .into_iter()
        .flat_map(|table| table.values().map(String::as_str))
        .collect()
}

/// `COMMAND_TARGETS` and the `commands` labels of every language.
pub fn command_labels() -> impl Iterator<Item = &'static str> {
    COMMAND_TARGETS.iter().copied().chain(
        commands()
            .values()
            .flat_map(|table| table.values().map(String::as_str)),
    )
}

/// `label` after `@rules_dx//`, like `dx:generate`.
#[must_use]
pub fn repo_relative(label: &str) -> &str {
    label.strip_prefix("@rules_dx//").unwrap_or(label)
}

/// The `setup.hubs` entries of every `lang.json`, sorted by ecosystem.
pub fn hubs() -> &'static [Hub] {
    static ALL: OnceLock<Vec<Hub>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut hubs: Vec<Hub> = shipped()
            .into_iter()
            .filter_map(|manifest| manifest.setup)
            .flat_map(|setup| setup.hubs)
            .collect();
        hubs.sort_by(|a, b| a.ecosystem.cmp(&b.ecosystem));
        hubs
    })
}

/// The languages whose `lang.json` has a `quality` entry, sorted by language.
pub fn quality_marks() -> &'static [LanguageMarks] {
    static ALL: OnceLock<Vec<LanguageMarks>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all: Vec<LanguageMarks> = shipped()
            .into_iter()
            .filter_map(|manifest| {
                let quality = manifest.quality?;
                Some(LanguageMarks {
                    family: quality.family.unwrap_or_else(|| manifest.name.clone()),
                    language: manifest.name,
                    marks: [quality.files.file_extensions, quality.files.file_names].concat(),
                })
            })
            .collect();
        all.sort_by(|a, b| a.language.cmp(&b.language));
        all
    })
}

/// The languages whose `lang.json` lists `setup.licenses`, sorted by language.
pub fn licenses() -> &'static [LanguageLicenses] {
    static ALL: OnceLock<Vec<LanguageLicenses>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all: Vec<LanguageLicenses> = shipped()
            .into_iter()
            .filter_map(|manifest| {
                let licenses = manifest.setup?.licenses;
                if licenses.is_empty() {
                    return None;
                }
                let marks = manifest
                    .quality
                    .map(|quality| {
                        [quality.files.file_extensions, quality.files.file_names].concat()
                    })
                    .unwrap_or_default();
                Some(LanguageLicenses {
                    language: manifest.name,
                    marks,
                    licenses,
                })
            })
            .collect();
        all.sort_by(|a, b| a.language.cmp(&b.language));
        all
    })
}

/// The `setup.flags` of each language in `names` that has any, as (language, flags), sorted by
/// language.
pub fn setup_flags<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<(String, Vec<String>)> {
    let names: BTreeSet<&str> = names.into_iter().collect();
    let mut found: Vec<(String, Vec<String>)> = shipped()
        .into_iter()
        .filter(|manifest| names.contains(manifest.name.as_str()))
        .filter_map(|manifest| Some((manifest.name, manifest.setup?.flags)))
        .filter(|(_, flags)| !flags.is_empty())
        .collect();
    found.sort();
    found
}

/// The `setup.hubs[].resolver` labels of `language`'s `lang.json`, sorted.
#[must_use]
pub fn resolvers(language: &str) -> Vec<String> {
    let mut labels: Vec<String> = shipped()
        .into_iter()
        .filter(|manifest| manifest.name == language)
        .filter_map(|manifest| manifest.setup)
        .flat_map(|setup| setup.hubs)
        .filter_map(|hub| hub.resolver)
        .collect();
    labels.sort();
    labels
}

/// The hub entry of `ecosystem`, if a `lang.json` has one.
#[must_use]
pub fn hub(ecosystem: &str) -> Option<&'static Hub> {
    hubs().iter().find(|hub| hub.ecosystem == ecosystem)
}

/// Merges the `quality` entries of `lang.json` texts by aspect file, sorted by file. A text
/// without a `quality` entry is skipped.
///
/// # Errors
///
/// Returns `Err` when a text is not valid `lang.json`.
pub fn parse_quality(texts: &[&str]) -> Result<Vec<QualityFiles>, serde_json::Error> {
    Ok(merge_quality(parse_manifests(texts)?))
}

fn parse_manifests(texts: &[&str]) -> Result<Vec<Manifest>, serde_json::Error> {
    texts
        .iter()
        .map(|text| serde_json::from_str(text))
        .collect()
}

fn merge_quality(manifests: Vec<Manifest>) -> Vec<QualityFiles> {
    let mut merged: BTreeMap<String, QualityFiles> = Default::default();
    for manifest in manifests {
        if let Some(ManifestQuality { files: quality, .. }) = manifest.quality {
            let entry = merged.entry(quality.bzl.clone()).or_default();
            entry.bzl = quality.bzl;
            entry.file_extensions.extend(quality.file_extensions);
            entry.file_names.extend(quality.file_names);
            entry.licenses.extend(quality.licenses);
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
    merged
        .into_values()
        .map(|mut files| {
            let extensions: BTreeSet<String> = files.file_extensions.drain(..).collect();
            files.file_extensions = extensions.into_iter().collect();
            let names: BTreeSet<String> = files.file_names.drain(..).collect();
            files.file_names = names.into_iter().collect();
            let licenses: BTreeSet<String> = files.licenses.drain(..).collect();
            files.licenses = licenses.into_iter().collect();
            files
        })
        .collect()
}

/// Merges the `docs` entries of `lang.json` texts by aspect file, sorted by file. A text
/// without a `docs` entry is skipped.
///
/// # Errors
///
/// Returns `Err` when a text is not valid `lang.json`.
pub fn parse_docs(texts: &[&str]) -> Result<Vec<DocsFiles>, serde_json::Error> {
    Ok(merge_docs(parse_manifests(texts)?))
}

fn merge_docs(manifests: Vec<Manifest>) -> Vec<DocsFiles> {
    let mut merged: BTreeMap<String, DocsFiles> = Default::default();
    for manifest in manifests {
        if let Some(docs) = manifest.docs {
            let entry = merged.entry(docs.bzl.clone()).or_default();
            entry.bzl = docs.bzl;
            entry.file_extensions.extend(docs.file_extensions);
            entry.aspects.extend(docs.aspects);
        }
    }
    merged
        .into_values()
        .map(|mut files| {
            let extensions: BTreeSet<String> = files.file_extensions.drain(..).collect();
            files.file_extensions = extensions.into_iter().collect();
            let aspects: BTreeSet<String> = files.aspects.drain(..).collect();
            files.aspects = aspects.into_iter().collect();
            files
        })
        .collect()
}

/// The docs aspect files of the languages, with the file extensions that make a workspace need
/// them.
pub fn docs_languages() -> &'static [DocsFiles] {
    static ALL: OnceLock<Vec<DocsFiles>> = OnceLock::new();
    ALL.get_or_init(|| merge_docs(shipped()))
}

/// The aspect files of every `lang.json` `quality` entry, with what makes a workspace need them.
pub fn quality_languages() -> &'static [QualityFiles] {
    static ALL: OnceLock<Vec<QualityFiles>> = OnceLock::new();
    ALL.get_or_init(|| merge_quality(shipped()))
}

#[cfg(test)]
mod tests;
