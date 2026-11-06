//! Lists locked hex packages whose newest release on hex.pm is newer than the one in mix.lock.

use super::{Candidate, Fetch, Findings, LookupError, release_cmp, wanted};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://hex.pm";

/// Checks each hexpm package of the mix.lock files against the API at `base`.
pub fn outdated(
    set: &str,
    lockfiles: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut locked: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in lockfiles {
        for (name, version) in locked_packages(text) {
            let versions = locked.entry(name).or_default();
            if !versions.contains(&version) {
                versions.push(version);
            }
        }
    }
    for (name, versions) in locked {
        if !wanted(only, &name) {
            continue;
        }
        let url = format!("{base}/api/packages/{name}");
        let (latest, published) = match fetch.get(&url).and_then(|body| newest(&body, prereleases))
        {
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
        for current in versions {
            if release_cmp(&latest, &current) == Ordering::Greater {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: latest.clone(),
                    published: published.clone(),
                    prerelease: latest.contains('-'),
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

/// Reads `(package, version)` of every `{:hex, :package, "version", ...}` entry on the
/// default hexpm repository; git entries and other repositories are skipped.
fn locked_packages(lock: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in lock.lines() {
        let Some((_, tuple)) = line.trim().split_once("{:hex,") else {
            continue;
        };
        let mut fields = tuple.split(',').map(str::trim);
        let (Some(package), Some(version)) = (fields.next(), fields.next()) else {
            continue;
        };
        let repo = tuple
            .trim_end_matches([',', '}', ' '])
            .rsplit(',')
            .nth(1)
            .map(str::trim);
        if repo != Some("\"hexpm\"") {
            continue;
        }
        let package = package.trim_start_matches(':').trim_matches('"');
        out.push((package.to_owned(), version.trim_matches('"').to_owned()));
    }
    out
}

fn newest(body: &str, prereleases: bool) -> Result<(String, Option<String>), String> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let releases = json["releases"].as_array().map_or(&[][..], Vec::as_slice);
    let mut best = json["latest_stable_version"].as_str().map(str::to_owned);
    if prereleases || best.is_none() {
        for entry in releases {
            let Some(version) = entry["version"].as_str() else {
                continue;
            };
            let newer = best
                .as_ref()
                .is_none_or(|best| release_cmp(version, best) == Ordering::Greater);
            if newer && (prereleases || !version.contains('-')) {
                best = Some(version.to_owned());
            }
        }
    }
    let best = best.ok_or_else(|| "no version".to_owned())?;
    let published = releases
        .iter()
        .find(|entry| entry["version"].as_str() == Some(best.as_str()))
        .and_then(|entry| entry["inserted_at"].as_str())
        .map(str::to_owned);
    Ok((best, published))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"%{
  "jason": {:hex, :jason, "1.4.0", "e855647bc964a44e2f67df589ccf49105ae039d4179db7f6271dfd3843dc27e6", [:mix], [{:decimal, "~> 2.0", [hex: :decimal, repo: "hexpm", optional: true]}], "hexpm", "79a3791085b2a0f743ca04cec0f7be26443738779d09302e01318f97bdb82121"},
  "current": {:hex, :current, "2.0.0", "aa", [:mix], [], "hexpm", "bb"},
  "pre": {:hex, :pre, "1.0.0", "aa", [:mix], [], "hexpm", "bb"},
  "private": {:hex, :private, "1.0.0", "aa", [:mix], [], "acme", "bb"},
  "from_git": {:git, "https://github.com/acme/from_git.git", "abc", []},
}
"#;

    fn api(url: &str) -> Result<String, String> {
        let name = url
            .strip_prefix("https://hex.pm/api/packages/")
            .unwrap_or(url);
        Ok(match name {
            "jason" => r#"{"latest_stable_version":"1.4.4","releases":[{"version":"1.5.0-rc.1"},{"version":"1.4.4","inserted_at":"2026-03-01T00:00:00Z"}]}"#,
            "current" => r#"{"latest_stable_version":"2.0.0","releases":[]}"#,
            "pre" => r#"{"latest_stable_version":"1.0.0","releases":[{"version":"2.0.0-rc.1","inserted_at":"2026-04-01T00:00:00Z"}]}"#,
            _ => return Err(format!("unexpected lookup {name}")),
        }
        .to_owned())
    }

    #[test]
    fn reads_only_hexpm_entries() {
        let names: Vec<_> = locked_packages(LOCK).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["jason", "current", "pre"]);
    }

    #[test]
    fn lists_outdated_and_skips_current_and_other_sources() {
        let found = outdated("deps", &[LOCK.to_owned()], &[], false, DEFAULT_BASE, &api);
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(listed, [("jason", "1.4.0", "1.4.4")]);
        assert_eq!(
            found.candidates[0].published.as_deref(),
            Some("2026-03-01T00:00:00Z")
        );
        assert!(found.errors.is_empty());
    }

    #[test]
    fn prereleases_need_the_flag() {
        let only = ["pre".to_owned()];
        let text = [LOCK.to_owned()];
        assert!(
            outdated("deps", &text, &only, false, DEFAULT_BASE, &api)
                .candidates
                .is_empty()
        );
        let found = outdated("deps", &text, &only, true, DEFAULT_BASE, &api);
        assert_eq!(found.candidates[0].latest, "2.0.0-rc.1");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn a_failed_lookup_is_reported() {
        let found = outdated(
            "deps",
            &[LOCK.to_owned()],
            &[],
            false,
            DEFAULT_BASE,
            &|_: &str| Err("down".to_owned()),
        );
        assert_eq!(found.errors.len(), 3);
    }
}
