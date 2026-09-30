use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdvisorySnapshot {
    pub set: String,
    pub url: String,
    pub sha256: String,
    pub retrieved_at: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SnapshotProblem {
    #[error("advisory snapshot missing {field}")]
    MissingField { field: &'static str },
    #[error("advisory snapshot has non-https URL {url:?}")]
    BadUrl { url: String },
    #[error("advisory snapshot has invalid sha256 {value:?}; want 64 lowercase hex")]
    BadDigest { value: String },
    #[error("advisory snapshot has invalid retrieved_at {value:?}; want YYYY-MM-DD")]
    BadDate { value: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Freshness {
    Fresh,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshOutcome {
    Fresh,
    Refreshed { snapshot: AdvisorySnapshot },
    Failed { detail: String },
}

pub const CODE_ADVISORY_REFRESH_FAILED: &str = "advisory_refresh_failed";

pub const CACHE_DAYS: u32 = 0;

pub fn advisory_family(set: &str) -> Option<&'static str> {
    match set {
        "cargo" => Some("cargo"),
        "npm" | "npm-adopt" | "npm-adopt-polyglot" | "npm-tools" => Some("npm"),
        "maven" => Some("maven"),
        "nuget" => Some("nuget"),
        "powershell" => Some("nuget"),
        "go" => Some("go"),
        "ruby" => Some("rubygems"),
        _ => None,
    }
}

pub fn advisory_source(set: &str) -> Option<&'static str> {
    match family_of(set) {
        "cargo" => Some("https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip"),
        "npm" => Some("https://osv-vulnerabilities.storage.googleapis.com/npm/all.zip"),
        "maven" => Some("https://osv-vulnerabilities.storage.googleapis.com/Maven/all.zip"),
        "nuget" => Some("https://osv-vulnerabilities.storage.googleapis.com/NuGet/all.zip"),
        "go" => Some("https://osv-vulnerabilities.storage.googleapis.com/Go/all.zip"),
        "rubygems" => Some("https://osv-vulnerabilities.storage.googleapis.com/RubyGems/all.zip"),
        _ => None,
    }
}

pub fn is_local_mirror(snapshot: &AdvisorySnapshot) -> bool {
    snapshot.url.starts_with("file://")
}

pub fn is_accepted_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("file://")) && url::Url::parse(url).is_ok()
}

pub fn snapshot_rel(set: &str) -> String {
    format!(".dx/advisory/{}.json", family_of(set))
}

pub fn identity_rel(set: &str) -> String {
    format!(".dx/advisory/{}.meta.json", family_of(set))
}

fn family_of(set: &str) -> &str {
    advisory_family(set).unwrap_or(set)
}

pub fn parse_identity(text: &str) -> Result<AdvisorySnapshot, String> {
    serde_json::from_str(text).map_err(|error| format!("invalid advisory identity: {error}"))
}

pub fn identity_matches_bytes(snapshot: &AdvisorySnapshot, bytes: &[u8]) -> bool {
    dx_digest::sha256_hex(bytes) == snapshot.sha256.trim()
}

pub fn validate_snapshot(snapshot: &AdvisorySnapshot) -> Result<(), SnapshotProblem> {
    for (field, value) in [
        ("set", snapshot.set.as_str()),
        ("url", snapshot.url.as_str()),
        ("sha256", snapshot.sha256.as_str()),
        ("retrieved_at", snapshot.retrieved_at.as_str()),
        ("path", snapshot.path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(SnapshotProblem::MissingField { field });
        }
    }
    if !is_accepted_url(&snapshot.url) {
        return Err(SnapshotProblem::BadUrl {
            url: snapshot.url.clone(),
        });
    }
    if !dx_digest::is_hex(&snapshot.sha256) {
        return Err(SnapshotProblem::BadDigest {
            value: snapshot.sha256.clone(),
        });
    }
    if !is_audit_date(&snapshot.retrieved_at) {
        return Err(SnapshotProblem::BadDate {
            value: snapshot.retrieved_at.clone(),
        });
    }
    Ok(())
}

pub fn freshness(snapshot: &AdvisorySnapshot, today: &str) -> Freshness {
    if snapshot.retrieved_at.trim() == today.trim() && is_audit_date(today) {
        Freshness::Fresh
    } else {
        Freshness::Stale
    }
}

pub fn map_refresh(
    snapshot: &AdvisorySnapshot,
    today: &str,
    refreshed: Option<AdvisorySnapshot>,
    refresh_error: Option<String>,
) -> RefreshOutcome {
    if freshness(snapshot, today) == Freshness::Fresh {
        return RefreshOutcome::Fresh;
    }
    if let Some(next) = refreshed {
        if validate_snapshot(&next).is_ok() {
            return RefreshOutcome::Refreshed { snapshot: next };
        }
    }
    RefreshOutcome::Failed {
        detail: refresh_error.unwrap_or_else(|| {
            format!(
                "could not obtain current advisory data for {} (stale snapshot {})",
                snapshot.set, snapshot.retrieved_at
            )
        }),
    }
}

pub fn may_analyze(outcome: &RefreshOutcome) -> bool {
    matches!(
        outcome,
        RefreshOutcome::Fresh | RefreshOutcome::Refreshed { .. }
    )
}

fn is_audit_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> AdvisorySnapshot {
        AdvisorySnapshot {
            set: "cargo".to_owned(),
            url: "https://osv.dev/snapshots/cargo-2026-09-18.json".to_owned(),
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
            retrieved_at: "2026-09-18".to_owned(),
            path: ".dx/advisory/cargo-2026-09-18.json".to_owned(),
        }
    }

    #[test]
    fn valid_snapshot_passes() {
        validate_snapshot(&snapshot()).expect("valid");
    }

    #[test]
    fn empty_fields_fail() {
        let mut bad = snapshot();
        bad.set = "  ".to_owned();
        assert_eq!(
            validate_snapshot(&bad),
            Err(SnapshotProblem::MissingField { field: "set" })
        );
        let mut bad = snapshot();
        bad.path = String::new();
        assert!(validate_snapshot(&bad).is_err());
    }

    #[test]
    fn non_https_url_fails() {
        let mut bad = snapshot();
        bad.url = "http://osv.dev/snapshot.json".to_owned();
        assert_eq!(
            validate_snapshot(&bad),
            Err(SnapshotProblem::BadUrl {
                url: "http://osv.dev/snapshot.json".to_owned()
            })
        );
    }

    #[test]
    fn vendored_file_mirror_validates_like_upstream() {
        let mut mirror = snapshot();
        mirror.url = "file:///opt/dx-offline/advisory/cargo.json".to_owned();
        validate_snapshot(&mirror).expect("vendored file:// mirror validates");
        assert!(is_local_mirror(&mirror));
        assert!(!is_local_mirror(&snapshot()));
        assert!(is_accepted_url(&mirror.url));
        assert!(is_accepted_url(&snapshot().url));
        assert!(!is_accepted_url("http://osv.dev/snapshot.json"));
        assert!(!is_accepted_url(""));
        assert_eq!(freshness(&mirror, "2026-09-18"), Freshness::Fresh);
        assert_eq!(freshness(&mirror, "2026-09-19"), Freshness::Stale);
        let bytes = b"[]";
        let mut bound = mirror.clone();
        bound.sha256 = dx_digest::sha256_hex(bytes);
        assert!(identity_matches_bytes(&bound, bytes));
        assert!(!identity_matches_bytes(&bound, b"tampered"));
    }

    #[test]
    fn malformed_digests_fail() {
        for digest in [
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
            "not-a-digest",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85",
        ] {
            let mut bad = snapshot();
            bad.sha256 = digest.to_owned();
            assert!(
                matches!(
                    validate_snapshot(&bad),
                    Err(SnapshotProblem::BadDigest { .. })
                ),
                "{digest} must fail"
            );
        }
    }

    #[test]
    fn malformed_dates_fail() {
        for bad_date in [
            "2026-9-18",
            "2026/09/18",
            "2026-13-01",
            "2026-0X-18",
            "not-a-date",
        ] {
            let mut bad = snapshot();
            bad.retrieved_at = bad_date.to_owned();
            assert_eq!(
                validate_snapshot(&bad),
                Err(SnapshotProblem::BadDate {
                    value: bad_date.to_owned()
                }),
                "{bad_date} must fail"
            );
        }
    }

    #[test]
    fn freshness_is_same_day_only() {
        let snap = snapshot();
        assert_eq!(freshness(&snap, "2026-09-18"), Freshness::Fresh);
        assert_eq!(freshness(&snap, "2026-09-19"), Freshness::Stale);
        assert_eq!(freshness(&snap, "2026-09-17"), Freshness::Stale);
        assert_eq!(freshness(&snap, "not-a-date"), Freshness::Stale);
    }

    #[test]
    fn fresh_snapshot_proceeds_without_refresh() {
        let snap = snapshot();
        assert_eq!(
            map_refresh(&snap, "2026-09-18", None, None),
            RefreshOutcome::Fresh
        );
        assert!(may_analyze(&RefreshOutcome::Fresh));
    }

    #[test]
    fn stale_refresh_success_proceeds_against_new_identity() {
        let snap = snapshot();
        let next = AdvisorySnapshot {
            retrieved_at: "2026-09-19".to_owned(),
            ..snapshot()
        };
        let outcome = map_refresh(&snap, "2026-09-19", Some(next.clone()), None);
        assert_eq!(outcome, RefreshOutcome::Refreshed { snapshot: next });
        assert!(may_analyze(&outcome));
    }

    #[test]
    fn stale_refresh_failure_fails_without_stale_fallback() {
        let snap = snapshot();
        let outcome = map_refresh(
            &snap,
            "2026-09-19",
            None,
            Some("network unreachable".to_owned()),
        );
        assert_eq!(
            outcome,
            RefreshOutcome::Failed {
                detail: "network unreachable".to_owned()
            }
        );
        assert!(!may_analyze(&outcome));
        let defaulted = map_refresh(&snap, "2026-09-19", None, None);
        match &defaulted {
            RefreshOutcome::Failed { detail } => {
                assert!(detail.contains("cargo"));
                assert!(!detail.contains("clean"));
            }
            _ => panic!("must fail"),
        }
        assert!(!may_analyze(&defaulted));
    }

    #[test]
    fn stale_refresh_with_invalid_identity_fails() {
        let snap = snapshot();
        let mut bad = snapshot();
        bad.sha256 = "bad".to_owned();
        let outcome = map_refresh(&snap, "2026-09-19", Some(bad), None);
        assert!(matches!(outcome, RefreshOutcome::Failed { .. }));
        assert!(!may_analyze(&outcome));
    }

    #[test]
    fn cache_window_and_code_are_pinned() {
        assert_eq!(CACHE_DAYS, 0);
        assert_eq!(CODE_ADVISORY_REFRESH_FAILED, "advisory_refresh_failed");
    }

    #[test]
    fn advisory_sources_are_https_database_downloads_without_inventory() {
        for (set, ecosystem) in [
            ("cargo", "crates.io"),
            ("npm", "npm"),
            ("maven", "Maven"),
            ("nuget", "NuGet"),
            ("go", "Go"),
            ("ruby", "RubyGems"),
        ] {
            let url = advisory_source(set).unwrap_or_else(|| panic!("{set} needs a source"));
            assert!(url.starts_with("https://"), "{set} must be https");
            assert!(
                url.contains(ecosystem) && url.ends_with("/all.zip"),
                "{set} must name its OSV ecosystem bucket"
            );
            assert!(!url.contains('?'), "{set} must carry no query");
            assert!(
                !url.contains("api.osv.dev"),
                "{set} must not use the query API"
            );
        }
        assert_eq!(advisory_source("unknown-set"), None);
        assert_eq!(advisory_source(""), None);
    }

    #[test]
    fn every_npm_set_shares_the_npm_snapshot() {
        for set in ["npm", "npm-adopt", "npm-adopt-polyglot", "npm-tools"] {
            assert_eq!(advisory_family(set), Some("npm"), "{set} is npm");
            assert_eq!(snapshot_rel(set), ".dx/advisory/npm.json", "{set}");
            assert_eq!(identity_rel(set), ".dx/advisory/npm.meta.json", "{set}");
        }
        assert_eq!(advisory_family("cargo"), Some("cargo"));
        assert_eq!(advisory_family("ruby"), Some("rubygems"));
        assert_eq!(snapshot_rel("ruby"), ".dx/advisory/rubygems.json");
        assert_eq!(advisory_family("powershell"), Some("nuget"));
        assert_eq!(snapshot_rel("powershell"), ".dx/advisory/nuget.json");
        assert_eq!(advisory_family("uv"), None);
        assert_eq!(advisory_family("unknown-set"), None);
    }

    #[test]
    fn snapshot_and_identity_rels_are_pinned() {
        assert_eq!(snapshot_rel("cargo"), ".dx/advisory/cargo.json");
        assert_eq!(identity_rel("cargo"), ".dx/advisory/cargo.meta.json");
        assert_eq!(snapshot_rel("go"), ".dx/advisory/go.json");
        assert_eq!(identity_rel("go"), ".dx/advisory/go.meta.json");
    }

    #[test]
    fn identity_round_trips_and_rejects_malformed() {
        let snap = snapshot();
        let text = serde_json::to_string(&snap).expect("serialize");
        assert_eq!(parse_identity(&text).expect("parse"), snap);
        assert!(parse_identity("not json").is_err());
        assert!(parse_identity("{}").is_err());
    }

    #[test]
    fn identity_must_match_bytes() {
        let bytes = b"[]";
        let digest = dx_digest::sha256_hex(bytes);
        let mut snap = snapshot();
        snap.sha256 = digest.clone();
        assert!(identity_matches_bytes(&snap, bytes));
        snap.sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned();
        assert!(!identity_matches_bytes(&snap, b"tampered"));
    }
}
