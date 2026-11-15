//! Lists locked hex packages whose newest release on hex.pm is newer than the one in the lock.

use dx_error::DxError;

use super::{Fetch, Findings, Latest, compare_declared, release_cmp};
use std::cmp::Ordering;

pub const DEFAULT_BASE: &str = "https://hex.pm";

/// Checks each package of the lock files against the API at `base`.
pub fn outdated(
    set: &str,
    lockfiles: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let declared = lockfiles.iter().flat_map(|text| locked_packages(text));
    compare_declared(set, declared, only, release_cmp, |name| {
        let url = format!("{base}/api/packages/{name}");
        let (latest, published) = fetch
            .get(&url)
            .and_then(|body| newest(&body, prereleases))?;
        Ok(Some(Latest {
            prerelease: latest.contains('-'),
            version: latest,
            published,
            repo: None,
        }))
    })
}

/// Reads `(package, version)` of every package of a hex lock.
fn locked_packages(lock: &str) -> Vec<(String, String)> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(lock) else {
        return Vec::new();
    };
    json.get("packages")
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|package| {
            let field = |key: &str| package.get(key).and_then(serde_json::Value::as_str);
            Some((field("name")?.to_owned(), field("version")?.to_owned()))
        })
        .collect()
}

fn newest(body: &str, prereleases: bool) -> Result<(String, Option<String>), DxError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| DxError::failure(e.to_string()))?;
    let releases = json
        .get("releases")
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut best = json
        .get("latest_stable_version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if prereleases || best.is_none() {
        for entry in releases {
            let Some(version) = entry.get("version").and_then(serde_json::Value::as_str) else {
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
    let best = best.ok_or_else(|| DxError::failure("no version"))?;
    let published = releases
        .iter()
        .find(|entry| {
            entry.get("version").and_then(serde_json::Value::as_str) == Some(best.as_str())
        })
        .and_then(|entry| entry.get("inserted_at").and_then(serde_json::Value::as_str))
        .map(str::to_owned);
    Ok((best, published))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"{"ecosystem":"hex","packages":[
  {"name":"jason","version":"1.4.0"},
  {"name":"current","version":"2.0.0"},
  {"name":"pre","version":"1.0.0"}
]}"#;

    fn api(url: &str) -> Result<String, DxError> {
        let name = url
            .strip_prefix("https://hex.pm/api/packages/")
            .unwrap_or(url);
        Ok(match name {
            "jason" => r#"{"latest_stable_version":"1.4.4","releases":[{"version":"1.5.0-rc.1"},{"version":"1.4.4","inserted_at":"2026-03-01T00:00:00Z"}]}"#,
            "current" => r#"{"latest_stable_version":"2.0.0","releases":[]}"#,
            "pre" => r#"{"latest_stable_version":"1.0.0","releases":[{"version":"2.0.0-rc.1","inserted_at":"2026-04-01T00:00:00Z"}]}"#,
            _ => return Err(DxError::failure(format!("unexpected lookup {name}"))),
        }
        .to_owned())
    }

    #[test]
    fn reads_every_locked_package() {
        let names: Vec<_> = locked_packages(LOCK).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["jason", "current", "pre"]);
        assert!(locked_packages("not json").is_empty());
    }

    #[test]
    fn lists_outdated_and_skips_current() {
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
            &|_: &str| Err(DxError::failure("down")),
        );
        assert_eq!(found.errors.len(), 3);
    }
}
