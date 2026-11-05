//! Lists npm dependencies whose latest registry version is outside the declared range.

use super::{wanted, Candidate, Fetch, Findings, LookupError};
use nodejs_semver::{Range, Version};
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://registry.npmjs.org";

const DEP_KEYS: [&str; 3] = ["dependencies", "devDependencies", "optionalDependencies"];
const SKIPPED_PREFIXES: [&str; 9] = [
    "workspace:",
    "file:",
    "link:",
    "portal:",
    "catalog:",
    "npm:",
    "git",
    "github:",
    "http",
];

/// Checks each registry dependency of the package.json files against the registry at `base`.
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
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
            continue;
        };
        for key in DEP_KEYS {
            let Some(table) = doc[key].as_object() else {
                continue;
            };
            for (name, spec) in table {
                let Some(spec) = spec.as_str().map(str::trim) else {
                    continue;
                };
                if skipped(spec) {
                    continue;
                }
                let list = specs.entry(name.clone()).or_default();
                if !list.iter().any(|known| known == spec) {
                    list.push(spec.to_owned());
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
        let (release, repo) = match fetch.get(&format!("{base}/{name}")).and_then(|body| {
            let release = latest(&body, prereleases)?;
            Ok((release, repo(&body)))
        }) {
            Ok(found) => found,
            Err(reason) => {
                found.errors.push(error(reason));
                continue;
            }
        };
        let Some((version, published)) = release else {
            continue;
        };
        for current in list {
            let Ok(range) = Range::parse(&current) else {
                found.errors.push(error(format!("invalid range {current}")));
                continue;
            };
            if !range.satisfies(&version) {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: version.to_string(),
                    published: published.clone(),
                    prerelease: version.is_prerelease(),
                    repo: repo.clone(),
                    tags: None,
                });
            }
        }
    }
    found
}

/// The `latest` stable version of package `name` in the registry at `base`.
pub fn latest_stable(name: &str, base: &str, fetch: &dyn Fetch) -> Result<String, String> {
    let body = fetch.get(&format!("{base}/{name}"))?;
    match latest(&body, false)? {
        Some((version, _)) => Ok(version.to_string()),
        None => Err("no stable version".to_owned()),
    }
}

fn skipped(spec: &str) -> bool {
    SKIPPED_PREFIXES
        .iter()
        .any(|prefix| spec.starts_with(prefix))
        || (spec.contains('/') && !spec.starts_with('@'))
}

fn repo(body: &str) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(body).ok()?;
    let field = &doc["repository"];
    super::github_repo(field.as_str().or_else(|| field["url"].as_str())?)
}

type Latest = Option<(Version, Option<String>)>;

fn latest(body: &str, prereleases: bool) -> Result<Latest, String> {
    let doc: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if prereleases {
        let newest = doc["versions"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, _)| Some((Version::parse(key).ok()?, key)))
            .max_by(|a, b| a.0.cmp(&b.0));
        if let Some((version, key)) = newest {
            let published = doc["time"][key.as_str()].as_str().map(str::to_owned);
            return Ok(Some((version, published)));
        }
    }
    let tag = doc["dist-tags"]["latest"]
        .as_str()
        .ok_or_else(|| "no latest tag".to_owned())?;
    let version = Version::parse(tag).map_err(|e| e.to_string())?;
    if version.is_prerelease() {
        return Ok(None);
    }
    let published = doc["time"][tag].as_str().map(str::to_owned);
    Ok(Some((version, published)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
  "dependencies": {
    "fresh": "^1.2.0",
    "old": "^1.0.0",
    "@scope/pkg": "~2.0.0",
    "local": "workspace:*",
    "file": "file:../x",
    "repo": "user/repo",
    "tarball": "https://example.com/a.tgz",
    "alias": "npm:other@1",
    "broken": "^1.0.0",
    "bad": "not a range!!"
  },
  "devDependencies": { "tool": ">=3" },
  "optionalDependencies": { "pre": "^1.0.0" }
}"#;

    fn doc(latest: &str, time: &str) -> Result<String, String> {
        Ok(format!(
            r#"{{"dist-tags":{{"latest":"{latest}"}},"versions":{{"{latest}":{{}}}},"time":{{"{latest}":"{time}"}}}}"#
        ))
    }

    fn registry(name: &str) -> Result<String, String> {
        match name {
            "fresh" => doc("1.9.0", "2026-01-01T00:00:00.000Z"),
            "old" => doc("2.0.0", "2026-01-02T00:00:00.000Z"),
            "@scope/pkg" => doc("2.1.0", "2026-01-03T00:00:00.000Z"),
            "tool" => doc("3.1.0", "2026-01-04T00:00:00.000Z"),
            "bad" => doc("1.0.0", "2026-01-05T00:00:00.000Z"),
            "pre" => doc("2.0.0-beta.1", "2026-01-06T00:00:00.000Z"),
            _ => Err("HTTP 404".to_owned()),
        }
    }

    fn run(only: &[String]) -> Findings {
        run_with(only, false)
    }

    fn run_with(only: &[String], prereleases: bool) -> Findings {
        outdated(
            "npm",
            &[MANIFEST.to_owned()],
            only,
            prereleases,
            DEFAULT_BASE,
            &|url: &str| {
                registry(
                    url.strip_prefix("https://registry.npmjs.org/")
                        .unwrap_or(url),
                )
            },
        )
    }

    fn names(found: &Findings) -> Vec<&str> {
        found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect()
    }

    #[test]
    fn the_repository_field_may_be_a_string_or_an_object() {
        let object = r#"{"repository":{"type":"git","url":"git+https://github.com/o/r.git"}}"#;
        assert_eq!(repo(object).as_deref(), Some("o/r"));
        assert_eq!(
            repo(r#"{"repository":"github:o/r"}"#).as_deref(),
            Some("o/r")
        );
        assert_eq!(repo("{}"), None);
    }

    #[test]
    fn a_range_that_still_matches_is_current() {
        let found = run(&[]);
        assert!(!names(&found).contains(&"fresh"));
        assert!(!names(&found).contains(&"tool"));
    }

    #[test]
    fn a_latest_outside_the_range_is_listed_with_its_publish_time() {
        let found = run(&[]);
        assert!(found.candidates.contains(&Candidate {
            set: "npm".into(),
            package: "old".into(),
            current: "^1.0.0".into(),
            latest: "2.0.0".into(),
            published: Some("2026-01-02T00:00:00.000Z".into()),
            prerelease: false,
            repo: None,
            tags: None,
        }));
    }

    #[test]
    fn scoped_names_are_looked_up_as_is() {
        assert!(names(&run(&[])).contains(&"@scope/pkg"));
    }

    #[test]
    fn local_git_url_and_alias_specs_are_skipped() {
        let found = run(&[]);
        let all: Vec<_> = names(&found)
            .into_iter()
            .chain(found.errors.iter().map(|e| e.package.as_str()))
            .collect();
        for skipped in ["local", "file", "repo", "tarball", "alias"] {
            assert!(!all.contains(&skipped), "{skipped}");
        }
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let found = run_with(&["pre".to_owned()], true);
        assert_eq!(found.candidates[0].latest, "2.0.0-beta.1");
        assert!(found.candidates[0].prerelease);
        assert_eq!(
            found.candidates[0].published.as_deref(),
            Some("2026-01-06T00:00:00.000Z")
        );
    }

    #[test]
    fn prerelease_latest_lists_nothing() {
        assert!(!names(&run(&[])).contains(&"pre"));
    }

    #[test]
    fn failed_lookups_and_bad_ranges_are_errors() {
        let found = run(&[]);
        let errors: Vec<_> = found
            .errors
            .iter()
            .map(|e| (e.package.as_str(), e.reason.as_str()))
            .collect();
        assert!(errors.contains(&("broken", "HTTP 404")));
        assert!(errors.contains(&("bad", "invalid range not a range!!")));
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["old".to_owned()]);
        assert_eq!(found.candidates.len(), 1);
        assert!(found.errors.is_empty());
    }
}
