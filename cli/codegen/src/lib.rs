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

use codegen_shard::{decode_validated, Error};
use dx_bep::TargetOutput;
use dx_digest::blake3 as digest;
use dx_roots::{
    build_argv, invocation_targets, repository_plan, resolve_exact_target, ExactScopeError,
    RepositoryRootPlan,
};
use serde::Serialize;

pub const OUTPUT_GROUP: &str = "dx_codegen_plans";

pub const SHARD_SUFFIX: &str = ".dxcodegen.pb";

pub const CODEGEN_ASPECT: &str = "//generation:codegen.bzl%dx_codegen_plan_aspect";

pub const REPOSITORY_TARGET: &str = "//dx:codegen";

pub const EXPANSION_KIND_FILTER: &str = ".*codegen_shard rule";

pub fn expansion_expression(schema: &str) -> String {
    format!(
        "kind('{EXPANSION_KIND_FILTER}', rdeps(//..., set({})))",
        quote_label(schema)
    )
}

pub fn quote_label(label: &str) -> String {
    let mut quoted = String::with_capacity(label.len() + 2);
    quoted.push('"');
    for ch in label.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

pub fn expand_roots(schema: &str, projections: &[String]) -> Vec<String> {
    let mut roots: Vec<String> = Vec::with_capacity(1 + projections.len());
    roots.push(schema.to_owned());
    roots.extend(projections.iter().cloned());
    roots.sort();
    roots.dedup();
    roots
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenScope {
    Repository,
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("expected at most one target, found {count}")]
    MultipleTargets { count: usize },
    #[error("invalid target {value:?}: patterns never select codegen")]
    TargetPattern { value: String },
    #[error("invalid target {value:?}: want an exact // or @ label")]
    NotTargetLabel { value: String },
}

pub fn resolve_scope(targets: &[String]) -> Result<CodegenScope, ScopeError> {
    match resolve_exact_target(targets) {
        Ok(None) => Ok(CodegenScope::Repository),
        Ok(Some(label)) => Ok(CodegenScope::Exact(label)),
        Err(ExactScopeError::MultipleTargets { count }) => {
            Err(ScopeError::MultipleTargets { count })
        }
        Err(ExactScopeError::TargetPattern { value }) => Err(ScopeError::TargetPattern { value }),
        Err(ExactScopeError::NotTargetLabel { value }) => Err(ScopeError::NotTargetLabel { value }),
    }
}

pub fn scope_targets(scope: &CodegenScope) -> Vec<String> {
    match scope {
        CodegenScope::Repository => targets_for_root_plan(&repository_plan()),
        CodegenScope::Exact(label) => vec![label.clone()],
    }
}

pub fn targets_for_root_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    invocation_targets(plan, REPOSITORY_TARGET)
}

pub fn build_argv_for_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    build_argv(
        plan,
        REPOSITORY_TARGET,
        &[CODEGEN_ASPECT.to_owned()],
        &[OUTPUT_GROUP.to_owned()],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenEntry {
    pub logical_path: String,
    pub import_root: String,
    pub namespace: String,
    pub exec_path: String,
    pub replaces: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenRecord {
    pub producer: String,
    pub language: String,
    pub entries: Vec<CodegenEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollectError {
    #[error("invalid shard {path:?}: {error}")]
    Shard { path: String, error: Error },
    #[error("{0}")]
    Conflict(String),
    #[error(
        "missing artifact for {producer} {logical_path:?}: no BEP artifact matches {exec_path:?}"
    )]
    MissingArtifact {
        producer: String,
        logical_path: String,
        exec_path: String,
    },
    #[error("duplicate artifact {exec_path:?}: claimed by {claimants:?}")]
    DuplicateArtifact {
        exec_path: String,
        claimants: Vec<String>,
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

pub fn decode_shards(outputs: &[TargetOutput]) -> Result<Vec<CodegenRecord>, CollectError> {
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
            records.push(CodegenRecord {
                producer: shard.producer,
                language: shard.language,
                entries: shard
                    .entries
                    .into_iter()
                    .map(|entry| CodegenEntry {
                        logical_path: entry.logical_path,
                        import_root: entry.import_root,
                        namespace: entry.namespace,
                        exec_path: entry.exec_path,
                        replaces: entry.replaces,
                    })
                    .collect(),
            });
        }
    }
    Ok(records)
}

fn generated_artifact_paths(outputs: &[TargetOutput]) -> Vec<String> {
    dx_bep::non_shard_artifact_paths(outputs, SHARD_SUFFIX)
}

type Bound<'a> = [(&'a CodegenRecord, &'a CodegenEntry)];

fn bound_entries(records: &[CodegenRecord]) -> Vec<(&CodegenRecord, &CodegenEntry)> {
    records
        .iter()
        .flat_map(|record| record.entries.iter().map(move |entry| (record, entry)))
        .filter(|(_, entry)| !entry.exec_path.is_empty())
        .collect()
}

fn bind_artifacts<'p>(
    bound: &Bound<'_>,
    generated_paths: &'p [String],
) -> Result<Vec<&'p str>, CollectError> {
    let mut resolved: Vec<&str> = Vec::with_capacity(bound.len());
    for (record, entry) in bound {
        let matches: Vec<&String> = generated_paths
            .iter()
            .filter(|path| dx_bep::suffix_matches(path, &entry.exec_path))
            .collect();
        if matches.is_empty() {
            return Err(CollectError::MissingArtifact {
                producer: record.producer.clone(),
                logical_path: entry.logical_path.clone(),
                exec_path: entry.exec_path.clone(),
            });
        }
        if matches.len() > 1 {
            return Err(CollectError::DuplicateArtifact {
                exec_path: entry.exec_path.clone(),
                claimants: matches.into_iter().map(|path| (*path).clone()).collect(),
            });
        }
        resolved.push(matches[0]);
    }
    for path in generated_paths {
        let claimants: Vec<(&CodegenRecord, &CodegenEntry)> = bound
            .iter()
            .filter(|(_, entry)| dx_bep::suffix_matches(path, &entry.exec_path))
            .map(|(record, entry)| (*record, *entry))
            .collect();
        if claimants.is_empty() {
            return Err(CollectError::UnreportedArtifact { path: path.clone() });
        }
        if claimants.len() > 1 {
            let exec = claimants[0].1.exec_path.clone();
            let rendered: Vec<String> = claimants
                .iter()
                .map(|(record, entry)| format!("{}:{}", record.producer, entry.logical_path))
                .collect();
            return Err(CollectError::DuplicateArtifact {
                exec_path: exec,
                claimants: rendered,
            });
        }
    }
    Ok(resolved)
}

type MergeIndex<'a> = BTreeMap<
    (&'a str, &'a str),
    BTreeMap<(&'a str, &'a str, &'a str, &'a str, &'a str), &'a CodegenEntry>,
>;

pub fn merge_records(records: &[CodegenRecord]) -> Vec<CodegenRecord> {
    let mut owners: MergeIndex<'_> = BTreeMap::new();
    for record in records {
        let owned = owners
            .entry((record.producer.as_str(), record.language.as_str()))
            .or_default();
        for entry in &record.entries {
            owned
                .entry((
                    entry.logical_path.as_str(),
                    entry.import_root.as_str(),
                    entry.namespace.as_str(),
                    entry.exec_path.as_str(),
                    entry.replaces.as_str(),
                ))
                .or_insert(entry);
        }
    }
    owners
        .into_iter()
        .map(|((producer, language), entries)| CodegenRecord {
            producer: producer.to_owned(),
            language: language.to_owned(),
            entries: entries.into_values().cloned().collect(),
        })
        .collect()
}

pub fn conflict_error(records: &[CodegenRecord]) -> String {
    type Claim<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, &'a str);
    let mut by_path: BTreeMap<&str, BTreeSet<Claim<'_>>> = BTreeMap::new();
    for record in records {
        for entry in &record.entries {
            by_path
                .entry(entry.logical_path.as_str())
                .or_default()
                .insert((
                    record.producer.as_str(),
                    record.language.as_str(),
                    entry.import_root.as_str(),
                    entry.namespace.as_str(),
                    entry.exec_path.as_str(),
                    entry.replaces.as_str(),
                ));
        }
    }
    let mut conflicts = Vec::new();
    for (path, claims) in &by_path {
        if claims.len() > 1 {
            let claimants: Vec<&str> = claims.iter().map(|claim| claim.0).collect();
            conflicts.push(format!(
                "logical path '{path}' claimed by {}",
                claimants.join(", ")
            ));
        }
    }
    if conflicts.is_empty() {
        String::new()
    } else {
        format!("codegen path conflict: {}", conflicts.join("; "))
    }
}

#[derive(Serialize)]
struct FingerprintEntry<'a> {
    exec_path: &'a str,
    import_root: &'a str,
    logical_path: &'a str,
    namespace: &'a str,
    read_only: bool,
    replaces: &'a str,
}

#[derive(Serialize)]
struct FingerprintRecord<'a> {
    entries: Vec<FingerprintEntry<'a>>,
    language: &'a str,
    producer: &'a str,
}

pub fn fingerprint(records: &[CodegenRecord]) -> Result<String, dx_fingerprint::FingerprintError> {
    let merged = merge_records(records);
    let view: Vec<FingerprintRecord<'_>> = merged
        .iter()
        .map(|record| FingerprintRecord {
            entries: record
                .entries
                .iter()
                .map(|entry| FingerprintEntry {
                    exec_path: entry.exec_path.as_str(),
                    import_root: entry.import_root.as_str(),
                    logical_path: entry.logical_path.as_str(),
                    namespace: entry.namespace.as_str(),
                    read_only: true,
                    replaces: entry.replaces.as_str(),
                })
                .collect(),
            language: record.language.as_str(),
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
    pub records: Vec<CodegenRecord>,
    pub fingerprint: String,
    pub digest: [u8; 32],
    pub projection: Vec<ProjectionEntry>,
}

pub fn collect_plan(outputs: &[TargetOutput]) -> Result<CollectedPlan, CollectError> {
    let records = decode_shards(outputs)?;
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
    pub logical_path: String,
    pub artifact: String,
    pub import_root: String,
    pub namespace: String,
    pub replaces: String,
}

fn plan_projection(
    records: &[CodegenRecord],
    outputs: &[TargetOutput],
) -> Result<Vec<ProjectionEntry>, CollectError> {
    let bound = bound_entries(records);
    let generated_paths = generated_artifact_paths(outputs);
    let resolved = bind_artifacts(&bound, &generated_paths)?;
    let mut projection: Vec<ProjectionEntry> = bound
        .iter()
        .zip(&resolved)
        .map(|((_, entry), artifact)| ProjectionEntry {
            logical_path: entry.logical_path.clone(),
            artifact: (*artifact).to_owned(),
            import_root: entry.import_root.clone(),
            namespace: entry.namespace.clone(),
            replaces: entry.replaces.clone(),
        })
        .collect();
    projection.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    Ok(projection)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
