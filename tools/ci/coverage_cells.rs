//! Aggregates per-cell `coverage` events into one required verdict.
//!
//! Each input line is a `dx coverage` NDJSON `coverage` event that the workflow
//! extended with `sha` and `collector`. Every expected cell must report exactly
//! once, at the expected SHA, with the same collector, and with covered equal to
//! eligible over a non-empty denominator.

use std::collections::BTreeMap;

use serde_json::Value;

/// Expected cell names from a `cells.txt` body (comments and blanks skipped).
pub fn expected_cells(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn number(event: &Value, key: &str) -> Option<u64> {
    event.get(key).and_then(Value::as_u64)
}

/// Returns every reason the aggregate fails; empty means it passes.
pub fn aggregate(expected: &[String], sha: &str, ndjson: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut by_cell: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (index, line) in ndjson.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            errors.push(format!("line {}: malformed report", index + 1));
            continue;
        };
        if event.get("event").and_then(Value::as_str) != Some("coverage") {
            continue;
        }
        let Some(cell) = event.get("cell").and_then(Value::as_str) else {
            errors.push(format!("line {}: coverage event names no cell", index + 1));
            continue;
        };
        by_cell.entry(cell.to_owned()).or_default().push(event);
    }
    for cell in by_cell.keys() {
        if !expected.contains(cell) {
            errors.push(format!("{cell}: unexpected cell"));
        }
    }
    let mut collectors = Vec::new();
    for cell in expected {
        let events = by_cell.get(cell).map(Vec::as_slice).unwrap_or_default();
        let [event] = events else {
            let found = events.len();
            errors.push(format!(
                "{cell}: expected exactly one report, found {found}"
            ));
            continue;
        };
        match event.get("sha").and_then(Value::as_str) {
            Some(found) if found == sha => {}
            Some(found) => errors.push(format!("{cell}: stale sha {found}, expected {sha}")),
            None => errors.push(format!("{cell}: report carries no sha")),
        }
        match event.get("collector").and_then(Value::as_str) {
            Some(collector) if !collector.is_empty() => collectors.push((cell, collector)),
            _ => errors.push(format!("{cell}: report carries no collector")),
        }
        let covered = number(event, "covered_lines");
        let eligible = number(event, "eligible_lines");
        match (covered, eligible) {
            (Some(_), Some(0)) => errors.push(format!("{cell}: empty denominator")),
            (Some(covered), Some(eligible)) if covered != eligible => {
                errors.push(format!(
                    "{cell}: covered {covered} of {eligible} eligible lines"
                ));
            }
            (Some(_), Some(_)) => {}
            _ => errors.push(format!("{cell}: report lacks line counts")),
        }
        if number(event, "errors") != Some(0) {
            errors.push(format!("{cell}: report records errors"));
        }
        if event.get("passed").and_then(Value::as_bool) != Some(true) {
            errors.push(format!("{cell}: gate did not pass"));
        }
    }
    if let Some((first_cell, first)) = collectors.first() {
        for (cell, collector) in &collectors {
            if collector != first {
                errors.push(format!(
                    "{cell}: collector {collector} differs from {first_cell} ({first})"
                ));
            }
        }
    }
    errors
}
