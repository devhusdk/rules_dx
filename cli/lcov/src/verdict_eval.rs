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
    let mut branches_found = 0;
    let mut branches_hit = 0;
    let mut branches_ignored = 0;
    let mut branches_missed = Vec::new();
    let mut branch_keys: Vec<(u32, u32, u32)> = hits.branches.keys().copied().collect();
    branch_keys.sort();
    for key in branch_keys {
        let line = key.0;
        if is_ignored(&ignores, line) {
            branches_ignored += 1;
        } else {
            branches_found += 1;
            match hits.branches[&key] {
                Some(taken) if taken > 0 => {
                    branches_hit += 1;
                }
                _ => {
                    branches_missed.push(key);
                }
            }
        }
    }
    if hits.lines.is_empty() && ignores.singles.is_empty() && ignores.ranges.is_empty() {
        errors.push(format!(
            "no instrumented lines and no validated ignores for eligible source: {path}"
        ));
    }
    if !hits.lines.is_empty() {
        for line in ignores.singles.keys() {
            if !hits.lines.contains_key(line) && !hits.branches.keys().any(|key| key.0 == *line) {
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
                && !hits
                    .branches
                    .keys()
                    .any(|key| range.0 <= key.0 && key.0 <= range.1)
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
        branches_found,
        branches_hit,
        branches_ignored,
        branches_missed,
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
            if is_starlark(path) && !hits.branches.is_empty() {
                verdict.errors.push(format!(
                    "unexpected Starlark branch data for {path}: the gate has no Starlark branch route"
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
        verdict.branches_found += file.branches_found;
        verdict.branches_hit += file.branches_hit;
        verdict.branches_ignored += file.branches_ignored;
        for key in &file.branches_missed {
            verdict
                .branches_missed
                .push(format!("{}:{}:{}:{}", file.path, key.0, key.1, key.2));
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
