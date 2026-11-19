#![deny(clippy::indexing_slicing, clippy::string_slice)]

//! The quality and docs aspect files and the dependency hubs of the `<lang>/lang.json` manifests.

use std::collections::BTreeSet;
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
    /// Aspect names defined in `bzl`, by command.
    pub aspects: std::collections::BTreeMap<String, Vec<String>>,
    /// Probe aspect names defined in `bzl`, by command.
    pub probes: std::collections::BTreeMap<String, Vec<String>>,
}

impl QualityFiles {
    /// Whether a workspace with the extensions and file names of `extensions` needs the aspect
    /// file.
    #[must_use]
    pub fn wanted(&self, extensions: &BTreeSet<String>) -> bool {
        self.file_extensions
            .iter()
            .chain(&self.file_names)
            .any(|mark| extensions.contains(mark))
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
    languages: Vec<Manifest>,
}

fn shipped() -> Vec<Manifest> {
    serde_json::from_str::<Languages>(LANGUAGES)
        .map(|table| table.languages)
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    name: String,
    quality: Option<QualityFiles>,
    docs: Option<DocsFiles>,
    setup: Option<Setup>,
    #[serde(default)]
    commands: Vec<String>,
}

/// Declares each language-independent `rules_dx` target a dx command runs twice: as `NAME`, the
/// target after the repository, and as `NAME_LABEL`, the `@rules_dx` label. `COMMAND_TARGETS`
/// lists them all.
macro_rules! command_targets {
    ($($(#[$doc:meta])* $name:ident, $label:ident = $target:literal;)*) => {
        $(
            $(#[$doc])*
            pub const $name: &str = $target;
            $(#[$doc])*
            pub const $label: &str = concat!("@rules_dx//", $target);
        )*
        /// The language-independent `rules_dx` targets dx commands run, each after the repository.
        pub const COMMAND_TARGETS: &[&str] = &[$($name),*];
    };
}

command_targets! {
    /// The target `dx generate` runs.
    GENERATE, GENERATE_LABEL = "dx:generate";
    /// The target `dx generate --check` runs.
    GENERATE_CHECK, GENERATE_CHECK_LABEL = "dx:generate_check";
    /// The cosign `dx security verify` runs.
    COSIGN, COSIGN_LABEL = "deploy/private/release:cosign";
    /// The gitleaks `dx audit` runs.
    GITLEAKS, GITLEAKS_LABEL = "core/quality/artifacts:gitleaks";
    /// The trivy `dx security` runs.
    TRIVY, TRIVY_LABEL = "core/quality/artifacts:trivy";
    /// The dx `dx upgrade` runs.
    DX, DX_LABEL = ":dx";
}

/// The `commands` of the `lang.json` of `language`: the `rules_dx` targets dx commands run for
/// it, each after the repository.
#[must_use]
pub fn language_commands(language: &str) -> &'static [String] {
    static ALL: OnceLock<Vec<(String, Vec<String>)>> = OnceLock::new();
    ALL.get_or_init(|| {
        shipped()
            .into_iter()
            .filter(|manifest| !manifest.commands.is_empty())
            .map(|manifest| (manifest.name, manifest.commands))
            .collect()
    })
    .iter()
    .find(|(name, _)| name == language)
    .map_or(&[], |(_, commands)| commands.as_slice())
}

/// The `@rules_dx` label of the `commands` target of `language` named `name`, if its `lang.json`
/// lists one.
#[must_use]
pub fn command_label(language: &str, name: &str) -> Option<String> {
    language_commands(language)
        .iter()
        .find(|target| {
            target
                .rsplit_once(':')
                .is_some_and(|(_, last)| last == name)
        })
        .map(|target| format!("@rules_dx//{target}"))
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
                    language: manifest.name,
                    marks: [quality.file_extensions, quality.file_names].concat(),
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
                    .map(|quality| [quality.file_extensions, quality.file_names].concat())
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

/// The hub entry of `ecosystem`, if a `lang.json` has one.
#[must_use]
pub fn hub(ecosystem: &str) -> Option<&'static Hub> {
    hubs().iter().find(|hub| hub.ecosystem == ecosystem)
}

/// Merges the `quality` entries of `lang.json` texts by aspect file, sorted by file. A text
/// without a `quality` entry is skipped.
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
    let mut merged: std::collections::BTreeMap<String, QualityFiles> = Default::default();
    for manifest in manifests {
        if let Some(quality) = manifest.quality {
            let entry = merged.entry(quality.bzl.clone()).or_default();
            entry.bzl = quality.bzl;
            entry.file_extensions.extend(quality.file_extensions);
            entry.file_names.extend(quality.file_names);
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
            files
        })
        .collect()
}

/// Merges the `docs` entries of `lang.json` texts by aspect file, sorted by file. A text
/// without a `docs` entry is skipped.
pub fn parse_docs(texts: &[&str]) -> Result<Vec<DocsFiles>, serde_json::Error> {
    Ok(merge_docs(parse_manifests(texts)?))
}

fn merge_docs(manifests: Vec<Manifest>) -> Vec<DocsFiles> {
    let mut merged: std::collections::BTreeMap<String, DocsFiles> = Default::default();
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
