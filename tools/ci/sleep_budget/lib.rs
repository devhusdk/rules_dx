use std::collections::BTreeMap;

use dx_guard::allowlist::{self, Counts, add};

pub use dx_guard::allowlist::lowered;

pub const BUDGET: &str = "tools/ci/sleep_budget.txt";

pub const UPDATE: &str = "tools/dx run //tools/ci:sleep_budget_update";

pub const KIND: &str = "sleep";

pub const MIN_FILES: usize = 5000;

pub const INPUTS: [&str; 3] = ["DX_SOURCES_JSON", "DX_CODE_JSON", "DX_OTHER_CODE_JSON"];

const TEST_DIRS: [&str; 7] = [
    "e2e", "fixture", "fixtures", "test", "testdata", "testing", "tests",
];

const TEST_STEMS: [&str; 5] = ["conftest", "test", "test_support", "testing", "tests"];

const TEST_ENDS: [&str; 3] = ["_test", "_tests", "_testing"];

const CALL_SUFFIXES: [&str; 4] = ["", "_for", "_ms", "_until"];

const DEFINERS: [&str; 3] = ["def ", "fn ", "func "];

const SHELL_ARG_STARTS: [char; 5] = ['"', '$', '\'', '(', '{'];

fn test_word(word: &str) -> bool {
    word.starts_with("test_") || TEST_ENDS.iter().any(|end| word.ends_with(end))
}

pub fn test_path(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    parts
        .iter()
        .any(|dir| TEST_DIRS.contains(dir) || test_word(dir))
        || TEST_STEMS.contains(&stem)
        || test_word(stem)
        || stem.ends_with("_SUITE")
        || name.contains(".test.")
        || name.contains(".spec.")
}

fn identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn sleep_at(line: &str, at: usize) -> bool {
    let Some((before, rest)) = line.split_at_checked(at) else {
        return false;
    };
    let after = rest.strip_prefix("sleep").unwrap_or(rest);
    let program = before.ends_with("\"/bin/") || before.ends_with("\"/usr/bin/");
    let quoted = program || before.ends_with("new(\"") || before.ends_with("[\"");
    if before.ends_with("start-") || (quoted && after.starts_with('"')) {
        return true;
    }
    let escaped = before.ends_with("\\n") || before.ends_with("\\t");
    if !escaped && before.bytes().next_back().is_some_and(identifier) {
        return false;
    }
    let called = CALL_SUFFIXES.iter().any(|suffix| {
        after
            .strip_prefix(suffix)
            .is_some_and(|rest| rest.starts_with('('))
    });
    if called {
        return !DEFINERS.iter().any(|word| before.ends_with(word));
    }
    let argument = after.trim_start_matches(' ');
    argument.len() < after.len()
        && argument
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit() || SHELL_ARG_STARTS.contains(&first))
}

pub fn line_sleeps(line: &str) -> usize {
    let lower = line.to_ascii_lowercase();
    let sleeps = lower
        .match_indices("sleep")
        .filter(|(at, _)| sleep_at(&lower, *at))
        .count();
    sleeps + lower.matches("timeout /t").count()
}

fn test_cfg(code: &str) -> bool {
    (code.starts_with("#[cfg(") || code.starts_with("#![cfg("))
        && code.contains("test")
        && !code.contains("not(test")
}

fn braces(line: &str) -> (usize, usize) {
    (line.matches('{').count(), line.matches('}').count())
}

pub fn inline_test_lines(text: &str) -> Vec<&str> {
    if text.lines().any(|line| {
        let code = line.trim();
        code.starts_with("#![") && test_cfg(code)
    }) {
        return text.lines().collect();
    }
    let mut lines = Vec::new();
    let mut pending = false;
    let mut depth = 0usize;
    for line in text.lines() {
        let code = line.trim();
        if depth > 0 {
            lines.push(line);
            let (open, close) = braces(line);
            depth = (depth + open).saturating_sub(close);
        } else if test_cfg(code) {
            pending = true;
        } else if pending && !code.starts_with("#[") {
            let open = code.find('{');
            let semi = code.find(';');
            if semi.is_some_and(|semi| open.is_none_or(|open| semi < open)) {
                pending = false;
            } else {
                lines.push(line);
                if open.is_some() {
                    pending = false;
                    let (open, close) = braces(line);
                    depth = open.saturating_sub(close);
                }
            }
        }
    }
    lines
}

pub fn sleeps(path: &str, text: &str) -> usize {
    if test_path(path) {
        return text.lines().map(line_sleeps).sum();
    }
    if path.ends_with(".rs") {
        return inline_test_lines(text).into_iter().map(line_sleeps).sum();
    }
    0
}

fn skipped(path: &str) -> bool {
    path.starts_with(".dx/") || path.split('/').any(|part| part == "third_party")
}

pub fn scanned(mut files: BTreeMap<String, String>) -> BTreeMap<String, String> {
    files.retain(|path, _| !skipped(path));
    files
}

pub fn found(files: &BTreeMap<String, String>) -> Counts {
    let mut counts = Counts::new();
    for (path, text) in files {
        add(&mut counts, path, KIND, sleeps(path, text));
    }
    counts
}

pub fn over_budget(found: &Counts, budget: &Counts) -> Vec<String> {
    allowlist::over_budget(found, budget, "sleeps")
}

pub fn under_budget(found: &Counts, budget: &Counts) -> Vec<String> {
    allowlist::under_budget(found, budget, "sleeps")
}
