//! Checks that a pull request body links the issues its work is for.

use std::collections::BTreeSet;

/// Phrases that, placed before `#N`, say the pull request does that issue's work.
const BEFORE: [&str; 8] = [
    "slice",
    "draft for",
    "fixes",
    "fix",
    "implements",
    "closes",
    "resolves",
    "part of",
];

/// Phrases that, placed after `#N`, say the pull request does that issue's work.
const AFTER: [&str; 2] = ["fixed by this pr", "done by this pr"];

/// One `#N` reference on a line, with the text since the previous one and up to the next one.
struct Ref<'a> {
    number: u64,
    before: &'a str,
    after: &'a str,
}

fn refs(line: &str) -> Vec<Ref<'_>> {
    let mut spans = Vec::new();
    let bytes = line.as_bytes();
    let mut at = 0;
    while let Some(offset) = line.get(at..).and_then(|rest| rest.find('#')) {
        let start = at + offset;
        let digits = line
            .get(start + 1..)
            .unwrap_or_default()
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        let word_before = start
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-'));
        let end = start + 1 + digits;
        if digits > 0
            && !word_before
            && let Some(number) = line.get(start + 1..end).and_then(|text| text.parse().ok())
        {
            spans.push((start, end, number));
        }
        at = end.max(start + 1);
    }
    let mut found = Vec::new();
    for (index, &(start, end, number)) in spans.iter().enumerate() {
        let previous_end = index
            .checked_sub(1)
            .and_then(|previous| spans.get(previous))
            .map_or(0, |&(_, previous_end, _)| previous_end);
        let next_start = spans
            .get(index + 1)
            .map_or(line.len(), |&(next_start, _, _)| next_start);
        found.push(Ref {
            number,
            before: line.get(previous_end..start).unwrap_or_default(),
            after: line.get(end..next_start).unwrap_or_default(),
        });
    }
    found
}

/// True when `text` holds only list separators, so a reference after it shares the one before.
fn list_gap(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || matches!(c, ',' | '&'))
        .all(|word| word.is_empty() || word.eq_ignore_ascii_case("and"))
}

/// True when `text` ends with `phrase` as whole words, followed by at most a count and "of" or "for".
fn ends_with_phrase(text: &str, phrase: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let Some(at) = text.rfind(phrase) else {
        return false;
    };
    let starts_word = !text
        .get(..at)
        .and_then(|head| head.chars().next_back())
        .is_some_and(char::is_alphanumeric);
    let tail = text.get(at + phrase.len()..).unwrap_or_default();
    let ends_word = !tail.chars().next().is_some_and(char::is_alphanumeric);
    starts_word
        && ends_word
        && tail
            .split(|c: char| c.is_whitespace() || matches!(c, ':' | '(' | '[' | '`'))
            .all(|word| {
                word.is_empty()
                    || matches!(word, "of" | "for" | "to")
                    || word.chars().all(|c| c.is_ascii_digit() || c == '/')
            })
}

/// True when `text` starts with `phrase` as whole words, ignoring leading punctuation and spaces.
fn starts_with_phrase(text: &str, phrase: &str) -> bool {
    let text = text
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '`' | ':' | '-'))
        .to_ascii_lowercase();
    text.strip_prefix(phrase)
        .is_some_and(|rest| !rest.chars().next().is_some_and(char::is_alphanumeric))
}

/// The issue numbers on `line` that a work phrase names, and the phrase for each.
fn worked(line: &str) -> Vec<(u64, &'static str)> {
    let mut found = Vec::new();
    let mut carried: Option<&'static str> = None;
    for reference in refs(line) {
        let before = BEFORE
            .into_iter()
            .find(|phrase| ends_with_phrase(reference.before, phrase))
            .or_else(|| carried.filter(|_| list_gap(reference.before)));
        let after = AFTER
            .into_iter()
            .find(|phrase| starts_with_phrase(reference.after, phrase));
        carried = before;
        if let Some(phrase) = before.or(after) {
            found.push((reference.number, phrase));
        }
    }
    found
}

/// The numbers a `Closes #N` or `Part of #N` line links, or `None` when `line` is not one.
fn link_line(line: &str) -> Option<Vec<u64>> {
    let trimmed =
        line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '*' | '>'));
    let lower = trimmed.to_ascii_lowercase();
    let rest = ["closes", "part of"]
        .into_iter()
        .find_map(|keyword| lower.strip_prefix(keyword))?;
    rest.trim_start().starts_with('#').then_some(())?;
    Some(
        refs(trimmed)
            .into_iter()
            .map(|reference| reference.number)
            .collect(),
    )
}

fn no_issue_line(line: &str) -> bool {
    line.trim()
        .strip_prefix("No issue:")
        .is_some_and(|reason| !reason.trim().is_empty())
}

/// The lines of `text` outside fenced code blocks.
fn prose_lines(text: &str) -> Vec<&str> {
    let mut fenced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            lines.push(line);
        }
    }
    lines
}

/// Every problem with the links in a pull request `body`, given the text of its `commits`.
pub fn problems(body: &str, commits: &str) -> Vec<String> {
    let body_lines = prose_lines(body);
    let mut linked = BTreeSet::new();
    let mut link_lines = 0;
    let mut no_issue = false;
    for line in &body_lines {
        if let Some(numbers) = link_line(line) {
            link_lines += 1;
            linked.extend(numbers);
        } else if no_issue_line(line) {
            no_issue = true;
        }
    }
    let mut problems = Vec::new();
    if link_lines == 0 && !no_issue {
        problems.push(
            "the body has no `Closes #N`, `Part of #N` or `No issue: <reason>` line".to_string(),
        );
    }
    let mut reported = BTreeSet::new();
    let sources = [
        ("the body", body_lines),
        ("a commit message", prose_lines(commits)),
    ];
    for (source, lines) in sources {
        for line in lines.into_iter().filter(|line| link_line(line).is_none()) {
            for (number, phrase) in worked(line) {
                if !linked.contains(&number) && reported.insert(number) {
                    problems.push(format!(
                        "{source} names #{number} with \"{phrase}\" but the body has no `Closes #{number}` or `Part of #{number}` line"
                    ));
                }
            }
        }
    }
    problems
}

/// Reads the pull request body from `GITHUB_EVENT_PATH` and the commit messages from an optional file.
pub fn check(args: &[String]) -> Result<(), String> {
    let commits = match args {
        [] => String::new(),
        [path] => std::fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))?,
        _ => return Err("usage: producers_bin --pr-links [COMMIT_MESSAGES_FILE]".into()),
    };
    let path = std::env::var("GITHUB_EVENT_PATH")
        .map_err(|error| format!("GITHUB_EVENT_PATH is not set: {error}"))?;
    let text = std::fs::read_to_string(&path).map_err(|error| format!("read {path}: {error}"))?;
    let event: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("parse {path}: {error}"))?;
    let body = event
        .get("pull_request")
        .and_then(|pull_request| pull_request.get("body"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let problems = problems(body, &commits);
    if problems.is_empty() {
        println!("pull request links are complete");
        return Ok(());
    }
    for problem in &problems {
        println!("::error title=Missing issue link::{problem}");
    }
    Err(format!(
        "{} missing issue link(s); add a `Closes #N`, `Part of #N` or `No issue: <reason>` line to the pull request body",
        problems.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::problems;

    /// The body of a merged pull request that fixed an issue only its commits named.
    const COVERAGE_BODY: &str = "Part of #13106.

- `cli/hpc_coverage` now runs the instrumented Haskell test binary directly.
- `patches/rules_haskell/ghc_xcode_env.patch` is new.

Left for the #13106 follow-up:
- Windows Julia coverage.
";

    const COVERAGE_COMMITS: &str =
        "coverage: haskell runs the test binary without the bash wrapper (part of #13106)

haskell: ghc actions carry the Xcode env on macOS (part of #13106, #13172)

haskell: static library action carries the Xcode env on macOS (part of #13106, #13172)
";

    /// The body of a merged pull request with no link line at all.
    const BOOTSTRAP_BODY: &str = "A fresh Linux container can now reach a first `tools/dx test` with one command.

## Changes

- `tools/dx`: the walk up stops at an execroot. That is why `dev_launcher_test` failed in the PR check, as on #13023.
- Three tests that fail on main are fixed in their own commit:
  - `docs_name_the_bazel_floor` still matched the README wording from before #13156.
";

    #[test]
    fn a_commit_that_works_on_an_unlinked_issue_fails() {
        assert_eq!(
            problems(COVERAGE_BODY, COVERAGE_COMMITS),
            [
                "a commit message names #13172 with \"part of\" but the body has no `Closes #13172` or `Part of #13172` line"
            ]
        );
    }

    #[test]
    fn a_body_without_a_link_line_fails() {
        assert_eq!(
            problems(BOOTSTRAP_BODY, ""),
            ["the body has no `Closes #N`, `Part of #N` or `No issue: <reason>` line"]
        );
    }

    #[test]
    fn closes_and_no_issue_lines_pass() {
        assert!(problems("Closes #12\n\nFixes #12 in the parser.", "").is_empty());
        assert!(problems("- part of #12, #13\n\nSlice 2 of #13.", "").is_empty());
        assert!(problems("No issue: typo in a comment.", "").is_empty());
        assert!(!problems("No issue:   ", "").is_empty());
    }

    #[test]
    fn work_phrases_name_only_the_issue_they_sit_next_to() {
        assert_eq!(
            problems("Closes #1\n\nSlice 1 of #2; #3 stays open.", ""),
            [
                "the body names #2 with \"slice\" but the body has no `Closes #2` or `Part of #2` line"
            ]
        );
        assert_eq!(
            problems("Closes #1\n\nRemaining for #1: #4 fixed by this PR.", ""),
            [
                "the body names #4 with \"fixed by this pr\" but the body has no `Closes #4` or `Part of #4` line"
            ]
        );
        assert_eq!(problems("Closes #1\n\nFixes #5, #6 and #7.", "").len(), 3);
    }

    #[test]
    fn code_blocks_and_urls_are_not_references() {
        let body = "Closes #1\n\n```\nFixes #9\n```\nSee https://example.com/page#10 and issue#11.";
        assert!(problems(body, "").is_empty());
    }
}
