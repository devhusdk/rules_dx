use super::sets::BumpSet;
use super::version::{compare, is_stable};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub selector: String,
    pub set: BumpSet,
    pub current: semver::Version,
    pub available: Vec<semver::Version>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutdatedCandidate {
    pub selector: String,
    pub current: semver::Version,
    pub latest: semver::Version,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DiscoveryError {
    #[error("empty discovery entry; expected `set:package` plus semver")]
    Empty,
    #[error(
        "unknown discovery selector {selector:?}; expected {} as `set:package`",
        BumpSet::pipe_list()
    )]
    UnknownSelector { selector: String },
    #[error("invalid version {version:?} for selector {selector:?}: expected exact semver")]
    InvalidVersion { selector: String, version: String },
}

pub fn registry_client(set: BumpSet) -> &'static str {
    match set {
        BumpSet::Bazel => "BCR",
        BumpSet::Cargo => "crates.io",
        BumpSet::Npm => "npm registry",
        BumpSet::Go => "Go proxy",
        BumpSet::Maven => "Maven Central",
        BumpSet::NuGet => "NuGet",
        BumpSet::GithubActions => "GitHub releases",
    }
}

pub fn parse_declared(selector: &str, current: &str) -> Result<Snapshot, DiscoveryError> {
    if selector.is_empty() || current.is_empty() {
        return Err(DiscoveryError::Empty);
    }
    let (head, tail) = match selector.split_once(':') {
        Some((head, tail)) => (head, tail),
        None => {
            return Err(DiscoveryError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    let set = match BumpSet::parse(head) {
        Some(set) => set,
        None => {
            return Err(DiscoveryError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    if tail.is_empty() {
        return Err(DiscoveryError::UnknownSelector {
            selector: selector.to_owned(),
        });
    }
    if set == BumpSet::GithubActions {
        return Err(DiscoveryError::UnknownSelector {
            selector: selector.to_owned(),
        });
    }
    if set != BumpSet::Maven && tail.contains(':') {
        return Err(DiscoveryError::UnknownSelector {
            selector: selector.to_owned(),
        });
    }
    let parsed: semver::Version = current
        .trim()
        .strip_prefix('v')
        .unwrap_or(current.trim())
        .parse()
        .map_err(|_| DiscoveryError::InvalidVersion {
            selector: selector.to_owned(),
            version: current.to_owned(),
        })?;
    Ok(Snapshot {
        selector: selector.to_owned(),
        set,
        current: parsed,
        available: Vec::new(),
    })
}

pub fn latest_stable(
    current: &semver::Version,
    available: &[semver::Version],
) -> Option<semver::Version> {
    let mut best: Option<&semver::Version> = None;
    for candidate in available {
        if !is_stable(candidate) {
            continue;
        }
        if compare(current, candidate) != std::cmp::Ordering::Less {
            continue;
        }
        match best {
            None => best = Some(candidate),
            Some(winner) => {
                if compare(winner, candidate) == std::cmp::Ordering::Less {
                    best = Some(candidate);
                }
            }
        }
    }
    best.cloned()
}

pub fn outdated_from_snapshot(snapshot: &Snapshot) -> Option<OutdatedCandidate> {
    latest_stable(&snapshot.current, &snapshot.available).map(|latest| OutdatedCandidate {
        selector: snapshot.selector.clone(),
        current: snapshot.current.clone(),
        latest,
    })
}

pub fn collect_outdated(snapshots: &[Snapshot]) -> Vec<OutdatedCandidate> {
    let mut out: Vec<OutdatedCandidate> = snapshots
        .iter()
        .filter_map(outdated_from_snapshot)
        .collect();
    out.sort_by(|left, right| {
        left.selector
            .cmp(&right.selector)
            .then_with(|| compare(&left.latest, &right.latest))
    });
    out
}

pub fn next_outdated(candidates: &[OutdatedCandidate]) -> Option<&OutdatedCandidate> {
    candidates.first()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(selector: &str, current: &str, available: &[&str]) -> Snapshot {
        let mut entry = parse_declared(selector, current).expect("declared");
        entry.available = available
            .iter()
            .map(|text| text.parse::<semver::Version>().expect("available"))
            .collect();
        entry
    }

    #[test]
    fn semver_sets_parse_declared_with_client_mapping() {
        for (selector, client) in [
            ("bazel:rules_rust", "BCR"),
            ("cargo:anyhow", "crates.io"),
            ("npm:jest", "npm registry"),
            ("go:example.com/mod", "Go proxy"),
            ("maven:junit:junit", "Maven Central"),
            ("nuget:FSharp.Core", "NuGet"),
        ] {
            let entry = parse_declared(selector, "1.2.3").expect("declared");
            assert_eq!(registry_client(entry.set), client, "{selector}");
        }
        assert_eq!(registry_client(BumpSet::GithubActions), "GitHub releases");
    }

    #[test]
    fn github_actions_has_no_semver_current() {
        assert!(matches!(
            parse_declared("github-actions:actions/checkout", "v4"),
            Err(DiscoveryError::UnknownSelector { .. })
        ));
    }

    #[test]
    fn malformed_entries_fail_closed() {
        assert!(matches!(
            parse_declared("", "1.2.3"),
            Err(DiscoveryError::Empty)
        ));
        assert!(matches!(
            parse_declared("cargo:anyhow", ""),
            Err(DiscoveryError::Empty)
        ));
        assert!(matches!(
            parse_declared("cargo", "1.2.3"),
            Err(DiscoveryError::UnknownSelector { .. })
        ));
        assert!(matches!(
            parse_declared("bogus:anyhow", "1.2.3"),
            Err(DiscoveryError::UnknownSelector { .. })
        ));
        assert!(matches!(
            parse_declared("cargo:anyhow", "not-a-version"),
            Err(DiscoveryError::InvalidVersion { .. })
        ));
    }

    #[test]
    fn latest_stable_proposes_max_above_current() {
        let current: semver::Version = "1.2.3".parse().expect("current");
        let available: Vec<semver::Version> = ["1.2.4", "1.10.0", "1.2.3"]
            .iter()
            .map(|text| text.parse().expect("available"))
            .collect();
        assert_eq!(
            latest_stable(&current, &available)
                .expect("latest")
                .to_string(),
            "1.10.0"
        );
    }

    #[test]
    fn stable_only_prerelease_never_wins() {
        let current: semver::Version = "1.2.3".parse().expect("current");
        let available: Vec<semver::Version> = ["1.2.4-alpha.1", "2.0.0-beta.1"]
            .iter()
            .map(|text| text.parse().expect("available"))
            .collect();
        assert_eq!(latest_stable(&current, &available), None);
        let mixed: Vec<semver::Version> = ["1.2.4-alpha.1", "1.2.4"]
            .iter()
            .map(|text| text.parse().expect("available"))
            .collect();
        assert_eq!(
            latest_stable(&current, &mixed).expect("stable").to_string(),
            "1.2.4"
        );
    }

    #[test]
    fn up_to_date_has_no_candidate() {
        let entry = snapshot("cargo:anyhow", "1.10.0", &["1.2.3", "1.10.0"]);
        assert_eq!(outdated_from_snapshot(&entry), None);
        let entry = snapshot("npm:jest", "30.3.0", &["30.2.0", "30.3.0"]);
        assert_eq!(outdated_from_snapshot(&entry), None);
    }

    #[test]
    fn collect_orders_by_selector_never_batch() {
        let entries = vec![
            snapshot("npm:jest", "30.2.0", &["30.3.0"]),
            snapshot("cargo:anyhow", "1.0.0", &["1.2.3"]),
        ];
        let candidates = collect_outdated(&entries);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].selector, "cargo:anyhow");
        assert_eq!(candidates[1].selector, "npm:jest");
        let next = next_outdated(&candidates).expect("next");
        assert_eq!(next.selector, "cargo:anyhow");
        assert_eq!(next.latest.to_string(), "1.2.3");
    }

    #[test]
    fn maven_group_artifact_orders_with_semver() {
        let entry = snapshot("maven:junit:junit", "4.13.2", &["4.13.3", "5.0.0"]);
        let candidate = outdated_from_snapshot(&entry).expect("outdated");
        assert_eq!(candidate.latest.to_string(), "5.0.0");
    }

    #[test]
    fn serde_json_and_semver_stay_upstream_owned() {
        let payload: serde_json::Value =
            serde_json::from_str(r#"{"name":"jest","version":"30.3.0"}"#).expect("json");
        assert_eq!(payload["version"], serde_json::json!("30.3.0"));
        let low: semver::Version = "1.2.3".parse().expect("low");
        let high: semver::Version = "1.10.0".parse().expect("high");
        assert_eq!(compare(&low, &high), std::cmp::Ordering::Less);
        assert!(is_stable(&high));
    }
}
