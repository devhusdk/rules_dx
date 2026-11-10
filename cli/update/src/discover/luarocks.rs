//! Lists LuaRocks pins whose newest version in the luarocks.org manifest is newer than the locked one.

use dx_error::DxError;
use super::{Candidate, Fetch, Findings, LookupError, wanted};
use std::cmp::Ordering;

pub const DEFAULT_BASE: &str = "https://luarocks.org";

const MANIFEST: &str = "manifest-5.4.json";

/// Compares `(rock, version)` pins against the manifest at `base`.
pub fn outdated(
    set: &str,
    pins: &[(String, String)],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let wanted_pins: Vec<_> = pins.iter().filter(|(name, _)| wanted(only, name)).collect();
    if wanted_pins.is_empty() {
        return found;
    }
    let manifest = fetch.get(&format!("{base}/{MANIFEST}")).and_then(|body| {
        serde_json::from_str::<serde_json::Value>(&body).map_err(|e| DxError::failure(e.to_string()))
    });
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(reason) => {
            found.errors.push(LookupError {
                set: set.to_owned(),
                package: MANIFEST.to_owned(),
                reason: reason.to_string(),
            });
            return found;
        }
    };
    for (name, current) in wanted_pins {
        let Some(versions) = manifest["repository"][name.as_str()].as_object() else {
            found.errors.push(LookupError {
                set: set.to_owned(),
                package: name.clone(),
                reason: format!("{name} is not in {MANIFEST}"),
            });
            continue;
        };
        let newest = versions
            .keys()
            .filter(|version| prereleases || !is_prerelease(version))
            .max_by(|a, b| version_cmp(a, b));
        if let Some(latest) = newest
            && version_cmp(latest, current) == Ordering::Greater
        {
            found.candidates.push(Candidate {
                set: set.to_owned(),
                package: name.clone(),
                current: current.clone(),
                prerelease: is_prerelease(latest),
                latest: latest.clone(),
                published: None,
                repo: None,
                tags: None,
            });
        }
    }
    found
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Part {
    Delta(i64),
    Number(u64),
}

impl Part {
    fn rank(self) -> (i64, u64) {
        match self {
            Part::Delta(delta) => (delta, 0),
            Part::Number(number) => (0, number),
        }
    }
}

fn delta(word: &str) -> i64 {
    match word {
        "alpha" => -1_000_000,
        "beta" => -100_000,
        "pre" => -10_000,
        "rc" => -1_000,
        "dev" | "scm" | "cvs" | "git" | "svn" => -100,
        _ => -50,
    }
}

fn is_prerelease(version: &str) -> bool {
    parts(version)
        .0
        .iter()
        .any(|part| matches!(part, Part::Delta(_)))
}

/// Splits `1.2beta3-1` into its components and the revision, as LuaRocks does.
fn parts(version: &str) -> (Vec<Part>, u64) {
    let (core, revision) = match version.rsplit_once('-') {
        Some((core, revision)) if revision.chars().all(|c| c.is_ascii_digit()) => {
            (core, revision.parse().unwrap_or(0))
        }
        _ => (version, 0),
    };
    let mut out = Vec::new();
    let mut chars = core.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut digits = String::new();
            while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                digits.push(d);
                chars.next();
            }
            out.push(Part::Number(digits.parse().unwrap_or(0)));
        } else if c.is_ascii_alphabetic() {
            let mut word = String::new();
            while let Some(&a) = chars.peek().filter(|a| a.is_ascii_alphabetic()) {
                word.push(a.to_ascii_lowercase());
                chars.next();
            }
            out.push(Part::Delta(delta(&word)));
        } else {
            chars.next();
        }
    }
    (out, revision)
}

/// LuaRocks version ordering: components first, then the rockspec revision.
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    let ((parts_a, rev_a), (parts_b, rev_b)) = (parts(a), parts(b));
    for i in 0..parts_a.len().max(parts_b.len()) {
        let left = parts_a.get(i).copied().unwrap_or(Part::Number(0));
        let right = parts_b.get(i).copied().unwrap_or(Part::Number(0));
        let order = left.rank().cmp(&right.rank());
        if order.is_ne() {
            return order;
        }
    }
    rev_a.cmp(&rev_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_JSON: &str = r#"{"repository":{
        "inspect":{"3.1.3-0":[{"arch":"rockspec"}],"3.1.10-1":[{"arch":"rockspec"}],"3.1.9-2":[{"arch":"rockspec"}]},
        "penlight":{"1.13.1-1":[{"arch":"rockspec"}],"1.14.0-1":[{"arch":"rockspec"}],"1.15.0beta1-1":[{"arch":"rockspec"}],"scm-1":[{"arch":"rockspec"}]},
        "current":{"1.0-1":[{"arch":"rockspec"}]}}}"#;

    fn registry(url: &str) -> Result<String, DxError> {
        if url == "https://luarocks.org/manifest-5.4.json" {
            Ok(MANIFEST_JSON.to_owned())
        } else {
            Err(DxError::failure(format!("unexpected lookup {url}")))
        }
    }

    fn pins() -> Vec<(String, String)> {
        [
            ("inspect", "3.1.3-0"),
            ("penlight", "1.13.1-1"),
            ("current", "1.0-1"),
        ]
        .map(|(n, v)| (n.to_owned(), v.to_owned()))
        .to_vec()
    }

    fn listed(found: &Findings) -> Vec<(&str, &str, &str)> {
        found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect()
    }

    #[test]
    fn orders_versions_like_luarocks() {
        assert_eq!(version_cmp("3.1.10-1", "3.1.9-2"), Ordering::Greater);
        assert_eq!(version_cmp("1.0-2", "1.0-1"), Ordering::Greater);
        assert_eq!(version_cmp("1.0", "1.0.0-0"), Ordering::Equal);
        assert_eq!(version_cmp("1.0beta1-1", "1.0-1"), Ordering::Less);
        assert_eq!(version_cmp("1.0rc1-1", "1.0beta2-1"), Ordering::Greater);
        assert_eq!(version_cmp("scm-1", "1.0-1"), Ordering::Less);
    }

    #[test]
    fn lists_newer_stable_versions() {
        let found = outdated("lua", &pins(), &[], false, DEFAULT_BASE, &registry);
        assert_eq!(
            listed(&found),
            [
                ("inspect", "3.1.3-0", "3.1.10-1"),
                ("penlight", "1.13.1-1", "1.14.0-1")
            ]
        );
        assert!(found.errors.is_empty());
    }

    #[test]
    fn prereleases_need_the_flag() {
        let only = ["penlight".to_owned()];
        let found = outdated("lua", &pins(), &only, true, DEFAULT_BASE, &registry);
        assert_eq!(listed(&found), [("penlight", "1.13.1-1", "1.15.0beta1-1")]);
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn unknown_rock_and_failed_manifest_are_errors() {
        let missing = [("ghost".to_owned(), "1.0-1".to_owned())];
        let found = outdated("lua", &missing, &[], false, DEFAULT_BASE, &registry);
        assert_eq!(found.errors.len(), 1);
        let found = outdated("lua", &pins(), &[], false, "https://x", &registry);
        assert_eq!(found.errors.len(), 1);
        assert!(found.candidates.is_empty());
    }
}
