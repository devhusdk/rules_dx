//! Lists direct Julia packages whose newest General registry release is newer than the one locked.

use super::{Candidate, Fetch, Findings, LookupError, release_cmp, wanted};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://raw.githubusercontent.com/JuliaRegistries/General/master";

/// Checks each `[deps]` package of the Project.toml files, as locked in the Manifest.toml files.
pub fn outdated(
    set: &str,
    projects: &[String],
    manifests: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let direct = direct_deps(projects);
    let mut locked: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in manifests {
        for (name, version) in locked_versions(text) {
            if direct.contains(&name) {
                let versions = locked.entry(name).or_default();
                if !versions.contains(&version) {
                    versions.push(version);
                }
            }
        }
    }
    for (name, versions) in locked {
        if !wanted(only, &name) {
            continue;
        }
        let first = name.chars().next().unwrap_or('_');
        let url = format!("{base}/{first}/{name}/Versions.toml");
        let latest = match fetch.get(&url).and_then(|body| newest(&body, prereleases)) {
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
                    published: None,
                    prerelease: latest.contains('-'),
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

fn direct_deps(projects: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for text in projects {
        let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
            continue;
        };
        if let Some(deps) = doc.get("deps").and_then(toml_edit::Item::as_table_like) {
            names.extend(deps.iter().map(|(name, _)| name.to_owned()));
        }
    }
    names
}

/// Reads `(name, version)` of every versioned `[[deps.Name]]` entry, without build metadata.
fn locked_versions(manifest: &str) -> Vec<(String, String)> {
    let Ok(doc) = manifest.parse::<toml_edit::DocumentMut>() else {
        return Vec::new();
    };
    let Some(deps) = doc.get("deps").and_then(toml_edit::Item::as_table_like) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, entries) in deps.iter() {
        let Some(entries) = entries.as_array_of_tables() else {
            continue;
        };
        for entry in entries {
            if let Some(version) = entry.get("version").and_then(toml_edit::Item::as_str) {
                out.push((name.to_owned(), strip_build(version)));
            }
        }
    }
    out
}

/// The newest version of a registry `Versions.toml`, skipping yanked ones.
fn newest(body: &str, prereleases: bool) -> Result<String, String> {
    let doc: toml_edit::DocumentMut = body
        .parse()
        .map_err(|e| format!("invalid Versions.toml: {e}"))?;
    let mut best: Option<String> = None;
    for (version, entry) in doc.iter() {
        let yanked = entry
            .get("yanked")
            .and_then(toml_edit::Item::as_bool)
            .unwrap_or(false);
        let version = strip_build(version);
        if yanked || (!prereleases && version.contains('-')) {
            continue;
        }
        if best
            .as_deref()
            .is_none_or(|top| release_cmp(&version, top) == Ordering::Greater)
        {
            best = Some(version);
        }
    }
    best.ok_or_else(|| "no usable version".to_owned())
}

fn strip_build(version: &str) -> String {
    version.split('+').next().unwrap_or(version).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "name = \"app\"\n\n[deps]\nOrderedCollections = \"bac558e1\"\nPreferences = \"21216c6a\"\nJSON = \"682c06a0\"\nDates = \"ade2ca70\"\n";
    const MANIFEST: &str = r#"manifest_format = "2.0"

[[deps.Dates]]
uuid = "ade2ca70"

[[deps.JSON]]
uuid = "682c06a0"
version = "0.21.4"

[[deps.OrderedCollections]]
uuid = "bac558e1"
version = "1.6.0"

[[deps.Preferences]]
uuid = "21216c6a"
version = "1.5.0"

[[deps.Transitive]]
uuid = "00000000"
version = "0.1.0"
"#;

    fn registry(url: &str) -> Result<String, String> {
        let name = url
            .strip_prefix(&format!("{DEFAULT_BASE}/"))
            .and_then(|rest| rest.strip_suffix("/Versions.toml"))
            .unwrap_or(url);
        Ok(match name {
            "O/OrderedCollections" => "[\"1.6.0\"]\ngit-tree-sha1 = \"a\"\n\n[\"1.8.1\"]\ngit-tree-sha1 = \"b\"\n\n[\"1.9.0\"]\ngit-tree-sha1 = \"c\"\nyanked = true\n",
            "P/Preferences" => "[\"1.5.0\"]\ngit-tree-sha1 = \"a\"\n\n[\"1.6.0-DEV\"]\ngit-tree-sha1 = \"b\"\n",
            "J/JSON" => "[\"0.21.4\"]\ngit-tree-sha1 = \"a\"\n",
            _ => return Err(format!("unexpected lookup {name}")),
        }
        .to_owned())
    }

    fn run(only: &[String], prereleases: bool) -> Findings {
        outdated(
            "julia",
            &[PROJECT.to_owned()],
            &[MANIFEST.to_owned()],
            only,
            prereleases,
            DEFAULT_BASE,
            &registry,
        )
    }

    #[test]
    fn lists_outdated_direct_packages_and_skips_yanked_and_prerelease() {
        let found = run(&[], false);
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(listed, [("OrderedCollections", "1.6.0", "1.8.1")]);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn prereleases_need_the_flag() {
        let only = ["Preferences".to_owned()];
        assert!(run(&only, false).candidates.is_empty());
        let found = run(&only, true);
        assert_eq!(found.candidates[0].latest, "1.6.0-DEV");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn a_failed_lookup_is_an_error_not_a_candidate() {
        let found = outdated(
            "julia",
            &[PROJECT.to_owned()],
            &[MANIFEST.to_owned()],
            &["JSON".to_owned()],
            false,
            DEFAULT_BASE,
            &|_: &str| Err("offline".to_owned()),
        );
        assert!(found.candidates.is_empty());
        assert_eq!(found.errors[0].package, "JSON");
    }
}
