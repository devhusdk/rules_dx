//! Lists PyPI dependencies whose latest release is outside the declared specifier.

use super::{wanted, Candidate, Fetch, Findings, LookupError};
use crate::edit::request::pyproject::normalize;
use pep440_rs::{Version, VersionSpecifiers};
use std::collections::BTreeMap;
use std::str::FromStr;
use toml_edit::{DocumentMut, Item};

pub const DEFAULT_BASE: &str = "https://pypi.org/pypi";

/// Checks each registry requirement of the pyproject.toml files against PyPI at `base`.
pub fn outdated(
    set: &str,
    manifests: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut specs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in manifests {
        let Ok(doc) = text.parse::<DocumentMut>() else {
            continue;
        };
        let mut entries = Vec::new();
        let project = doc.get("project").and_then(Item::as_table_like);
        collect(project.and_then(|p| p.get("dependencies")), &mut entries);
        for table in [
            project.and_then(|p| p.get("optional-dependencies")),
            doc.get("dependency-groups"),
        ] {
            if let Some(table) = table.and_then(Item::as_table_like) {
                for (_, list) in table.iter() {
                    collect(Some(list), &mut entries);
                }
            }
        }
        for entry in entries {
            if let Some((name, spec)) = split(&entry) {
                let list = specs.entry(name).or_default();
                if !list.contains(&spec) {
                    list.push(spec);
                }
            }
        }
    }
    for (name, list) in specs {
        if !wanted(only, &name) {
            continue;
        }
        let error = |reason: String| LookupError {
            set: set.to_owned(),
            package: name.clone(),
            reason,
        };
        let (release, repo) = match fetch.get(&format!("{base}/{name}/json")).and_then(|body| {
            let release = newest(&body, prereleases)?;
            Ok((release, repo(&body)))
        }) {
            Ok(found) => found,
            Err(reason) => {
                found.errors.push(error(reason));
                continue;
            }
        };
        for current in list {
            let Ok(specifiers) = VersionSpecifiers::from_str(&current) else {
                found
                    .errors
                    .push(error(format!("invalid specifier {current}")));
                continue;
            };
            if !specifiers.contains(&release.0) {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: release.0.to_string(),
                    published: release.1.clone(),
                    prerelease: release.0.any_prerelease(),
                    repo: repo.clone(),
                    tags: None,
                });
            }
        }
    }
    found
}

/// The newest stable release of `name` on PyPI at `base`.
pub fn latest_stable(name: &str, base: &str, fetch: &dyn Fetch) -> Result<String, String> {
    let body = fetch.get(&format!("{base}/{name}/json"))?;
    newest(&body, false).map(|(version, _)| version.to_string())
}

fn collect(item: Option<&Item>, into: &mut Vec<String>) {
    if let Some(list) = item.and_then(Item::as_array) {
        into.extend(list.iter().filter_map(|v| v.as_str().map(str::to_owned)));
    }
}

fn split(requirement: &str) -> Option<(String, String)> {
    let requirement = requirement.split(';').next()?.trim();
    let name_end = requirement
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(requirement.len());
    let (name, mut rest) = requirement.split_at(name_end);
    rest = rest.trim_start();
    if rest.starts_with('[') {
        rest = rest[rest.find(']')? + 1..].trim_start();
    }
    if rest.starts_with('@') {
        return None;
    }
    let spec = rest.trim_start_matches('(').trim_end_matches(')').trim();
    if name.is_empty() || spec.is_empty() {
        return None;
    }
    Some((normalize(name), spec.to_owned()))
}

fn repo(body: &str) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(body).ok()?;
    let urls = doc["info"]["project_urls"].as_object()?;
    let mut keys: Vec<_> = urls.keys().collect();
    keys.sort_by_key(|key| {
        let key = key.to_lowercase();
        ["source", "repository", "changelog"]
            .iter()
            .position(|word| key.contains(word))
            .unwrap_or(usize::MAX)
    });
    keys.into_iter()
        .filter_map(|key| urls[key].as_str())
        .find_map(super::github_repo)
}

/// The release versions whose every file is yanked (PEP 592).
pub fn yanked_releases(body: &str) -> Result<Vec<String>, String> {
    let doc: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let releases = doc["releases"]
        .as_object()
        .ok_or_else(|| "no releases".to_owned())?;
    Ok(releases
        .iter()
        .filter(|(_, files)| {
            let files = files.as_array().map(Vec::as_slice).unwrap_or_default();
            !files.is_empty() && files.iter().all(is_yanked)
        })
        .map(|(version, _)| version.clone())
        .collect())
}

fn is_yanked(file: &serde_json::Value) -> bool {
    file["yanked"].as_bool().unwrap_or(false)
}

fn newest(body: &str, prereleases: bool) -> Result<(Version, Option<String>), String> {
    let doc: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let releases = doc["releases"]
        .as_object()
        .ok_or_else(|| "no releases".to_owned())?;
    let yanked = yanked_releases(body)?;
    let mut best: Option<(Version, Option<String>)> = None;
    for (key, files) in releases {
        if yanked.contains(key) {
            continue;
        }
        let Ok(version) = Version::from_str(key) else {
            continue;
        };
        if !prereleases && version.any_prerelease() {
            continue;
        }
        let live: Vec<_> = files
            .as_array()
            .into_iter()
            .flatten()
            .filter(|file| !is_yanked(file))
            .collect();
        if live.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(known, _)| version > *known) {
            let published = live
                .iter()
                .filter_map(|file| file["upload_time_iso_8601"].as_str())
                .min()
                .map(str::to_owned);
            best = Some((version, published));
        }
    }
    best.ok_or_else(|| "no stable version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_yanked_when_every_file_is() {
        let body = r#"{"releases":{"1.0":[{"yanked":true},{"yanked":true}],"1.1":[{"yanked":true},{"yanked":false}],"1.2":[],"1.3":[{"yanked":false}]}}"#;
        assert_eq!(yanked_releases(body).unwrap(), vec!["1.0"]);
        assert!(yanked_releases("not json").is_err());
        assert!(yanked_releases("{}").is_err());
    }

    const MANIFEST: &str = r#"
[project]
dependencies = [
  "fresh>=1.0",
  "Old_Pkg[extra] (>=1,<2) ; python_version >= '3.9'",
  "direct @ https://example.com/d.whl",
  "bare",
  "yanked>=1,<2",
  "pre>=1,<2",
  "broken>=1",
]
[project.optional-dependencies]
dev = ["tool==1.0"]
[dependency-groups]
test = ["grouped<3", { include-group = "dev" }]
"#;

    fn files(version: &str, time: &str, yanked: bool) -> String {
        format!(r#""{version}":[{{"yanked":{yanked},"upload_time_iso_8601":"{time}"}}]"#)
    }

    fn registry(name: &str) -> Result<String, String> {
        let releases = match name {
            "fresh" => files("1.5", "2026-01-01T00:00:00Z", false),
            "old-pkg" => format!(
                "{},{}",
                files("1.0", "2025-01-01T00:00:00Z", false),
                files("2.0", "2026-01-02T00:00:00Z", false)
            ),
            "tool" => files("1.0", "2026-01-03T00:00:00Z", false),
            "grouped" => files("3.1", "2026-01-04T00:00:00Z", false),
            "yanked" => format!(
                "{},{}",
                files("1.0", "2025-01-01T00:00:00Z", false),
                files("2.0", "2026-01-05T00:00:00Z", true)
            ),
            "pre" => format!(
                "{},{}",
                files("1.0", "2025-01-01T00:00:00Z", false),
                files("2.0rc1", "2026-01-06T00:00:00Z", false)
            ),
            _ => return Err("HTTP 404".to_owned()),
        };
        Ok(format!(r#"{{"releases":{{{releases}}}}}"#))
    }

    fn run_with(only: &[String], prereleases: bool) -> Findings {
        outdated(
            "pypi",
            &[MANIFEST.to_owned()],
            only,
            prereleases,
            DEFAULT_BASE,
            &|url: &str| {
                let name = url
                    .strip_prefix("https://pypi.org/pypi/")
                    .and_then(|rest| rest.strip_suffix("/json"))
                    .unwrap_or(url);
                registry(name)
            },
        )
    }

    fn run(only: &[String]) -> Findings {
        run_with(only, false)
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let found = run_with(&["pre".to_owned()], true);
        assert_eq!(found.candidates[0].latest, "2.0rc1");
        assert!(found.candidates[0].prerelease);
        assert!(run(&["pre".to_owned()]).candidates.is_empty());
    }

    #[test]
    fn project_urls_prefer_source_over_the_homepage() {
        let body = r#"{"info":{"project_urls":{"Homepage":"https://github.com/o/home","Source":"https://github.com/o/src"}}}"#;
        assert_eq!(repo(body).as_deref(), Some("o/src"));
        assert_eq!(repo(r#"{"info":{"project_urls":null}}"#), None);
    }

    fn names(found: &Findings) -> Vec<&str> {
        found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect()
    }

    #[test]
    fn a_specifier_that_still_matches_is_current() {
        let found = run(&[]);
        for current in ["fresh", "tool", "yanked", "pre"] {
            assert!(!names(&found).contains(&current), "{current}");
        }
    }

    #[test]
    fn a_latest_outside_the_specifier_is_listed_with_its_upload_time() {
        let found = run(&[]);
        assert!(found.candidates.contains(&Candidate {
            set: "pypi".into(),
            package: "old-pkg".into(),
            current: ">=1,<2".into(),
            latest: "2.0".into(),
            published: Some("2026-01-02T00:00:00Z".into()),
            prerelease: false,
            repo: None,
            tags: None,
        }));
        assert!(names(&found).contains(&"grouped"));
    }

    #[test]
    fn direct_references_bare_names_and_include_groups_are_skipped() {
        let found = run(&[]);
        let all: Vec<_> = names(&found)
            .into_iter()
            .chain(found.errors.iter().map(|e| e.package.as_str()))
            .collect();
        for skipped in ["direct", "bare", "dev"] {
            assert!(!all.contains(&skipped), "{skipped}");
        }
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "broken");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["old-pkg".to_owned()]);
        assert_eq!(found.candidates.len(), 1);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn requirements_split_into_name_and_specifier() {
        assert_eq!(
            split("Foo.Bar[x,y] (>=1.0) ; marker"),
            Some(("foo-bar".into(), ">=1.0".into()))
        );
        assert_eq!(split("foo>=1"), Some(("foo".into(), ">=1".into())));
        assert_eq!(split("foo @ url"), None);
    }
}
