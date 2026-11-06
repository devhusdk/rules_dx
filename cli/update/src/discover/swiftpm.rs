//! Lists Swift packages whose newest semver tag on GitHub is newer than the Package.resolved pin.

use super::{Candidate, Fetch, Findings, LookupError, github_repo, wanted};
use semver::Version;
use serde_json::Value;

pub const DEFAULT_BASE: &str = "https://api.github.com";

/// Checks each versioned pin of the Package.resolved files against the tags API at `base`.
/// Pins outside GitHub are skipped.
pub fn outdated(
    set: &str,
    resolved: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    for text in resolved {
        let Ok(root) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        let pins = root["pins"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        for pin in pins {
            let (Some(location), Some(current)) =
                (pin["location"].as_str(), pin["state"]["version"].as_str())
            else {
                continue;
            };
            let package = name(location);
            if !wanted(only, &package) || !package.starts_with("github.com/") {
                continue;
            }
            let Some(repo) = github_repo(&package) else {
                continue;
            };
            let error = |reason: String| LookupError {
                set: set.to_owned(),
                package: package.clone(),
                reason,
            };
            let Ok(current_version) = Version::parse(current) else {
                found
                    .errors
                    .push(error(format!("invalid version {current}")));
                continue;
            };
            let body = match fetch.get(&format!("{base}/repos/{repo}/tags?per_page=100")) {
                Ok(body) => body,
                Err(reason) => {
                    found.errors.push(error(reason));
                    continue;
                }
            };
            let Some((latest, tag)) = newest(&body, prereleases) else {
                continue;
            };
            if latest <= current_version {
                continue;
            }
            let prefix = if tag.starts_with('v') { "v" } else { "" };
            found.candidates.push(Candidate {
                set: set.to_owned(),
                package: package.clone(),
                current: current.to_owned(),
                latest: latest.to_string(),
                published: None,
                prerelease: !latest.pre.is_empty(),
                repo: Some(repo),
                tags: Some((format!("{prefix}{current}"), tag)),
            });
        }
    }
    found
}

/// Host and path of a pin location, without scheme, credentials or `.git`.
fn name(location: &str) -> String {
    let url = location.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .unwrap_or(url);
    let url = url
        .strip_prefix("git@")
        .map_or_else(|| url.to_owned(), |scp| scp.replacen(':', "/", 1));
    url.split_once('@')
        .map_or(url.clone(), |(_, rest)| rest.to_owned())
}

fn newest(body: &str, prereleases: bool) -> Option<(Version, String)> {
    let tags: Value = serde_json::from_str(body).ok()?;
    tags.as_array()?
        .iter()
        .filter_map(|tag| tag["name"].as_str())
        .filter_map(|tag| {
            let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
            (prereleases || version.pre.is_empty()).then(|| (version, tag.to_owned()))
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(location: &str, version: &str) -> String {
        format!(
            r#"{{"identity":"x","kind":"remoteSourceControl","location":"{location}","state":{{"revision":"r","version":"{version}"}}}}"#
        )
    }

    fn resolved(pins: &[String]) -> Vec<String> {
        vec![format!(r#"{{"pins":[{}],"version":2}}"#, pins.join(","))]
    }

    fn tags(url: &str) -> Result<String, String> {
        Ok(match url.split("/repos/").nth(1).unwrap_or("") {
            u if u.starts_with("apple/swift-argument-parser/") => {
                r#"[{"name":"1.9.0-beta.1"},{"name":"1.8.3"},{"name":"1.8.2"},{"name":"docs"}]"#
            }
            u if u.starts_with("vapor/vapor/") => r#"[{"name":"v4.50.0"},{"name":"v4.49.0"}]"#,
            _ => return Err("HTTP 404".to_owned()),
        }
        .to_owned())
    }

    fn run(pins: &[String], prereleases: bool) -> Findings {
        outdated(
            "swift",
            &resolved(pins),
            &[],
            prereleases,
            DEFAULT_BASE,
            &tags,
        )
    }

    #[test]
    fn a_newer_stable_tag_is_listed_and_a_prerelease_tag_is_skipped() {
        let found = run(
            &[pin(
                "https://github.com/apple/swift-argument-parser.git",
                "1.8.2",
            )],
            false,
        );
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(
            listed,
            [("github.com/apple/swift-argument-parser", "1.8.2", "1.8.3")]
        );
        assert_eq!(
            found.candidates[0].repo.as_deref(),
            Some("apple/swift-argument-parser")
        );
    }

    #[test]
    fn prerelease_tags_need_the_flag() {
        let pins = [pin(
            "https://github.com/apple/swift-argument-parser",
            "1.8.3",
        )];
        assert!(run(&pins, false).candidates.is_empty());
        let found = run(&pins, true);
        assert!(found.candidates[0].prerelease);
        assert_eq!(found.candidates[0].latest, "1.9.0-beta.1");
    }

    #[test]
    fn a_current_pin_is_not_listed_and_v_tags_keep_their_prefix() {
        assert!(
            run(&[pin("https://github.com/vapor/vapor", "4.50.0")], false)
                .candidates
                .is_empty()
        );
        let found = run(&[pin("https://github.com/vapor/vapor", "4.49.0")], false);
        assert_eq!(
            found.candidates[0].tags,
            Some(("v4.49.0".to_owned(), "v4.50.0".to_owned()))
        );
    }

    #[test]
    fn other_hosts_are_skipped_and_failed_lookups_are_errors() {
        let found = run(
            &[
                pin("https://gitlab.com/o/p.git", "1.0.0"),
                pin("https://github.com/o/gone", "1.0.0"),
            ],
            false,
        );
        assert!(found.candidates.is_empty());
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "github.com/o/gone");
    }
}
