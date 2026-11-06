//! `dx release`: the next version from conventional commits and the manifests that carry it.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use semver::Version;

use crate::edit::own_version::{self, Manifest};

pub mod conventional;

use conventional::Bump;

pub const TAG_PREFIX: &str = "v";

/// The `git log` arguments every release read uses: merge commits are never read.
pub const LOG_ARGS: [&str; 3] = ["log", "--no-merges", "--format=%H%x00%s%x00%b%x1e"];

/// The commits in `log` whose subject is not a conventional commit, as `(hash, subject)`.
pub fn lint(log: &str) -> Vec<(String, String)> {
    conventional::parse(log)
        .into_iter()
        .filter(|commit| commit.kind == conventional::Kind::Other)
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
    /// Deploy targets whose api-compat check found breaking changes.
    pub api_breaking: Vec<String>,
}

impl Report {
    /// The next major version after the current one, which the api-compat step compares against the last tag.
    pub fn major_candidate(&self) -> Option<String> {
        let current = Version::parse(self.current.as_deref()?).ok()?;
        bumped(&current, Bump::Major).map(|version| version.to_string())
    }

    /// Raises the bump to major because these deploy targets report breaking API changes.
    pub fn raise_for_api(&mut self, targets: Vec<String>) {
        if targets.is_empty() || self.bump == Bump::Major {
            self.api_breaking = targets;
            return;
        }
        self.api_breaking = targets;
        if let Some(current) = self.current.as_deref().and_then(|v| Version::parse(v).ok()) {
            self.bump = Bump::Major;
            self.next = bumped(&current, Bump::Major).map(|v| v.to_string());
        }
    }

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
        for target in &self.api_breaking {
            out.push_str(&format!(
                "api compat: {target} reports breaking changes; the bump is major\n"
            ));
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

/// The deploy target a `_crates_config` label belongs to.
pub fn deploy_target(config_label: &str) -> &str {
    config_label.strip_suffix("_config").unwrap_or(config_label)
}

/// True when a deploy tool's output says cargo-semver-checks found breaking changes.
pub fn reports_breaking(output: &str) -> bool {
    output.lines().any(|line| {
        line.trim()
            .strip_prefix("api compat: ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|count| count.parse::<usize>().ok())
            .is_some_and(|count| count > 0)
    })
}

/// Runs `check` for each config label and returns the deploy targets that report breaking changes.
pub fn breaking_targets(
    config_labels: &str,
    check: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    let mut breaking = Vec::new();
    for label in config_labels
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        let target = deploy_target(label);
        if reports_breaking(&check(target)?) {
            breaking.push(target.to_owned());
        }
    }
    Ok(breaking)
}

fn parse_tag(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(TAG_PREFIX).unwrap_or(tag)).ok()
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
    last_tag: Option<&str>,
    log: &str,
    forced: Option<&str>,
) -> Result<Report, String> {
    let manifests = own_version::find(workspace)
        .map_err(|error| format!("cannot scan {}: {error}", workspace.display()))?;
    let commits = conventional::parse(log);
    let bump = conventional::bump(&commits);
    let current = last_tag
        .and_then(parse_tag)
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
        api_breaking: Vec::new(),
    })
}

fn notes(log: &str) -> String {
    use conventional::Kind;
    let commits = conventional::parse(log);
    let groups = [
        (Kind::Feat, "Features"),
        (Kind::Fix, "Fixes"),
        (Kind::Perf, "Performance"),
    ];
    conventional::notes(&commits, &groups)
}

/// The annotated tag message: the title line, then the notes for the commits in `log`.
pub fn tag_message(tag: &str, log: &str) -> String {
    format!("{tag}\n\n{}", notes(log))
}

/// The `## [version] - date` section for the commits in `log`, with the headings lowered one level.
pub fn changelog_section(version: &str, date: &str, log: &str) -> String {
    let body: String = notes(log)
        .lines()
        .map(|line| match line.strip_prefix("## ") {
            Some(heading) => format!("### {heading}\n"),
            None => format!("{line}\n"),
        })
        .collect();
    format!("## [{version}] - {date}\n\n{body}")
}

/// `existing` with `section` placed above the older release sections.
pub fn prepend_section(existing: &str, section: &str) -> String {
    match existing.find("\n## ") {
        Some(at) => {
            let (head, tail) = existing.split_at(at + 1);
            format!("{head}{section}\n{tail}")
        }
        None => format!("{}\n\n{section}", existing.trim_end_matches('\n')),
    }
}

/// Adds the section for `version` to `CHANGELOG.md` when the workspace has one; true when it was written.
pub fn write_changelog(
    workspace: &Path,
    version: &str,
    date: &str,
    log: &str,
) -> Result<bool, String> {
    let path = workspace.join("CHANGELOG.md");
    let Ok(existing) = fs::read_to_string(&path) else {
        return Ok(false);
    };
    let section = changelog_section(version, date, log);
    fs::write(&path, prepend_section(&existing, &section))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(true)
}

/// Today's UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date_of_day((secs / 86_400) as i64)
}

fn date_of_day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
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
