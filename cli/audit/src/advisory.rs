use dx_error::DxError;
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

pub const CODE_ADVISORY_REFRESH_FAILED: &str = "advisory_refresh_failed";

/// Command that creates or refreshes the advisory snapshots.
pub const REFRESH_COMMAND: &str = "bazel run @rules_dx//cli/advisory_prep --";

/// Maps a dependency set name to its advisory family; the sole set-to-family step.
pub fn advisory_family(set: &str) -> Option<&'static str> {
    match set {
        "cargo" => Some("cargo"),
        "npm" | "npm-adopt" | "npm-adopt-polyglot" | "npm-tools" => Some("npm"),
        "maven" => Some("maven"),
        "nuget" => Some("nuget"),
        "go" => Some("go"),
        "ruby" | "rubygems" => Some("rubygems"),
        PYPI_FAMILY => Some(PYPI_FAMILY),
        "cpan" => Some("cpan"),
        _ => crate::vuln::VERSION_FAMILIES
            .iter()
            .copied()
            .find(|family| *family == set),
    }
}

/// Maps a dependency set name to the family whose yanked-version snapshot covers it.
pub fn yanked_family(set: &str) -> Option<&'static str> {
    match set {
        "cargo" => Some("cargo-yanked"),
        PYPI_FAMILY => Some("pypi-yanked"),
        _ => None,
    }
}

pub const PYPI_FAMILY: &str = "pypi";

/// The pin family of the trivy database and checks, one `.tar.gz` of the cache directory.
pub const TRIVY_FAMILY: &str = "trivy";

pub fn advisory_source(family: &str) -> Option<&'static str> {
    match family {
        "cargo" => Some("https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip"),
        "npm" => Some("https://osv-vulnerabilities.storage.googleapis.com/npm/all.zip"),
        "maven" => Some("https://osv-vulnerabilities.storage.googleapis.com/Maven/all.zip"),
        "nuget" => Some("https://osv-vulnerabilities.storage.googleapis.com/NuGet/all.zip"),
        "go" => Some("https://osv-vulnerabilities.storage.googleapis.com/Go/all.zip"),
        "rubygems" => Some("https://osv-vulnerabilities.storage.googleapis.com/RubyGems/all.zip"),
        "pypi" => Some("https://osv-vulnerabilities.storage.googleapis.com/PyPI/all.zip"),
        "hex" => Some("https://osv-vulnerabilities.storage.googleapis.com/Hex/all.zip"),
        "pub" => Some("https://osv-vulnerabilities.storage.googleapis.com/Pub/all.zip"),
        "packagist" => Some("https://osv-vulnerabilities.storage.googleapis.com/Packagist/all.zip"),
        "hackage" => Some("https://osv-vulnerabilities.storage.googleapis.com/Hackage/all.zip"),
        "swifturl" => Some("https://osv-vulnerabilities.storage.googleapis.com/SwiftURL/all.zip"),
        "julia" => Some("https://osv-vulnerabilities.storage.googleapis.com/Julia/all.zip"),
        "opam" => Some("https://osv-vulnerabilities.storage.googleapis.com/opam/all.zip"),
        "cran" => Some("https://osv-vulnerabilities.storage.googleapis.com/CRAN/all.zip"),
        "cpan" => Some(crate::advisory_prep::CPANSA_URL),
        _ => None,
    }
}

pub fn is_accepted_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("file://")) && url::Url::parse(url).is_ok()
}

pub const CODE_ADVISORY_PIN_MISSING: &str = "advisory_pin_missing";
pub const CODE_ADVISORY_PIN_MISMATCH: &str = "advisory_pin_mismatch";

/// Pins older than this many days draw a warning.
pub const PIN_MAX_AGE_DAYS: i64 = 14;

/// One `advisory <family> <YYYY-MM-DD> <sha256> <url>` line of `.dx/pins`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvisoryPin {
    pub family: String,
    pub date: String,
    pub sha256: String,
    pub url: String,
}

impl AdvisoryPin {
    /// The line this pin takes in `.dx/pins`.
    pub fn line(&self) -> String {
        format!(
            "advisory {} {} {} {}",
            self.family, self.date, self.sha256, self.url
        )
    }

    /// Where the verified snapshot is cached, relative to the workspace.
    pub fn cache_rel(&self) -> String {
        format!(".dx/local/advisory/{}/{}.json", self.sha256, self.family)
    }

    /// Where the unpacked trivy cache lives, relative to the workspace.
    pub fn trivy_cache_rel(&self) -> String {
        format!(".dx/local/advisory/{}/trivy", self.sha256)
    }

    /// Whole days from the pin date to `today`, when both are audit dates.
    pub fn age_days(&self, today: &str) -> Option<i64> {
        let parse = |value: &str| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok();
        if !is_audit_date(today) {
            return None;
        }
        Some((parse(today)? - parse(&self.date)?).num_days())
    }
}

/// The pin for one family; lines of any other shape are ignored.
pub fn find_pin(text: &str, family: &str) -> Option<AdvisoryPin> {
    text.lines().find_map(|line| {
        let fields: Vec<&str> = line.trim_end_matches('\r').split_whitespace().collect();
        let ["advisory", line_family, date, sha256, url] = fields[..] else {
            return None;
        };
        (line_family == family
            && is_audit_date(date)
            && dx_digest::is_hex(sha256)
            && is_accepted_url(url))
        .then(|| AdvisoryPin {
            family: family.to_owned(),
            date: date.to_owned(),
            sha256: sha256.to_owned(),
            url: url.to_owned(),
        })
    })
}

/// The local path a `file://` URL names.
pub fn file_url_path(url: &str) -> Option<std::path::PathBuf> {
    url::Url::parse(url).ok()?.to_file_path().ok()
}

/// The `file://` URL of one absolute local path.
pub fn file_url(path: &std::path::Path) -> Option<String> {
    url::Url::from_file_path(path).ok().map(String::from)
}

pub fn snapshot_rel(family: &str) -> String {
    format!(".dx/local/advisory/{family}.json")
}

pub fn identity_rel(family: &str) -> String {
    format!(".dx/local/advisory/{family}.meta.json")
}

pub fn parse_identity(text: &str) -> Result<AdvisorySnapshot, DxError> {
    serde_json::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid advisory identity: {error}")))
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

/// Whether one value is an audit date, `YYYY-MM-DD`.
pub fn is_audit_date(value: &str) -> bool {
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
            path: ".dx/local/advisory/cargo-2026-09-18.json".to_owned(),
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
    fn refresh_failure_code_is_pinned() {
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
            ("rubygems", "RubyGems"),
            ("pypi", "PyPI"),
            ("hex", "Hex"),
            ("pub", "Pub"),
            ("packagist", "Packagist"),
            ("hackage", "Hackage"),
            ("swifturl", "SwiftURL"),
            ("julia", "Julia"),
            ("opam", "opam"),
            ("cran", "CRAN"),
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
            let family = advisory_family(set).expect("family");
            assert_eq!(snapshot_rel(family), ".dx/local/advisory/npm.json", "{set}");
            assert_eq!(
                identity_rel(family),
                ".dx/local/advisory/npm.meta.json",
                "{set}"
            );
        }
        assert_eq!(advisory_family("cargo"), Some("cargo"));
        assert_eq!(advisory_family("ruby"), Some("rubygems"));
        for key in ["ruby", "rubygems"] {
            assert_eq!(advisory_family(key), Some("rubygems"), "{key}");
            let key = advisory_family(key).expect("family");
            assert_eq!(
                advisory_source(key),
                Some("https://osv-vulnerabilities.storage.googleapis.com/RubyGems/all.zip"),
                "{key}"
            );
            assert_eq!(
                snapshot_rel(key),
                ".dx/local/advisory/rubygems.json",
                "{key}"
            );
            assert_eq!(
                identity_rel(key),
                ".dx/local/advisory/rubygems.meta.json",
                "{key}"
            );
        }
        assert_eq!(advisory_family("powershell"), None);
        assert_eq!(advisory_source("powershell"), None);
        assert_eq!(
            advisory_source("ruby"),
            None,
            "sets must map to a family first"
        );
        assert_eq!(advisory_family("cpan"), Some("cpan"));
        assert_eq!(
            advisory_source("cpan"),
            Some("https://metacpan.org/release/CPANSA-DB")
        );
        assert_eq!(advisory_family("uv"), None);
        assert_eq!(yanked_family("pypi"), Some("pypi-yanked"));
        assert_eq!(yanked_family("npm"), None);
        assert_eq!(advisory_family("unknown-set"), None);
    }

    #[test]
    fn osv_only_sets_are_their_own_family() {
        for set in crate::vuln::VERSION_FAMILIES {
            assert_eq!(advisory_family(set), Some(*set));
            assert_eq!(snapshot_rel(set), format!(".dx/local/advisory/{set}.json"));
        }
    }

    #[test]
    fn snapshot_and_identity_rels_are_pinned() {
        assert_eq!(snapshot_rel("cargo"), ".dx/local/advisory/cargo.json");
        assert_eq!(identity_rel("cargo"), ".dx/local/advisory/cargo.meta.json");
        assert_eq!(snapshot_rel("go"), ".dx/local/advisory/go.json");
        assert_eq!(identity_rel("go"), ".dx/local/advisory/go.meta.json");
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

    #[test]
    fn pins_are_found_by_family_and_other_lines_are_ignored() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let text = format!(
            "0.0.0\n1.2.3 {sha} dx-linux\nadvisory npm 2026-10-01 {sha} https://example.test/npm.json\nadvisory cargo 2026-10-02 {sha} https://example.test/cargo.json\nadvisory cargo bad {sha} https://example.test/x\n"
        );
        let pin = find_pin(&text, "cargo").expect("cargo pin");
        assert_eq!(pin.date, "2026-10-02");
        assert_eq!(
            pin.line(),
            format!("advisory cargo 2026-10-02 {sha} https://example.test/cargo.json")
        );
        assert_eq!(
            pin.cache_rel(),
            format!(".dx/local/advisory/{sha}/cargo.json")
        );
        assert_eq!(pin.age_days("2026-10-16"), Some(14));
        assert_eq!(pin.age_days("nope"), None);
        assert!(find_pin(&text, "go").is_none());
    }
}
