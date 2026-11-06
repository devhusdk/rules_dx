//! Lists Cargo dependencies whose newest stable crates.io version is outside the declared requirement.

use super::{github_repo, wanted, Candidate, Fetch, Findings, LookupError};
use semver::{Version, VersionReq};
use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item, Table};

pub const DEFAULT_BASE: &str = "https://index.crates.io";
pub const API_BASE: &str = "https://crates.io";

const DEP_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Checks each registry dependency of the manifests against the sparse index at `base`.
/// Outdated crates get their GitHub repository from the crates.io API at `api`.
pub fn outdated(
    set: &str,
    manifests: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    api: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut wanted_reqs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in manifests {
        let Ok(doc) = text.parse::<DocumentMut>() else {
            continue;
        };
        collect(doc.as_table(), &mut wanted_reqs);
    }
    for (name, reqs) in wanted_reqs {
        if !wanted(only, &name) {
            continue;
        }
        let error = |reason: String| LookupError {
            set: set.to_owned(),
            package: name.clone(),
            reason,
        };
        let release = match fetch
            .get(&format!("{base}/{}", index_path(&name)))
            .and_then(|body| newest(&body, prereleases))
        {
            Ok(release) => release,
            Err(reason) => {
                found.errors.push(error(reason));
                continue;
            }
        };
        let mut repo = None;
        let mut looked_up = false;
        for current in reqs {
            let Ok(req) = VersionReq::parse(&current) else {
                found
                    .errors
                    .push(error(format!("invalid requirement {current}")));
                continue;
            };
            if !req.matches(&release.version) {
                if !looked_up {
                    looked_up = true;
                    repo = repository(api, &name, fetch);
                }
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: release.version.to_string(),
                    published: release.published.clone(),
                    prerelease: !release.version.pre.is_empty(),
                    repo: repo.clone(),
                    tags: None,
                });
            }
        }
    }
    found
}

/// The newest stable version of crate `name` in the sparse index at `base`.
pub fn latest_stable(name: &str, base: &str, fetch: &dyn Fetch) -> Result<String, String> {
    let body = fetch.get(&format!("{base}/{}", index_path(name)))?;
    newest(&body, false).map(|release| release.version.to_string())
}

fn repository(api: &str, name: &str, fetch: &dyn Fetch) -> Option<String> {
    let body = fetch.get(&format!("{api}/api/v1/crates/{name}")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    json["crate"]["repository"].as_str().and_then(github_repo)
}

fn collect(root: &Table, into: &mut BTreeMap<String, Vec<String>>) {
    for key in DEP_TABLES {
        add_table(root.get(key), into);
    }
    add_table(
        root.get("workspace")
            .and_then(Item::as_table_like)
            .and_then(|workspace| workspace.get("dependencies")),
        into,
    );
    if let Some(targets) = root.get("target").and_then(Item::as_table_like) {
        for (_, target) in targets.iter() {
            if let Some(target) = target.as_table_like() {
                for key in DEP_TABLES {
                    add_table(target.get(key), into);
                }
            }
        }
    }
}

fn add_table(item: Option<&Item>, into: &mut BTreeMap<String, Vec<String>>) {
    let Some(table) = item.and_then(Item::as_table_like) else {
        return;
    };
    for (name, entry) in table.iter() {
        if let Some(req) = requirement(entry) {
            let reqs = into.entry(name.to_owned()).or_default();
            if !reqs.contains(&req) {
                reqs.push(req);
            }
        }
    }
}

fn requirement(entry: &Item) -> Option<String> {
    if let Some(text) = entry.as_str() {
        return Some(text.to_owned());
    }
    let table = entry.as_table_like()?;
    let skipped = ["path", "git", "workspace", "package"];
    if skipped.iter().any(|key| table.contains_key(key)) {
        return None;
    }
    table.get("version")?.as_str().map(str::to_owned)
}

pub fn index_path(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.len() {
        1 => format!("1/{lower}"),
        2 => format!("2/{lower}"),
        3 => format!("3/{}/{lower}", &lower[..1]),
        _ => format!("{}/{}/{lower}", &lower[..2], &lower[2..4]),
    }
}

struct Release {
    version: Version,
    published: Option<String>,
}

/// One index line: the version, whether it is yanked and its publish time.
pub struct IndexEntry {
    pub version: String,
    pub yanked: bool,
    pub published: Option<String>,
}

pub fn index_entries(body: &str) -> Result<Vec<IndexEntry>, String> {
    let mut out = Vec::new();
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let entry: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let Some(version) = entry["vers"].as_str() else {
            continue;
        };
        out.push(IndexEntry {
            version: version.to_owned(),
            yanked: entry["yanked"].as_bool().unwrap_or(false),
            published: entry["pubtime"].as_str().map(str::to_owned),
        });
    }
    Ok(out)
}

/// The yanked versions one crate's index file lists.
pub fn yanked_versions(body: &str) -> Result<Vec<String>, String> {
    Ok(index_entries(body)?
        .into_iter()
        .filter(|entry| entry.yanked)
        .map(|entry| entry.version)
        .collect())
}

fn newest(body: &str, prereleases: bool) -> Result<Release, String> {
    let mut best: Option<Release> = None;
    for entry in index_entries(body)? {
        if entry.yanked {
            continue;
        }
        let Ok(version) = Version::parse(&entry.version) else {
            continue;
        };
        if !prereleases && !version.pre.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|b| version > b.version) {
            best = Some(Release {
                version,
                published: entry.published,
            });
        }
    }
    best.ok_or_else(|| "no stable version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yanked_versions_lists_only_the_yanked_lines() {
        let body = "{\"vers\":\"1.0.0\",\"yanked\":true}\n{\"vers\":\"1.1.0\",\"yanked\":false}\n\n{\"vers\":\"2.0.0\",\"yanked\":true}\n";
        assert_eq!(yanked_versions(body).unwrap(), vec!["1.0.0", "2.0.0"]);
        assert!(yanked_versions("not json").is_err());
    }

    const MANIFEST: &str = r#"
[dependencies]
anyhow = "1"
serde = { version = "1.0.100", features = ["derive"] }
old = "0.3"
local = { path = "../local", version = "0.1" }
renamed = { package = "real", version = "0.1" }
shared = { workspace = true }
broken = "1"

[dev-dependencies]
tool = "2"

[workspace.dependencies]
wsdep = "1.0"
"#;

    fn index(name: &str) -> Result<String, String> {
        let body = match name {
            "an/yh/anyhow" => r#"{"vers":"1.0.104","yanked":false}"#,
            "se/rd/serde" => r#"{"vers":"1.0.200","yanked":false}"#,
            "3/o/old" => {
                "{\"vers\":\"0.3.1\",\"yanked\":false}\n{\"vers\":\"0.4.0\",\"yanked\":false,\"pubtime\":\"2026-01-02T00:00:00Z\"}\n{\"vers\":\"0.5.0\",\"yanked\":true}\n{\"vers\":\"0.6.0-rc.1\",\"yanked\":false}"
            }
            "to/ol/tool" => r#"{"vers":"2.1.0","yanked":false}"#,
            "ws/de/wsdep" => r#"{"vers":"2.0.0","yanked":false}"#,
            _ => return Err("HTTP 404".to_owned()),
        };
        Ok(body.to_owned())
    }

    fn run_with(only: &[String], prereleases: bool) -> Findings {
        outdated(
            "cargo",
            &[MANIFEST.to_owned()],
            only,
            prereleases,
            DEFAULT_BASE,
            API_BASE,
            &|url: &str| match url.strip_prefix("https://crates.io/api/v1/crates/") {
                Some("old") => {
                    Ok(r#"{"crate":{"repository":"https://github.com/acme/old"}}"#.to_owned())
                }
                Some(_) => Err("HTTP 500".to_owned()),
                None => index(url.strip_prefix("https://index.crates.io/").unwrap_or(url)),
            },
        )
    }

    fn run(only: &[String]) -> Findings {
        run_with(only, false)
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let found = run_with(&["old".to_owned()], true);
        assert_eq!(found.candidates[0].latest, "0.6.0-rc.1");
        assert!(found.candidates[0].prerelease);
        assert!(!run(&["old".to_owned()]).candidates[0].prerelease);
    }

    #[test]
    fn a_requirement_that_still_matches_is_current() {
        let found = run(&[]);
        let names: Vec<_> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect();
        assert!(!names.contains(&"anyhow"));
        assert!(!names.contains(&"serde"));
        assert!(!names.contains(&"tool"));
    }

    #[test]
    fn a_stable_version_outside_the_range_is_listed_with_its_publish_time() {
        let found = run(&[]);
        let old = found.candidates.iter().find(|c| c.package == "old");
        assert_eq!(
            old,
            Some(&Candidate {
                set: "cargo".into(),
                package: "old".into(),
                current: "0.3".into(),
                latest: "0.4.0".into(),
                published: Some("2026-01-02T00:00:00Z".into()),
                prerelease: false,
                repo: Some("acme/old".into()),
                tags: None,
            })
        );
        assert!(found.candidates.iter().any(|c| c.package == "wsdep"));
    }

    #[test]
    fn a_failed_metadata_fetch_leaves_the_repository_empty() {
        let found = run(&[]);
        let wsdep = found.candidates.iter().find(|c| c.package == "wsdep");
        assert_eq!(wsdep.map(|c| c.repo.clone()), Some(None));
    }

    #[test]
    fn path_renamed_and_workspace_entries_are_not_looked_up() {
        let found = run(&[]);
        let all: Vec<_> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .chain(found.errors.iter().map(|e| e.package.as_str()))
            .collect();
        for skipped in ["local", "renamed", "real", "shared"] {
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
        let found = run(&["old".to_owned()]);
        assert_eq!(found.candidates.len(), 1);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn index_paths_follow_the_sparse_layout() {
        assert_eq!(index_path("a"), "1/a");
        assert_eq!(index_path("ab"), "2/ab");
        assert_eq!(index_path("abc"), "3/a/abc");
        assert_eq!(index_path("Serde"), "se/rd/serde");
    }
}
