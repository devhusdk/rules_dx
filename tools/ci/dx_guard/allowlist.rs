//! Allowlists of `<path> <kind> <count>` lines, and their check against what a guard found.

use std::collections::BTreeMap;

use crate::snapshot::read_input;

/// Occurrences keyed by file path and kind.
pub type Counts = BTreeMap<(String, String), usize>;

/// The fix a guard names for a new occurrence when it has no better one.
pub const REMOVE: &str = "remove the new one";

/// Adds one occurrence of `kind` in `path`.
pub fn hit(counts: &mut Counts, path: &str, kind: &str) {
    *counts
        .entry((path.to_owned(), kind.to_owned()))
        .or_default() += 1;
}

/// The counts of an allowlist whose kinds are among `kinds`, and the lines that break the format.
pub fn parse(text: &str, kinds: &[&str]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, kind, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count.parse::<usize>().unwrap_or(0);
        if count == 0 || !kinds.contains(kind) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        let key = ((*path).to_owned(), (*kind).to_owned());
        if previous.as_ref().is_some_and(|before| *before >= key) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(key.clone());
        counts.insert(key, count);
    }
    (counts, problems)
}

/// Lines that differ between the tree and the allowlist; `fix` tells how to drop a new occurrence.
pub fn differences(found: &Counts, allowed: &Counts, fix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for ((path, kind), count) in found {
        let limit = allowed
            .get(&(path.clone(), kind.clone()))
            .copied()
            .unwrap_or(0);
        if *count > limit {
            out.push(format!("{path} {kind} {count}: {limit} allowed; {fix}"));
        }
    }
    for ((path, kind), limit) in allowed {
        let count = found
            .get(&(path.clone(), kind.clone()))
            .copied()
            .unwrap_or(0);
        if count == 0 {
            out.push(format!(
                "{path} {kind} {limit}: none left; delete the allowlist line"
            ));
        } else if count < *limit {
            out.push(format!(
                "{path} {kind} {limit}: {count} left; lower the allowlist count"
            ));
        }
    }
    out
}

/// The format problems of the allowlist `text`, then its differences from `found`.
pub fn check(found: &Counts, text: &str, kinds: &[&str], fix: &str) -> Vec<String> {
    let (allowed, mut problems) = parse(text, kinds);
    problems.extend(differences(found, &allowed, fix));
    problems
}

/// Fails when `found` and the allowlist named by `ALLOWLIST` disagree.
pub fn verify(found: &Counts, kinds: &[&str]) {
    let key = std::env::var("ALLOWLIST").unwrap_or_else(|_| panic!("ALLOWLIST is set by the test"));
    let problems = check(found, &read_input("ALLOWLIST"), kinds, REMOVE);
    assert!(
        problems.is_empty(),
        "{key} and the tree disagree:\n{}",
        problems.join("\n")
    );
}
