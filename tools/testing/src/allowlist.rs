//! Allowlists of whole lines, sorted and without repeats.

use std::collections::BTreeSet;

/// The lines of an allowlist.
pub type Lines = BTreeSet<String>;

/// The lines of a sorted allowlist that `well_formed` accepts, and the lines that break the format.
pub fn parse_lines(text: &str, well_formed: impl Fn(&str) -> bool) -> (Lines, Vec<String>) {
    let mut lines = Lines::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        if line.trim() != line || !well_formed(line) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        lines.insert(line.to_owned());
    }
    (lines, problems)
}
