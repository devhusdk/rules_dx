//! The changed-line coverage gate: added lines from a unified diff crossed with LCOV line hits.

use std::collections::{BTreeMap, BTreeSet};

use crate::accounting::{count_lines, source_key};
use crate::{find_ignores, merge_lcov_reports, CellAccount, Disposition, FileHits, LcovError};

/// The added lines of every file a diff touches, keyed by the new path.
pub type AddedLines = BTreeMap<String, BTreeSet<u32>>;

/// One changed executable line the coverage report saw without hits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangedLine {
    pub path: String,
    pub line: u32,
}

/// The changed lines of the owned production sources, accounted against the merged LCOV report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedAccount {
    pub covered_lines: u64,
    pub eligible_lines: u64,
    pub uncovered: Vec<ChangedLine>,
    /// Changed production sources an instrumented target owns that the report never saw.
    pub uninstrumented: Vec<String>,
}

/// The verdict of the changed-line gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangedResult {
    Pass,
    Fail,
    NotApplicable,
    Incomplete,
}

impl ChangedResult {
    /// The token the `changed_line_coverage` event carries.
    pub fn name(self) -> &'static str {
        match self {
            ChangedResult::Pass => "pass",
            ChangedResult::Fail => "fail",
            ChangedResult::NotApplicable => "not-applicable",
            ChangedResult::Incomplete => "incomplete",
        }
    }

    /// True when the gate lets the run pass.
    pub fn ok(self) -> bool {
        matches!(self, ChangedResult::Pass | ChangedResult::NotApplicable)
    }
}

impl ChangedAccount {
    /// Incomplete with any uninstrumented file, else not applicable without eligible lines, else the exact percent compare.
    pub fn result(&self, minimum: u32) -> ChangedResult {
        if !self.uninstrumented.is_empty() {
            ChangedResult::Incomplete
        } else if self.eligible_lines == 0 {
            ChangedResult::NotApplicable
        } else if self.covered_lines * 100 >= u64::from(minimum) * self.eligible_lines {
            ChangedResult::Pass
        } else {
            ChangedResult::Fail
        }
    }
}

/// Undoes Git's C-style quoting of one diff path, or returns it as is when unquoted.
fn unquote(raw: &str) -> String {
    let Some(inner) = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return raw.to_string();
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.bytes().peekable();
    while let Some(byte) = chars.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        match chars.next() {
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(b'a') => bytes.push(7),
            Some(b'b') => bytes.push(8),
            Some(b'f') => bytes.push(12),
            Some(b'v') => bytes.push(11),
            Some(b'r') => bytes.push(b'\r'),
            Some(digit @ b'0'..=b'7') => {
                let mut value = u32::from(digit - b'0');
                for _ in 0..2 {
                    match chars.peek() {
                        Some(next @ b'0'..=b'7') => {
                            value = value * 8 + u32::from(next - b'0');
                            chars.next();
                        }
                        _ => break,
                    }
                }
                bytes.push((value & 0xff) as u8);
            }
            Some(other) => bytes.push(other),
            None => bytes.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The new path a `+++` header names, or `None` for a deleted file.
fn target_path(header: &str) -> Option<String> {
    let raw = header.strip_suffix('\t').unwrap_or(header);
    if raw == "/dev/null" {
        return None;
    }
    let path = unquote(raw);
    let path = path.strip_prefix("b/").unwrap_or(&path);
    Some(source_key(path))
}

/// The start line and line count of one side of a hunk header (`+c,d`, `+c`).
fn hunk_range(spec: &str) -> Option<(u32, u32)> {
    let digits = spec.get(1..)?;
    match digits.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((digits.parse().ok()?, 1)),
    }
}

/// Parses the added lines of a unified diff (`git diff -U0` or with context) by new path.
///
/// Renames count only their edited lines, deletions and binary files add nothing, and paths that
/// differ only by a `./` prefix or backslashes share one key.
pub fn parse_added_lines(diff: &str) -> AddedLines {
    let mut added = AddedLines::new();
    let mut current: Option<String> = None;
    let mut old_left = 0u32;
    let mut new_left = 0u32;
    let mut next_line = 0u32;
    for line in diff.lines() {
        if old_left > 0 || new_left > 0 {
            match line.as_bytes().first() {
                Some(b'+') => {
                    if let Some(path) = &current {
                        added.entry(path.clone()).or_default().insert(next_line);
                    }
                    next_line += 1;
                    new_left = new_left.saturating_sub(1);
                }
                Some(b'-') => old_left = old_left.saturating_sub(1),
                Some(b'\\') => {}
                _ => {
                    next_line += 1;
                    old_left = old_left.saturating_sub(1);
                    new_left = new_left.saturating_sub(1);
                }
            }
            continue;
        }
        if line.starts_with("diff --git ") {
            current = None;
        } else if let Some(header) = line.strip_prefix("+++ ") {
            current = target_path(header);
        } else if let Some(header) = line.strip_prefix("@@ ") {
            let mut specs = header.split(' ');
            let old = specs.next().and_then(hunk_range);
            let new = specs.next().and_then(hunk_range);
            if let (Some((_, old_count)), Some((start, new_count))) = (old, new) {
                old_left = old_count;
                new_left = new_count;
                next_line = start;
            }
        }
    }
    added
}

/// Every line of one new file, which counts as added in full.
pub fn all_lines(text: &str) -> BTreeSet<u32> {
    (1..=text.lines().count() as u32).collect()
}

/// Accounts the added lines of every eligible owned source against the merged LCOV report.
///
/// Support, test, unsupported, excluded and unowned files are skipped, ignore markers still apply,
/// and a changed production source the report never saw is uninstrumented.
pub fn changed_account(
    account: &CellAccount,
    lcov_documents: &[String],
    added: &AddedLines,
    load_source: &dyn Fn(&str) -> Result<String, LcovError>,
) -> ChangedAccount {
    let changed: BTreeMap<String, &BTreeSet<u32>> = added
        .iter()
        .map(|(path, lines)| (source_key(path), lines))
        .collect();
    let report: BTreeMap<String, FileHits> = merge_lcov_reports(lcov_documents)
        .unwrap_or_default()
        .into_iter()
        .map(|(path, hits)| (source_key(&path), hits))
        .collect();
    let mut result = ChangedAccount::default();
    for (path, disposition) in &account.files {
        if *disposition != Disposition::Eligible {
            continue;
        }
        let key = source_key(path);
        let (Some(lines), Some(hits)) = (changed.get(&key), report.get(&key)) else {
            continue;
        };
        let ignores = load_source(path)
            .ok()
            .and_then(|text| find_ignores(path, &text).ok())
            .unwrap_or_default();
        let mut touched = FileHits::default();
        for line in lines.iter() {
            if let Some(count) = hits.lines.get(line) {
                touched.lines.insert(*line, *count);
            }
        }
        let count = count_lines(&touched, &ignores);
        result.covered_lines += count.covered;
        result.eligible_lines += count.eligible;
        result
            .uncovered
            .extend(count.uncovered.into_iter().map(|line| ChangedLine {
                path: path.clone(),
                line,
            }));
    }
    for path in &account.absent {
        if changed.contains_key(&source_key(path)) {
            result.uninstrumented.push(path.clone());
        }
    }
    result
}

#[cfg(test)]
#[path = "changed_tests.rs"]
mod changed_tests;
