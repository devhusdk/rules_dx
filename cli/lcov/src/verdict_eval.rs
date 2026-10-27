use std::collections::{BTreeMap, BTreeSet};

use super::{is_covered_language, is_starlark, FileVerdict, GateVerdict};
use crate::accounting::{count_lines, LineCount};
use crate::{find_ignores, is_ignored, FileHits, LcovError, ELIGIBLE, SUPPORT};

fn check_file(
    path: &str,
    hits: &FileHits,
    load_source: &dyn Fn(&str) -> Result<String, LcovError>,
    errors: &mut Vec<String>,
) -> Option<FileVerdict> {
    let source = match load_source(path) {
        Ok(text) => text,
        Err(error) => {
            errors.push(error.to_string());
            return None;
        }
    };
    let ignores = match find_ignores(path, &source) {
        Ok(valid) => valid,
        Err(message) => {
            errors.push(message.to_string());
            return None;
        }
    };
    let LineCount {
        covered,
        eligible,
        ignored,
        uncovered,
    } = count_lines(hits, &ignores);
    let mut branch_eligible = 0;
    let mut branch_ignored = 0;
    let mut branch_hit = 0;
    let mut missed_branches = Vec::new();
    for ((line, block, branch), taken) in &hits.branches {
        if is_ignored(&ignores, *line) {
            branch_ignored += 1;
            continue;
        }
        branch_eligible += 1;
        if taken.is_some_and(|count| count > 0) {
            branch_hit += 1;
        } else {
            missed_branches.push((*line, *block, *branch));
        }
    }
    if hits.lines.is_empty() && ignores.singles.is_empty() && ignores.ranges.is_empty() {
        errors.push(format!(
            "no instrumented lines and no validated ignores for eligible source: {path}"
        ));
    }
    if !hits.lines.is_empty() {
        for line in ignores.singles.keys() {
            if !hits.lines.contains_key(line) {
                errors.push(format!(
                    "stale coverage ignore at {path}:{line}: no instrumented line; remove it"
                ));
            }
        }
        for range in &ignores.ranges {
            if !hits
                .lines
                .keys()
                .any(|line| range.0 <= *line && *line <= range.1)
            {
                errors.push(format!(
                    "stale coverage range at {path}:{}-{}: covers no instrumented lines; remove it",
                    range.0, range.1
                ));
            }
        }
        if eligible == 0 {
            errors.push(format!(
                "broad coverage ignore at {path}: all {} instrumented lines ignored; narrow the range or reclassify to support",
                hits.lines.len()
            ));
        }
    }
    Some(FileVerdict {
        path: path.to_string(),
        covered,
        eligible,
        uncovered,
        ignored,
        branch_eligible,
        branch_ignored,
        branch_hit,
        branch_data: hits.branch_data,
        missed_branches,
    })
}

pub fn evaluate(
    inventory: &BTreeMap<String, String>,
    bazel_sources: &[String],
    report: &BTreeMap<String, FileHits>,
    load_source: &dyn Fn(&str) -> Result<String, LcovError>,
) -> GateVerdict {
    let mut verdict = GateVerdict::default();
    if report.is_empty() {
        verdict.errors.push(
            "missing coverage report: no SF records; run bazel coverage //... --combined_report=lcov first"
                .to_string(),
        );
    }
    for (path, hits) in report {
        if is_covered_language(path) {
            match inventory.get(path.as_str()) {
                None => verdict
                    .errors
                    .push(format!("instrumented source not in inventory: {path}")),
                Some(disposition) if disposition == SUPPORT => {}
                Some(disposition) if disposition == ELIGIBLE => {
                    if let Some(file) = check_file(path, hits, load_source, &mut verdict.errors) {
                        verdict.files.push(file);
                    }
                }
                Some(disposition) => verdict.errors.push(format!(
                    "unknown disposition {disposition:?} for {path}; want \"eligible\" or \"support\""
                )),
            }
        } else {
            verdict.other_sources.push(path.clone());
            if is_starlark(path) && !hits.lines.is_empty() {
                verdict.errors.push(format!(
                    "unexpected Starlark line data for {path}: the gate has no Starlark line route"
                ));
            }
        }
    }
    for source in bazel_sources {
        if !inventory.contains_key(source) {
            verdict.errors.push(format!(
                "Bazel-declared source missing from inventory: {source}"
            ));
        }
    }
    for (path, disposition) in inventory {
        if disposition == ELIGIBLE {
            if !bazel_sources.contains(path) {
                verdict.errors.push(format!(
                    "inventory eligible source not declared by Bazel: {path}"
                ));
            }
            if !report.contains_key(path) {
                verdict.errors.push(format!(
                    "eligible source absent from coverage report: {path}"
                ));
            }
        } else if disposition == SUPPORT {
            verdict.support_files.push(path.clone());
            if !bazel_sources.contains(path) {
                verdict.errors.push(format!(
                    "inventory support source not declared by Bazel: {path}"
                ));
            }
        } else {
            verdict.errors.push(format!(
                "unknown disposition {disposition:?} for {path}; want \"eligible\" or \"support\""
            ));
        }
    }
    for file in &verdict.files {
        verdict.covered += file.covered;
        verdict.eligible += file.eligible;
        verdict.ignored += file.ignored;
        verdict.branch_eligible += file.branch_eligible;
        verdict.branch_ignored += file.branch_ignored;
        verdict.branch_hit += file.branch_hit;
        if file.branch_data {
            verdict.branch_files += 1;
        } else {
            verdict.branch_bare += 1;
        }
    }
    verdict.instrumented = verdict.eligible + verdict.ignored;
    let mut clean = verdict.errors.is_empty();
    for file in &verdict.files {
        if !file.uncovered.is_empty() {
            clean = false;
        }
    }
    if clean && verdict.eligible > 0 {
        verdict.passed = true;
    } else if clean {
        verdict
            .errors
            .push("no executable lines in scope: an empty denominator is never a pass".to_string());
    }
    verdict
}

pub(crate) fn normalize_path(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    let mut rest = slashed.as_str();
    while let Some(stripped) = rest.strip_prefix("./") {
        rest = stripped;
    }
    rest.to_string()
}

pub(crate) fn alias_errors(label: &str, paths: &[&str], errors: &mut Vec<String>) {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for path in paths {
        let key = normalize_path(path);
        match seen.get(&key) {
            Some(first) if first != path => errors.push(format!(
                "path alias in {label}: {first} and {path} name the same file"
            )),
            Some(_) => errors.push(format!("duplicate path in {label}: {path}")),
            None => {
                seen.insert(key, path);
            }
        }
    }
}

pub fn reconcile_tracked(
    inventory: &BTreeMap<String, String>,
    bazel_sources: &[String],
    tracked: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    let inventory_paths: Vec<&str> = inventory.keys().map(String::as_str).collect();
    let bazel_paths: Vec<&str> = bazel_sources.iter().map(String::as_str).collect();
    let tracked_paths: Vec<&str> = tracked.iter().map(String::as_str).collect();
    alias_errors("inventory", &inventory_paths, &mut errors);
    alias_errors("Bazel source list", &bazel_paths, &mut errors);
    alias_errors("tracked source list", &tracked_paths, &mut errors);
    let tracked_set: BTreeSet<&str> = tracked_paths.iter().copied().collect();
    for path in tracked {
        if is_covered_language(path) && !inventory.contains_key(path.as_str()) {
            errors.push(format!(
                "tracked production source has no disposition: {path}; add it to the inventory as eligible or support"
            ));
        }
    }
    for path in inventory.keys() {
        if !tracked_set.contains(path.as_str()) {
            errors.push(format!(
                "inventory source is not a tracked file: {path}; remove the stale row"
            ));
        }
    }
    errors
}
