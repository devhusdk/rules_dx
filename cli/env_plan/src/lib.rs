#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_bep::TargetOutput;
use dx_digest::blake3 as digest;
use dx_roots::{
    build_argv, invocation_targets, repository_plan, resolve_exact_target, ExactScopeError,
    RepositoryRootPlan,
};
use env_shard::{decode_validated, Error};
use serde::Serialize;

pub const OUTPUT_GROUP: &str = "dx_env_plans";

pub const SHARD_SUFFIX: &str = ".dxenv.pb";

pub const ENV_ASPECT: &str = "//env:plan.bzl%dx_env_plan_aspect";

pub const REPOSITORY_TARGET: &str = "//dx:env";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvScope {
    Repository,
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("expected at most one target, found {count}")]
    MultipleTargets { count: usize },
    #[error("invalid target {value:?}: patterns never select env")]
    TargetPattern { value: String },
    #[error("invalid target {value:?}: want an exact // or @ label")]
    NotTargetLabel { value: String },
}

pub fn resolve_scope(targets: &[String]) -> Result<EnvScope, ScopeError> {
    match resolve_exact_target(targets) {
        Ok(None) => Ok(EnvScope::Repository),
        Ok(Some(label)) => Ok(EnvScope::Exact(label)),
        Err(ExactScopeError::MultipleTargets { count }) => {
            Err(ScopeError::MultipleTargets { count })
        }
        Err(ExactScopeError::TargetPattern { value }) => Err(ScopeError::TargetPattern { value }),
        Err(ExactScopeError::NotTargetLabel { value }) => Err(ScopeError::NotTargetLabel { value }),
    }
}

pub fn scope_targets(scope: &EnvScope) -> Vec<String> {
    match scope {
        EnvScope::Repository => targets_for_root_plan(&repository_plan()),
        EnvScope::Exact(label) => vec![label.clone()],
    }
}

pub fn targets_for_root_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    invocation_targets(plan, REPOSITORY_TARGET)
}

pub fn build_argv_for_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    build_argv(
        plan,
        REPOSITORY_TARGET,
        &[ENV_ASPECT.to_owned()],
        &[OUTPUT_GROUP.to_owned()],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
    pub exec_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRecord {
    pub producer: String,
    pub integration: String,
    pub entries: Vec<EnvEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollectError {
    #[error("invalid shard {path:?}: {error}")]
    Shard { path: String, error: Error },
    #[error("{0}")]
    Conflict(String),
    #[error("missing artifact for {producer} {key:?}: no BEP artifact matches {exec_path:?}")]
    MissingArtifact {
        producer: String,
        key: String,
        exec_path: String,
    },
    #[error("ambiguous artifact {exec_path:?}: matches {artifacts:?}")]
    AmbiguousArtifact {
        exec_path: String,
        artifacts: Vec<String>,
    },
    #[error("unreported artifact {path:?}: claimed by no entry")]
    UnreportedArtifact { path: String },
    #[error("{0}")]
    Fingerprint(#[from] dx_fingerprint::FingerprintError),
}

pub fn is_shard_artifact(path: &Path) -> bool {
    dx_bep::is_shard_artifact(path, SHARD_SUFFIX)
}

pub fn exec_matches(artifact_path: &Path, exec_path: &str) -> bool {
    dx_bep::exec_matches(artifact_path, exec_path)
}

pub fn collect_shards(outputs: &[TargetOutput]) -> Result<Vec<EnvRecord>, CollectError> {
    let backing_paths = backing_artifact_paths(outputs);
    let mut records = Vec::new();
    for output in outputs {
        for artifact in &output.artifacts {
            if !is_shard_artifact(&artifact.exec_path) {
                continue;
            }
            let shard = decode_validated(&artifact.bytes).map_err(|error| CollectError::Shard {
                path: artifact.exec_path.display().to_string(),
                error,
            })?;
            records.push(EnvRecord {
                producer: shard.producer,
                integration: shard.integration,
                entries: shard
                    .entries
                    .into_iter()
                    .map(|entry| EnvEntry {
                        key: entry.key,
                        value: entry.value,
                        exec_path: entry.exec_path,
                    })
                    .collect(),
            });
        }
    }
    let _ = index_artifacts(&records, &backing_paths)?;
    Ok(records)
}

fn backing_artifact_paths(outputs: &[TargetOutput]) -> Vec<String> {
    dx_bep::non_shard_artifact_paths(outputs, SHARD_SUFFIX)
}

fn index_artifacts<'a>(
    records: &'a [EnvRecord],
    backing_paths: &[String],
) -> Result<BTreeMap<(&'a str, &'a str), String>, CollectError> {
    let bound: Vec<(&EnvRecord, &EnvEntry)> = records
        .iter()
        .flat_map(|record| record.entries.iter().map(move |entry| (record, entry)))
        .filter(|(_, entry)| !entry.exec_path.is_empty())
        .collect();
    let mut resolved: BTreeMap<(&str, &str), String> = BTreeMap::new();
    for (record, entry) in &bound {
        let matches: Vec<&String> = backing_paths
            .iter()
            .filter(|path| dx_bep::suffix_matches(path, &entry.exec_path))
            .collect();
        if matches.is_empty() {
            return Err(CollectError::MissingArtifact {
                producer: record.producer.clone(),
                key: entry.key.clone(),
                exec_path: entry.exec_path.clone(),
            });
        }
        if matches.len() > 1 {
            return Err(CollectError::AmbiguousArtifact {
                exec_path: entry.exec_path.clone(),
                artifacts: matches.into_iter().map(|path| (*path).clone()).collect(),
            });
        }
        resolved.insert(
            (record.producer.as_str(), entry.key.as_str()),
            (*matches[0]).clone(),
        );
    }
    for path in backing_paths {
        if !bound
            .iter()
            .any(|(_, entry)| dx_bep::suffix_matches(path, &entry.exec_path))
        {
            return Err(CollectError::UnreportedArtifact { path: path.clone() });
        }
    }
    Ok(resolved)
}

type MergeIndex<'a> =
    BTreeMap<(&'a str, &'a str), BTreeMap<(&'a str, &'a str, &'a str), &'a EnvEntry>>;

pub fn merge_records(records: &[EnvRecord]) -> Vec<EnvRecord> {
    let mut owners: MergeIndex<'_> = BTreeMap::new();
    for record in records {
        let owned = owners
            .entry((record.producer.as_str(), record.integration.as_str()))
            .or_default();
        for entry in &record.entries {
            owned
                .entry((
                    entry.key.as_str(),
                    entry.value.as_str(),
                    entry.exec_path.as_str(),
                ))
                .or_insert(entry);
        }
    }
    owners
        .into_iter()
        .map(|((producer, integration), entries)| EnvRecord {
            producer: producer.to_owned(),
            integration: integration.to_owned(),
            entries: entries.into_values().cloned().collect(),
        })
        .collect()
}

type Claim<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str);

pub fn conflict_error(records: &[EnvRecord]) -> String {
    let mut by_key: BTreeMap<&str, BTreeSet<Claim<'_>>> = BTreeMap::new();
    for record in records {
        for entry in &record.entries {
            by_key.entry(entry.key.as_str()).or_default().insert((
                record.producer.as_str(),
                record.integration.as_str(),
                entry.key.as_str(),
                entry.value.as_str(),
                entry.exec_path.as_str(),
            ));
        }
    }
    let mut conflicts = Vec::new();
    for (key, claims) in &by_key {
        if claims.len() > 1 {
            let claimants: Vec<&str> = claims.iter().map(|claim| claim.0).collect();
            conflicts.push(format!(
                "identity key '{key}' claimed by {}",
                claimants.join(", ")
            ));
        }
    }
    if conflicts.is_empty() {
        String::new()
    } else {
        format!("env plan conflict: {}", conflicts.join("; "))
    }
}

#[derive(Serialize)]
struct FingerprintEntry<'a> {
    exec_path: &'a str,
    key: &'a str,
    value: &'a str,
}

#[derive(Serialize)]
struct FingerprintRecord<'a> {
    entries: Vec<FingerprintEntry<'a>>,
    integration: &'a str,
    producer: &'a str,
}

pub fn fingerprint(records: &[EnvRecord]) -> Result<String, dx_fingerprint::FingerprintError> {
    let merged = merge_records(records);
    let view: Vec<FingerprintRecord<'_>> = merged
        .iter()
        .map(|record| FingerprintRecord {
            entries: record
                .entries
                .iter()
                .map(|entry| FingerprintEntry {
                    exec_path: entry.exec_path.as_str(),
                    key: entry.key.as_str(),
                    value: entry.value.as_str(),
                })
                .collect(),
            integration: record.integration.as_str(),
            producer: record.producer.as_str(),
        })
        .collect();
    dx_fingerprint::to_json(&view)
}

pub fn plan_digest(fingerprint: &str) -> [u8; 32] {
    digest(fingerprint.as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedPlan {
    pub records: Vec<EnvRecord>,
    pub fingerprint: String,
    pub digest: [u8; 32],
    pub projection: Vec<ProjectionEntry>,
}

pub fn collect_plan(outputs: &[TargetOutput]) -> Result<CollectedPlan, CollectError> {
    let records = collect_shards(outputs)?;
    let conflict = conflict_error(&records);
    if !conflict.is_empty() {
        return Err(CollectError::Conflict(conflict));
    }
    let merged = merge_records(&records);
    let rendered = fingerprint(&merged)?;
    let digest = digest(rendered.as_bytes());
    let projection = plan_projection(&merged, outputs)?;
    Ok(CollectedPlan {
        records: merged,
        fingerprint: rendered,
        digest,
        projection,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionEntry {
    pub key: String,
    pub value: String,
    pub artifact: String,
}

fn plan_projection(
    records: &[EnvRecord],
    outputs: &[TargetOutput],
) -> Result<Vec<ProjectionEntry>, CollectError> {
    let backing_paths = backing_artifact_paths(outputs);
    let resolved = index_artifacts(records, &backing_paths)?;
    let mut projection: Vec<ProjectionEntry> = Vec::new();
    for record in records {
        for entry in &record.entries {
            if entry.exec_path.is_empty() {
                continue;
            }
            let artifact = resolved
                .get(&(record.producer.as_str(), entry.key.as_str()))
                .cloned()
                .unwrap_or_else(|| entry.exec_path.clone());
            projection.push(ProjectionEntry {
                key: entry.key.clone(),
                value: entry.value.clone(),
                artifact,
            });
        }
    }
    projection.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(projection)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
