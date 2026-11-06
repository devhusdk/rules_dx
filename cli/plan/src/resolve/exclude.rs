//! `dx.toml` `[scope] exclude`: the only place that reads it.
//!
//! Excludes never reach Bazel flags, so they never change an action key. Concrete
//! labels under an exclude are dropped here; patterns such as `//...` keep their
//! spelling and get `-pattern` terms when the Bazel command line is built.

use std::path::Path;

use dx_config::conventions::pattern_covers;

use super::{ResolveError, ResolvedScope, quote_set};

/// The exclude patterns of `workspace/dx.toml`; none when the file is missing.
pub fn load(workspace: &Path) -> Result<Vec<String>, ResolveError> {
    match dx_config::load(workspace) {
        Ok(config) => Ok(config
            .map(|config| config.scope.exclude)
            .unwrap_or_default()),
        Err(problem) => Err(ResolveError::Config {
            message: problem.to_string(),
        }),
    }
}

/// Whether any exclude pattern covers `label`.
pub fn covered(exclude: &[String], label: &str) -> bool {
    exclude.iter().any(|pattern| pattern_covers(pattern, label))
}

/// Drops excluded targets, keeps the ones named in `explicit`, and records the
/// patterns Bazel must subtract from what is left.
pub fn apply(
    mut resolved: ResolvedScope,
    exclude: &[String],
    explicit: &[String],
) -> Result<ResolvedScope, ResolveError> {
    if exclude.is_empty() {
        return Ok(resolved);
    }
    let mut targets = Vec::new();
    for target in std::mem::take(&mut resolved.targets) {
        if !covered(exclude, &target) {
            targets.push(target);
        } else if explicit.contains(&target) {
            resolved.kept.push(target.clone());
            targets.push(target);
        }
    }
    if targets.is_empty() {
        let scopes = if explicit.is_empty() {
            vec!["//...".to_owned()]
        } else {
            explicit.to_vec()
        };
        return Err(ResolveError::AllExcluded { scopes });
    }
    resolved.excluded = exclude
        .iter()
        .filter(|pattern| {
            targets
                .iter()
                .any(|target| !resolved.kept.contains(target) && pattern_covers(target, pattern))
        })
        .cloned()
        .collect();
    if let dx_process::Scope::Labels(labels) | dx_process::Scope::ResolvedOwners(labels) =
        &mut resolved.scope
    {
        labels.clone_from(&targets);
    }
    resolved.targets = targets;
    Ok(resolved)
}

/// The labels for a Bazel command line: targets, then `-pattern` for each exclude,
/// then the explicitly named targets an exclude would otherwise remove.
pub fn bazel_labels(resolved: &ResolvedScope) -> Vec<String> {
    let mut labels: Vec<String> = resolved
        .targets
        .iter()
        .filter(|target| !resolved.kept.contains(target))
        .cloned()
        .collect();
    labels.extend(
        resolved
            .excluded
            .iter()
            .map(|pattern| format!("-{pattern}")),
    );
    labels.extend(resolved.kept.iter().cloned());
    labels
}

/// The query expression for the targets, with the excludes subtracted.
pub fn query_set(resolved: &ResolvedScope) -> String {
    let plain: Vec<String> = resolved
        .targets
        .iter()
        .filter(|target| !resolved.kept.contains(target))
        .cloned()
        .collect();
    let mut expression = format!("set({})", quote_set(&plain));
    if !resolved.excluded.is_empty() {
        expression = format!(
            "({expression} except set({}))",
            quote_set(&resolved.excluded)
        );
    }
    if !resolved.kept.is_empty() {
        expression = format!("({expression} + set({}))", quote_set(&resolved.kept));
    }
    expression
}

#[cfg(test)]
#[path = "exclude_tests.rs"]
mod tests;
