//! Lists Composer packages whose newest stable release on Packagist is newer than composer.lock.

use super::{Candidate, Fetch, Findings, LookupError, github_repo, release_cmp, wanted};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://repo.packagist.org";

/// Checks each direct require of the composer.json files against its locked version.
///
/// `locks` are the composer.lock texts of the same set; a package that is not locked is skipped.
pub fn outdated(
    set: &str,
    manifests: &[String],
    locks: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let locked = locked_versions(locks);
    let mut direct: Vec<String> = manifests.iter().flat_map(|text| requires(text)).collect();
    direct.sort();
    direct.dedup();
    for name in direct {
        if !wanted(only, &name) {
            continue;
        }
        let Some(current) = locked.get(&name) else {
            continue;
        };
        let url = format!("{base}/p2/{name}.json");
        let release = fetch
            .get(&url)
            .and_then(|body| newest(&body, &name, prereleases));
        let (latest, published, prerelease, repo) = match release {
            Ok(release) => release,
            Err(reason) => {
                found.errors.push(LookupError {
                    set: set.to_owned(),
                    package: name,
                    reason,
                });
                continue;
            }
        };
        if release_cmp(&latest, current) == Ordering::Greater {
            found.candidates.push(Candidate {
                set: set.to_owned(),
                package: name,
                current: current.clone(),
                latest,
                published,
                prerelease,
                repo,
                tags: None,
            });
        }
    }
    found
}

/// The packages in `require` and `require-dev`, without the platform pseudo-packages.
fn requires(text: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    ["require", "require-dev"]
        .iter()
        .filter_map(|key| json.get(key)?.as_object())
        .flat_map(|map| map.keys())
        .filter(|name| name.contains('/'))
        .cloned()
        .collect()
}

fn locked_versions(locks: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for text in locks {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            continue;
        };
        for key in ["packages", "packages-dev"] {
            for entry in json
                .get(key)
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                if let (Some(name), Some(version)) =
                    (entry["name"].as_str(), entry["version"].as_str())
                {
                    let version = version.strip_prefix('v').unwrap_or(version);
                    if !version.starts_with("dev-") && !version.ends_with("-dev") {
                        out.insert(name.to_owned(), version.to_owned());
                    }
                }
            }
        }
    }
    out
}

fn is_prerelease(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    ["alpha", "beta", "rc", "dev"]
        .iter()
        .any(|word| lower.contains(word))
}

type Release = (String, Option<String>, bool, Option<String>);

fn newest(body: &str, name: &str, prereleases: bool) -> Result<Release, String> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let entries = json["packages"][name]
        .as_array()
        .ok_or_else(|| "package not found".to_owned())?;
    entries
        .iter()
        .filter_map(|entry| {
            let version = entry["version"].as_str()?;
            if version.starts_with("dev-") || version.ends_with("-dev") {
                return None;
            }
            let version = version.strip_prefix('v').unwrap_or(version);
            let prerelease = is_prerelease(version);
            if prerelease && !prereleases {
                return None;
            }
            let repo = entry["source"]["url"].as_str().and_then(github_repo);
            Some((
                version.to_owned(),
                entry["time"].as_str().map(str::to_owned),
                prerelease,
                repo,
            ))
        })
        .max_by(|a, b| release_cmp(&a.0, &b.0))
        .ok_or_else(|| "no stable version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"require":{"php":"^8.2","ext-json":"*","psr/log":"^1.0","acme/gone":"^1.0"},"require-dev":{"phpunit/phpunit":"^10.0","acme/fresh":"^2.0"}}"#;
    const LOCK: &str = r#"{"packages":[{"name":"psr/log","version":"1.1.3"},{"name":"acme/gone","version":"v1.0.0"},{"name":"acme/fresh","version":"2.0.0"},{"name":"psr/extra","version":"1.0.0"}],"packages-dev":[{"name":"phpunit/phpunit","version":"10.5.1"}]}"#;

    fn api(url: &str) -> Result<String, String> {
        let path = url
            .strip_prefix("https://repo.packagist.org/")
            .unwrap_or(url);
        Ok(match path {
            "p2/psr/log.json" => {
                r#"{"packages":{"psr/log":[{"version":"3.0.0-RC1","time":"2026-03-01T00:00:00+00:00"},{"version":"3.0.2","time":"2026-02-01T00:00:00+00:00","source":{"url":"https://github.com/php-fig/log.git"}},{"version":"dev-master"},{"version":"1.1.3"}]}}"#
            }
            "p2/phpunit/phpunit.json" => {
                r#"{"packages":{"phpunit/phpunit":[{"version":"v10.5.1"}]}}"#
            }
            "p2/acme/fresh.json" => r#"{"packages":{"acme/fresh":[{"version":"2.0.0"}]}}"#,
            _ => return Err("HTTP 404".to_owned()),
        }
        .to_owned())
    }

    fn run(only: &[String], prereleases: bool) -> Findings {
        outdated(
            "php",
            &[JSON.to_owned()],
            &[LOCK.to_owned()],
            only,
            prereleases,
            DEFAULT_BASE,
            &api,
        )
    }

    #[test]
    fn the_newest_stable_release_is_listed() {
        let found = run(&["psr/log".to_owned()], false);
        assert_eq!(
            found.candidates,
            [Candidate {
                set: "php".into(),
                package: "psr/log".into(),
                current: "1.1.3".into(),
                latest: "3.0.2".into(),
                published: Some("2026-02-01T00:00:00+00:00".into()),
                prerelease: false,
                repo: Some("php-fig/log".into()),
                tags: None,
            }]
        );
    }

    #[test]
    fn prereleases_on_lists_the_release_candidate_only_when_newer() {
        let found = run(&["psr/log".to_owned()], true);
        assert_eq!(found.candidates[0].latest, "3.0.2");
    }

    #[test]
    fn platform_requires_and_current_packages_are_not_listed() {
        let found = run(&[], false);
        let names: Vec<_> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect();
        assert_eq!(names, ["psr/log"]);
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[], false);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "acme/gone");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["acme/fresh".to_owned()], false);
        assert!(found.candidates.is_empty() && found.errors.is_empty());
    }
}
