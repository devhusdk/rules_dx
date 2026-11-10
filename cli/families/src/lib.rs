//! The quality and docs aspect files of the `<lang>/lang.json` manifests.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::Deserialize;

/// The `quality` entry of one `<lang>/lang.json`.
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(default)]
pub struct QualityFiles {
    /// The aspect file, like `go/private/quality:go.bzl`.
    pub bzl: String,
    /// Whether every workspace needs the file, whatever files it has.
    pub every_workspace: bool,
    pub file_extensions: Vec<String>,
    /// Aspect names defined in `bzl`, by command.
    pub aspects: std::collections::BTreeMap<String, Vec<String>>,
    /// Probe aspect names defined in `bzl`, by command.
    pub probes: std::collections::BTreeMap<String, Vec<String>>,
}

impl QualityFiles {
    /// Whether a workspace with the files of `extensions` needs the aspect file.
    pub fn wanted(&self, extensions: &BTreeSet<String>) -> bool {
        self.every_workspace
            || self
                .file_extensions
                .iter()
                .any(|extension| extensions.contains(extension))
    }

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
    pub fn labels(&self) -> Vec<String> {
        self.aspects
            .iter()
            .map(|name| format!("{}%{name}", self.bzl))
            .collect()
    }
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
    quality: Option<QualityFiles>,
    docs: Option<DocsFiles>,
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
            entry.every_workspace |= quality.every_workspace;
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
    merged
        .into_values()
        .map(|mut files| {
            let extensions: BTreeSet<String> = files.file_extensions.drain(..).collect();
            files.file_extensions = extensions.into_iter().collect();
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
