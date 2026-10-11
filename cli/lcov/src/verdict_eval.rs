use std::collections::BTreeMap;

use super::{is_covered_language, is_starlark, FileVerdict, GateVerdict};
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
    let mut covered = 0;
    let mut eligible = 0;
    let mut ignored = 0;
    let mut uncovered = Vec::new();
    let mut numbered: Vec<u32> = hits.lines.keys().copied().collect();
    numbered.sort();
    for line in numbered {
        if is_ignored(&ignores, line) {
            ignored += 1;
        } else if hits.lines[&line] > 0 {
            covered += 1;
            eligible += 1;
        } else {
            uncovered.push(line);
            eligible += 1;
        }
    }
    let mut branch_found = 0;
    let mut branch_hit = 0;
    let mut missed_branches = Vec::new();
    for ((line, block, branch), taken) in &hits.branches {
        branch_found += 1;
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
        branch_found,
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
        verdict.branch_found += file.branch_found;
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
