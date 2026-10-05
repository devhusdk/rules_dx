use serde::{Deserialize, Serialize};
use std::path::Path;

use super::advisory::{self, AdvisorySnapshot};
use super::backend::{is_empty_set, vuln_locks};

pub const PREP_CODE: &str = "advisory_prepare_failed";

pub const CURL_PROGRAM: &str = "curl";

pub const DEFAULT_CONNECT_TIMEOUT_SECONDS: u32 = 30;

pub const DEFAULT_MAX_SECONDS: u32 = 600;

pub const DEFAULT_RETRIES: u32 = 3;

pub const DEFAULT_RETRY_DELAY_SECONDS: u32 = 5;

/// One prepared snapshot, its canonical bytes and the sidecar that binds them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PreparedSnapshot {
    pub snapshot: AdvisorySnapshot,
    pub payload: Vec<u8>,
    pub advisories: usize,
}

/// The bounded limits one download may spend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchLimits {
    pub connect_timeout_seconds: u32,
    pub max_seconds: u32,
    pub retries: u32,
    pub retry_delay_seconds: u32,
}

pub const DEFAULT_LIMITS: FetchLimits = FetchLimits {
    connect_timeout_seconds: DEFAULT_CONNECT_TIMEOUT_SECONDS,
    max_seconds: DEFAULT_MAX_SECONDS,
    retries: DEFAULT_RETRIES,
    retry_delay_seconds: DEFAULT_RETRY_DELAY_SECONDS,
};

/// One download, as the program and the arguments it fetches it with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchPlan {
    pub program: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PrepError {
    #[error("{PREP_CODE}: no advisory ecosystem covers set {set:?}")]
    UnsupportedSet { set: String },
    #[error("{PREP_CODE}: {family} archive holds no JSON entry")]
    NoJsonEntries { family: String },
    #[error("{PREP_CODE}: {family} archive holds no advisory")]
    NoAdvisories { family: String },
    #[error("{PREP_CODE}: {family} archive entry {name:?} is not an object or array: {detail}")]
    BadEntry {
        family: String,
        name: String,
        detail: String,
    },
    #[error("{PREP_CODE}: {family} archive entry {name:?} is not valid JSON: {detail}")]
    BadEntryJson {
        family: String,
        name: String,
        detail: String,
    },
    #[error("{PREP_CODE}: {retrieved_at:?} is not a YYYY-MM-DD date")]
    BadDate { retrieved_at: String },
    #[error("{PREP_CODE}: snapshot identity for {family} is unusable: {detail}")]
    BadIdentity { family: String, detail: String },
    #[error("{PREP_CODE}: {family} archive is unreadable: {detail}")]
    Archive { family: String, detail: String },
    #[error("{PREP_CODE}: could not read the {family} archive at {path}: {detail}")]
    ArchiveRead {
        family: String,
        path: String,
        detail: String,
    },
    #[error("{PREP_CODE}: could not write {path}: {detail}")]
    Write { path: String, detail: String },
    #[error("{PREP_CODE}: {program} could not fetch the snapshot: {detail}")]
    Fetch { program: String, detail: String },
    #[error("{PREP_CODE}: {url:?} is not an https or file URL")]
    BadUrl { url: String },
    #[error("{PREP_CODE}: fetch limits {limits:?} leave no room for one transfer")]
    BadLimits { limits: FetchLimits },
    #[error("{PREP_CODE}: {detail}")]
    Usage { detail: String },
}

/// Every dependency set the audit reads lockfiles for.
pub const AUDITED_SETS: &[&str] = &[
    "cargo",
    "npm",
    "npm-adopt",
    "npm-adopt-polyglot",
    "npm-tools",
    "maven",
    "nuget",
    "powershell",
    "go",
    "ruby",
];

/// The advisory families the dependency sets in one workspace need.
pub fn needed_families(workspace: &Path) -> Result<Vec<&'static str>, PrepError> {
    let mut families: Vec<&'static str> = Vec::new();
    for set in AUDITED_SETS {
        if is_empty_set(set) {
            continue;
        }
        let Some(family) = advisory::advisory_family(set) else {
            continue;
        };
        if families.contains(&family) || !lock_is_present(workspace, set) {
            continue;
        }
        families.push(family);
    }
    families.sort_unstable();
    Ok(families)
}

fn lock_is_present(workspace: &Path, set: &str) -> bool {
    vuln_locks(set)
        .iter()
        .any(|rel| workspace.join(rel).is_file())
}

/// Merges archive entries into one canonical snapshot and the sidecar that binds it.
pub fn convert_entries(
    family: &str,
    url: &str,
    retrieved_at: &str,
    entries: &[(String, Vec<u8>)],
) -> Result<PreparedSnapshot, PrepError> {
    if !advisory::is_audit_date(retrieved_at) {
        return Err(PrepError::BadDate {
            retrieved_at: retrieved_at.to_owned(),
        });
    }
    let mut ordered: Vec<&(String, Vec<u8>)> = entries.iter().collect();
    ordered.sort_by(|left, right| left.0.cmp(&right.0));
    if ordered.is_empty() {
        return Err(PrepError::NoJsonEntries {
            family: family.to_owned(),
        });
    }
    let mut advisories: Vec<serde_json::Value> = Vec::new();
    for (name, bytes) in ordered {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| PrepError::BadEntryJson {
                family: family.to_owned(),
                name: name.clone(),
                detail: error.to_string(),
            })?;
        match value {
            serde_json::Value::Array(items) => advisories.extend(items),
            serde_json::Value::Object(_) => advisories.push(value),
            other => {
                return Err(PrepError::BadEntry {
                    family: family.to_owned(),
                    name: name.clone(),
                    detail: format!("{other}"),
                });
            }
        }
    }
    if advisories.is_empty() {
        return Err(PrepError::NoAdvisories {
            family: family.to_owned(),
        });
    }
    let payload = serde_json::to_vec(&advisories).map_err(|error| PrepError::BadEntry {
        family: family.to_owned(),
        name: "<merged>".to_owned(),
        detail: error.to_string(),
    })?;
    let snapshot = AdvisorySnapshot {
        set: family.to_owned(),
        url: url.to_owned(),
        sha256: dx_digest::sha256_hex(&payload),
        retrieved_at: retrieved_at.to_owned(),
        path: advisory::snapshot_rel(family),
    };
    validate_prepared(&snapshot, &payload)?;
    Ok(PreparedSnapshot {
        snapshot,
        payload,
        advisories: advisories.len(),
    })
}

/// Checks one prepared snapshot the way the audit backend checks it later.
pub fn validate_prepared(snapshot: &AdvisorySnapshot, payload: &[u8]) -> Result<(), PrepError> {
    advisory::validate_snapshot(snapshot).map_err(|error| PrepError::BadIdentity {
        family: snapshot.set.clone(),
        detail: error.to_string(),
    })?;
    if !advisory::identity_matches_bytes(snapshot, payload) {
        return Err(PrepError::BadIdentity {
            family: snapshot.set.clone(),
            detail: format!("sha256 does not match {}", snapshot.path),
        });
    }
    Ok(())
}

/// The canonical sidecar bytes one snapshot identity serializes to.
pub fn identity_bytes(snapshot: &AdvisorySnapshot) -> Result<Vec<u8>, PrepError> {
    let mut bytes =
        serde_json::to_vec_pretty(snapshot).map_err(|error| PrepError::BadIdentity {
            family: snapshot.set.clone(),
            detail: error.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The bounded download that fetches one snapshot archive.
pub fn fetch_plan(
    url: &str,
    destination: &Path,
    limits: FetchLimits,
) -> Result<FetchPlan, PrepError> {
    if !advisory::is_accepted_url(url) {
        return Err(PrepError::BadUrl {
            url: url.to_owned(),
        });
    }
    if limits.connect_timeout_seconds == 0
        || limits.retry_delay_seconds == 0
        || limits.max_seconds <= limits.connect_timeout_seconds
    {
        return Err(PrepError::BadLimits { limits });
    }
    let mut argv = [
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https,file",
        "--proto-redir",
        "=https",
        "--connect-timeout",
    ]
    .map(str::to_owned)
    .to_vec();
    argv.push(limits.connect_timeout_seconds.to_string());
    argv.push("--max-time".to_owned());
    argv.push(limits.max_seconds.to_string());
    argv.push("--retry".to_owned());
    argv.push(limits.retries.to_string());
    argv.push("--retry-delay".to_owned());
    argv.push(limits.retry_delay_seconds.to_string());
    argv.push("--output".to_owned());
    argv.push(destination.display().to_string());
    argv.push(url.to_owned());
    Ok(FetchPlan {
        program: CURL_PROGRAM.to_owned(),
        argv,
    })
}
#[cfg(test)]
#[path = "advisory_prep_tests.rs"]
mod advisory_prep_tests;
