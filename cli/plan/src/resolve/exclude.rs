use std::path::Path;

use dx_config::conventions::pattern_covers;

use super::{ResolveError, ResolvedScope, quote_set, rule_graph::is_pattern};

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

#[must_use]
pub fn covered(exclude: &[String], label: &str) -> bool {
    exclude.iter().any(|pattern| pattern_covers(pattern, label))
}

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

#[must_use]
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

#[must_use]
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

#[must_use]
pub fn test_support_query(resolved: &ResolvedScope) -> Option<String> {
    let (patterns, labels): (Vec<String>, Vec<String>) = resolved
        .targets
        .iter()
        .filter(|target| !resolved.kept.contains(target))
        .cloned()
        .partition(|target| is_pattern(target));
    if patterns.is_empty() {
        return None;
    }
    let mut scope = format!("set({})", quote_set(&patterns));
    if !resolved.excluded.is_empty() {
        scope = format!("({scope} except set({}))", quote_set(&resolved.excluded));
    }
    let mut expression = format!(
        "let s = {scope} in ((attr(testonly, 1, $s) except tests($s)) except kind(\"^test_suite rule$\", $s))"
    );
    let named: Vec<String> = labels
        .into_iter()
        .chain(resolved.kept.iter().cloned())
        .collect();
    if !named.is_empty() {
        expression = format!("({expression} except set({}))", quote_set(&named));
    }
    Some(expression)
}

#[cfg(test)]
#[path = "exclude_tests.rs"]
mod tests;
