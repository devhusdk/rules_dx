//! Lists DUB selections whose newest release on code.dlang.org is newer than the pinned one.

use super::{Candidate, Fetch, Findings, LookupError, release_cmp, wanted};
use std::cmp::Ordering;

pub const DEFAULT_BASE: &str = "https://code.dlang.org";

/// Checks each package of the dub.selections.json files against the API at `base`.
pub fn outdated(
    set: &str,
    selections: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    for text in selections {
        let pinned = match pinned(text) {
            Ok(pinned) => pinned,
            Err(reason) => {
                found.errors.push(LookupError {
                    set: set.to_owned(),
                    package: "dub.selections.json".to_owned(),
                    reason,
                });
                continue;
            }
        };
        for (name, current) in pinned {
            if !wanted(only, &name) {
                continue;
            }
            let url = format!("{base}/api/packages/{name}/latest");
            let latest = match fetch.get(&url).and_then(|body| latest(&body)) {
                Ok(latest) => latest,
                Err(reason) => {
                    found.errors.push(LookupError {
                        set: set.to_owned(),
                        package: name,
                        reason,
                    });
                    continue;
                }
            };
            if (prereleases || !latest.contains('-'))
                && release_cmp(&latest, &current) == Ordering::Greater
            {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name,
                    current,
                    prerelease: latest.contains('-'),
                    latest,
                    published: None,
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

/// The `(name, version)` pairs of released versions; branch pins such as `~master` are skipped.
fn pinned(text: &str) -> Result<Vec<(String, String)>, String> {
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid dub.selections.json: {e}"))?;
    let versions = root["versions"]
        .as_object()
        .ok_or("invalid dub.selections.json: missing versions")?;
    Ok(versions
        .iter()
        .filter_map(|(name, entry)| {
            let version = entry.as_str().or_else(|| entry["version"].as_str())?;
            version
                .starts_with(|c: char| c.is_ascii_digit())
                .then(|| (name.clone(), version.to_owned()))
        })
        .collect())
}

fn latest(body: &str) -> Result<String, String> {
    serde_json::from_str::<String>(body.trim()).map_err(|_| "no version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SELECTIONS: &str = r#"{"fileVersion":1,"versions":{"semver":"0.8.0","vibe-d":"0.9.0","current":"1.0.0","branch":"~master","beta":"1.0.0"}}"#;

    fn api(url: &str) -> Result<String, String> {
        let name = url
            .strip_prefix("https://code.dlang.org/api/packages/")
            .and_then(|rest| rest.strip_suffix("/latest"))
            .unwrap_or(url);
        Ok(match name {
            "semver" => r#""0.9.1""#,
            "vibe-d" => r#""0.9.0""#,
            "current" => r#""1.0.0""#,
            "beta" => r#""2.0.0-beta.1""#,
            _ => return Err(format!("unexpected lookup {name}")),
        }
        .to_owned())
    }

    #[test]
    fn lists_newer_releases_and_skips_branch_pins() {
        let found = outdated(
            "dub",
            &[SELECTIONS.to_owned()],
            &[],
            false,
            DEFAULT_BASE,
            &api,
        );
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(listed, [("semver", "0.8.0", "0.9.1")]);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn prereleases_need_the_flag() {
        let only = ["beta".to_owned()];
        let text = [SELECTIONS.to_owned()];
        assert!(
            outdated("dub", &text, &only, false, DEFAULT_BASE, &api)
                .candidates
                .is_empty()
        );
        let found = outdated("dub", &text, &only, true, DEFAULT_BASE, &api);
        assert!(found.candidates[0].prerelease);
    }
}
