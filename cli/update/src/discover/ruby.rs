//! Lists gems whose newest release on rubygems.org is newer than the version in the Gemfile.

use super::{
    Candidate, Fetch, Findings, LookupError, github_repo, numeric_cmp, release_cmp, wanted,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://rubygems.org";

/// Checks each versioned `gem` line of the Gemfiles against the API at `base`.
pub fn outdated(
    set: &str,
    gemfiles: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut declared: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in gemfiles {
        for line in text.lines() {
            if let Some((name, current)) = declaration(line) {
                let versions = declared.entry(name).or_default();
                if !versions.contains(&current) {
                    versions.push(current);
                }
            }
        }
    }
    for (name, versions) in declared {
        if !wanted(only, &name) {
            continue;
        }
        let url = format!("{base}/api/v1/versions/{name}.json");
        let (latest, published, prerelease) =
            match fetch.get(&url).and_then(|body| newest(&body, prereleases)) {
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
        let mut repo = None;
        let mut looked_up = false;
        for current in versions {
            if release_cmp(&latest, &current) == Ordering::Greater {
                if !looked_up {
                    looked_up = true;
                    repo = repository(base, &name, fetch);
                }
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: latest.clone(),
                    published: published.clone(),
                    prerelease,
                    repo: repo.clone(),
                    tags: None,
                });
            }
        }
    }
    found
}

/// The newest stable release of gem `name` on the API at `base`.
pub fn latest_stable(name: &str, base: &str, fetch: &dyn Fetch) -> Result<String, String> {
    let body = fetch.get(&format!("{base}/api/v1/versions/{name}.json"))?;
    newest(&body, false).map(|(version, _, _)| version)
}

fn repository(base: &str, name: &str, fetch: &dyn Fetch) -> Option<String> {
    let body = fetch.get(&format!("{base}/api/v1/gems/{name}.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    ["source_code_uri", "homepage_uri"]
        .iter()
        .find_map(|key| json[key].as_str().and_then(github_repo))
}

fn declaration(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("gem ")?;
    let mut args = rest.split(',').map(str::trim);
    let name = unquote(args.next()?)?;
    let mut best: Option<&str> = None;
    for arg in args {
        let Some(requirement) = unquote(arg) else {
            break;
        };
        let literal = requirement.trim_start_matches(['>', '<', '=', '~', ' ']);
        if literal.starts_with(|c: char| c.is_ascii_digit())
            && best.is_none_or(|b| numeric_cmp(literal, b) == Ordering::Greater)
        {
            best = Some(literal);
        }
    }
    best.map(|current| (name.to_owned(), current.to_owned()))
}

fn unquote(text: &str) -> Option<&str> {
    let quote = text.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let inner = &text[1..];
    inner.split_once(quote).map(|(inside, _)| inside)
}

fn newest(body: &str, prereleases: bool) -> Result<(String, Option<String>, bool), String> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let number = entry["number"].as_str()?;
            let prerelease = entry["prerelease"].as_bool().unwrap_or(false);
            if prerelease && !prereleases {
                return None;
            }
            Some((
                number.to_owned(),
                entry["created_at"].as_str().map(str::to_owned),
                prerelease,
            ))
        })
        .max_by(|a, b| release_cmp(&a.0, &b.0))
        .ok_or_else(|| "no stable version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEMFILE: &str = "source 'https://rubygems.org'\ngem 'rake'\ngem \"rails\", \"~> 7.0\", \">= 7.0.4\"\ngem 'fresh', '2.0', require: false\ngem 'gone', '1.0'\n  gem 'nokogiri', '>= 1.0'\n";

    fn api(url: &str) -> Result<String, String> {
        let path = url.strip_prefix("https://rubygems.org/").unwrap_or(url);
        Ok(match path {
            "api/v1/versions/rails.json" => {
                r#"[{"number":"8.0.0.rc1","prerelease":true,"created_at":"2026-03-01T00:00:00Z"},{"number":"7.2.1","prerelease":false,"created_at":"2026-02-01T00:00:00Z"},{"number":"7.0.4","prerelease":false,"created_at":"2025-01-01T00:00:00Z"}]"#
            }
            "api/v1/gems/rails.json" => {
                r#"{"source_code_uri":"https://example.org/rails","homepage_uri":"https://github.com/rails/rails"}"#
            }
            "api/v1/versions/fresh.json" => r#"[{"number":"2.0","prerelease":false}]"#,
            "api/v1/versions/nokogiri.json" => r#"[{"number":"1.16.0","prerelease":false}]"#,
            _ => return Err("HTTP 404".to_owned()),
        }
        .to_owned())
    }

    fn run(only: &[String]) -> Findings {
        outdated(
            "ruby",
            &[GEMFILE.to_owned()],
            only,
            false,
            DEFAULT_BASE,
            &api,
        )
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let only = ["rails".to_owned()];
        let found = outdated(
            "ruby",
            &[GEMFILE.to_owned()],
            &only,
            true,
            DEFAULT_BASE,
            &api,
        );
        assert_eq!(found.candidates[0].latest, "8.0.0.rc1");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn the_newest_release_is_listed_with_its_created_time() {
        let found = run(&["rails".to_owned()]);
        assert_eq!(
            found.candidates,
            [Candidate {
                set: "ruby".into(),
                package: "rails".into(),
                current: "7.0.4".into(),
                latest: "7.2.1".into(),
                published: Some("2026-02-01T00:00:00Z".into()),
                prerelease: false,
                repo: Some("rails/rails".into()),
                tags: None,
            }]
        );
    }

    #[test]
    fn a_failed_metadata_fetch_leaves_the_repository_empty() {
        let found = run(&["nokogiri".to_owned()]);
        assert_eq!(found.candidates[0].repo, None);
    }

    #[test]
    fn unversioned_and_current_gems_are_not_looked_up() {
        let found = run(&[]);
        let names: Vec<_> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect();
        assert_eq!(names, ["nokogiri", "rails"]);
        assert!(!found.errors.iter().any(|e| e.package == "rake"));
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "gone");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["fresh".to_owned()]);
        assert!(found.candidates.is_empty() && found.errors.is_empty());
    }
}
