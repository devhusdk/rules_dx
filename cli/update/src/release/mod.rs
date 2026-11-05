//! `dx release --check`: the next version from conventional commits and the manifests that carry it.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use semver::Version;

use crate::edit::own_version::{self, Manifest};

pub mod conventional;

use conventional::Bump;

const DEFAULT_TAG_PREFIX: &str = "v";

/// What `release.toml` sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub tag_prefix: String,
    /// Subject prefixes `--commits` lets through, such as `Merge `.
    pub skip: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            tag_prefix: DEFAULT_TAG_PREFIX.to_owned(),
            skip: Vec::new(),
        }
    }
}

/// Reads `release.toml` at the workspace root; a missing file is the defaults.
pub fn read_config(workspace: &Path) -> Result<Config, String> {
    let path = workspace.join("release.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    parse_config(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn parse_config(text: &str) -> Result<Config, String> {
    let doc: toml_edit::DocumentMut = text.parse().map_err(|error| format!("{error}"))?;
    let mut config = Config::default();
    if let Some(item) = doc.get("tag_prefix") {
        config.tag_prefix = item
            .as_str()
            .ok_or("tag_prefix must be a string")?
            .to_owned();
    }
    if let Some(item) = doc.get("skip") {
        let table = item.as_table_like().ok_or("skip must be a table")?;
        if let Some(item) = table.get("subjects") {
            config.skip = item
                .as_array()
                .and_then(|array| {
                    array
                        .iter()
                        .map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .ok_or("skip.subjects must be an array of strings")?;
        }
    }
    Ok(config)
}

/// The commits in `log` whose subject is not a conventional commit and does not start with a `skip` entry, as `(hash, subject)`.
pub fn lint(config: &Config, log: &str) -> Vec<(String, String)> {
    conventional::parse(log)
        .into_iter()
        .filter(|commit| commit.kind == conventional::Kind::Other)
        .filter(|commit| {
            !config
                .skip
                .iter()
                .any(|s| commit.subject.starts_with(s.as_str()))
        })
        .map(|commit| (commit.hash, commit.subject))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestLine {
    pub path: String,
    pub version: String,
    pub in_sync: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    pub last_tag: Option<String>,
    pub commits: usize,
    pub current: Option<String>,
    pub bump: Bump,
    pub next: Option<String>,
    pub manifests: Vec<ManifestLine>,
}

impl Report {
    pub fn out_of_sync(&self) -> usize {
        self.manifests.iter().filter(|line| !line.in_sync).count()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        match &self.last_tag {
            Some(tag) => out.push_str(&format!(
                "last tag: {tag} ({} commits since)\n",
                self.commits
            )),
            None => out.push_str(&format!("last tag: none ({} commits)\n", self.commits)),
        }
        match &self.current {
            Some(current) => out.push_str(&format!("current version: {current}\n")),
            None => out.push_str("current version: unknown\n"),
        }
        match (&self.next, self.bump) {
            (Some(next), Bump::None) => {
                out.push_str(&format!("next version: {next} (from --to)\n"))
            }
            (Some(next), bump) => {
                out.push_str(&format!("next version: {next} ({})\n", bump.name()))
            }
            (None, _) => out.push_str("next version: none (no feat, fix or breaking commits)\n"),
        }
        if self.manifests.is_empty() {
            out.push_str("manifests: none carry a version\n");
        } else {
            out.push_str("manifests:\n");
            for line in &self.manifests {
                let state = if line.in_sync { "ok" } else { "out of sync" };
                out.push_str(&format!("  {} {} ({state})\n", line.path, line.version));
            }
        }
        out
    }
}

fn parse_tag(tag: &str, prefix: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(prefix).unwrap_or(tag)).ok()
}

fn most_common(manifests: &[Manifest]) -> Option<Version> {
    let mut counts: HashMap<Version, usize> = HashMap::new();
    for manifest in manifests {
        if let Ok(version) = Version::parse(&manifest.version) {
            *counts.entry(version).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(version, _)| version)
}

fn bumped(current: &Version, bump: Bump) -> Option<Version> {
    let mut next = Version::new(current.major, current.minor, current.patch);
    match bump {
        Bump::None => return None,
        Bump::Patch => next.patch += 1,
        Bump::Minor => {
            next.minor += 1;
            next.patch = 0;
        }
        Bump::Major => {
            next.major += 1;
            next.minor = 0;
            next.patch = 0;
        }
    }
    Some(next)
}

/// Builds the report from the last tag, the `git log` since it and the manifests found.
pub fn plan(
    workspace: &Path,
    config: &Config,
    last_tag: Option<&str>,
    log: &str,
    forced: Option<&str>,
) -> Result<Report, String> {
    let manifests = own_version::find(workspace)
        .map_err(|error| format!("cannot scan {}: {error}", workspace.display()))?;
    let commits = conventional::parse(log);
    let bump = conventional::bump(&commits);
    let current = last_tag
        .and_then(|tag| parse_tag(tag, &config.tag_prefix))
        .or_else(|| most_common(&manifests));
    let next = match forced {
        Some(raw) => Some(Version::parse(raw).map_err(|error| format!("--to {raw}: {error}"))?),
        None => current.as_ref().and_then(|current| bumped(current, bump)),
    };
    let target = next.as_ref().or(current.as_ref());
    let lines = manifests
        .iter()
        .map(|manifest| ManifestLine {
            path: manifest
                .path
                .strip_prefix(workspace)
                .unwrap_or(&manifest.path)
                .to_string_lossy()
                .replace('\\', "/"),
            version: manifest.version.clone(),
            in_sync: current
                .as_ref()
                .is_some_and(|current| manifest.version == current.to_string())
                || target.is_some_and(|target| manifest.version == target.to_string()),
        })
        .collect();
    Ok(Report {
        last_tag: last_tag.map(str::to_owned),
        commits: commits.len(),
        current: current.map(|version| version.to_string()),
        bump: if forced.is_some() { Bump::None } else { bump },
        next: next.map(|version| version.to_string()),
        manifests: lines,
    })
}

/// The annotated tag message: the title line, then the notes for the commits in `log`.
pub fn tag_message(tag: &str, log: &str) -> String {
    use conventional::Kind;
    let commits = conventional::parse(log);
    let groups = [
        (Kind::Feat, "Features"),
        (Kind::Fix, "Fixes"),
        (Kind::Perf, "Performance"),
    ];
    format!("{tag}\n\n{}", conventional::notes(&commits, &groups))
}

/// Writes `version` into every manifest that carries one; returns their paths relative to `workspace`.
pub fn apply(workspace: &Path, version: &str) -> Result<Vec<String>, String> {
    let manifests = own_version::find(workspace)
        .map_err(|error| format!("cannot scan {}: {error}", workspace.display()))?;
    let mut changed = Vec::new();
    for manifest in &manifests {
        own_version::set(manifest, version)
            .map_err(|error| format!("{}: {error}", manifest.path.display()))?;
        changed.push(
            manifest
                .path
                .strip_prefix(workspace)
                .unwrap_or(&manifest.path)
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    Ok(changed)
}

#[cfg(test)]
mod tests;
