//! Conventional-commit parsing over `git log --format=%H%x00%s%x00%b%x1e`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Feat,
    Fix,
    Perf,
    Refactor,
    Docs,
    Test,
    Ci,
    Build,
    Chore,
    Style,
    Revert,
    Other,
}

impl Kind {
    fn from_word(word: &str) -> Kind {
        match word {
            "feat" => Kind::Feat,
            "fix" => Kind::Fix,
            "perf" => Kind::Perf,
            "refactor" => Kind::Refactor,
            "docs" => Kind::Docs,
            "test" => Kind::Test,
            "ci" => Kind::Ci,
            "build" => Kind::Build,
            "chore" => Kind::Chore,
            "style" => Kind::Style,
            "revert" => Kind::Revert,
            _ => Kind::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub kind: Kind,
    pub scope: Option<String>,
    pub breaking: bool,
    pub subject: String,
}

/// The `git log` arguments every read uses: merge commits are never read.
pub const LOG_ARGS: [&str; 3] = ["log", "--no-merges", "--format=%H%x00%s%x00%b%x1e"];

/// Kinds in the order `notes` lists them, with their headings.
pub const DEFAULT_GROUPS: &[(Kind, &str)] = &[
    (Kind::Feat, "Features"),
    (Kind::Fix, "Fixes"),
    (Kind::Perf, "Performance"),
    (Kind::Refactor, "Refactoring"),
    (Kind::Docs, "Documentation"),
    (Kind::Build, "Build"),
    (Kind::Ci, "CI"),
    (Kind::Test, "Tests"),
    (Kind::Revert, "Reverts"),
    (Kind::Other, "Other"),
];

pub fn parse(log: &str) -> Vec<Commit> {
    log.split('\x1e')
        .filter_map(|record| {
            let record = record.trim_start_matches(['\n', '\r']);
            if record.trim().is_empty() {
                return None;
            }
            let mut fields = record.splitn(3, '\0');
            let hash = fields.next()?.trim();
            let subject = fields.next()?.trim();
            let body = fields.next().unwrap_or("");
            if hash.is_empty() {
                return None;
            }
            Some(parse_commit(hash, subject, body))
        })
        .collect()
}

fn parse_commit(hash: &str, subject: &str, body: &str) -> Commit {
    let footer = body
        .lines()
        .any(|line| line.starts_with("BREAKING CHANGE:") || line.starts_with("BREAKING-CHANGE:"));
    match parse_subject(subject) {
        Some((kind, scope, bang, rest)) => Commit {
            hash: hash.to_owned(),
            kind,
            scope,
            breaking: bang || footer,
            subject: rest.to_owned(),
        },
        None => Commit {
            hash: hash.to_owned(),
            kind: Kind::Other,
            scope: None,
            breaking: footer,
            subject: subject.to_owned(),
        },
    }
}

fn parse_subject(subject: &str) -> Option<(Kind, Option<String>, bool, &str)> {
    let (head, rest) = subject.split_once(": ")?;
    let (head, bang) = match head.strip_suffix('!') {
        Some(head) => (head, true),
        None => (head, false),
    };
    let (word, scope) = match head.split_once('(') {
        Some((word, scope)) => {
            let scope = scope.strip_suffix(')')?;
            if scope.is_empty() {
                return None;
            }
            (word, Some(scope.to_owned()))
        }
        None => (head, None),
    };
    if word.is_empty() || !word.chars().all(|c| c.is_ascii_lowercase()) {
        return None;
    }
    Some((Kind::from_word(word), scope, bang, rest.trim()))
}

/// The commits in `log` whose subject is not a conventional commit, as `(hash, subject)`.
pub fn lint(log: &str) -> Vec<(String, String)> {
    parse(log)
        .into_iter()
        .filter(|commit| commit.kind == Kind::Other)
        .map(|commit| (commit.hash, commit.subject))
        .collect()
}

pub fn notes(commits: &[Commit], groups: &[(Kind, &str)]) -> String {
    let mut out = String::new();
    let breaking: Vec<&Commit> = commits.iter().filter(|c| c.breaking).collect();
    section(&mut out, "Breaking changes", &breaking);
    for (kind, heading) in groups {
        let in_group: Vec<&Commit> = commits.iter().filter(|c| c.kind == *kind).collect();
        section(&mut out, heading, &in_group);
    }
    out
}

fn section(out: &mut String, heading: &str, commits: &[&Commit]) {
    if commits.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("## {heading}\n\n"));
    for commit in commits {
        let short: String = commit.hash.chars().take(7).collect();
        match &commit.scope {
            Some(scope) => out.push_str(&format!("- **{scope}**: {} ({short})\n", commit.subject)),
            None => out.push_str(&format!("- {} ({short})\n", commit.subject)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINOR: &str = include_str!("../testdata/minor.log");
    const MAJOR: &str = include_str!("../testdata/major.log");
    const QUIET: &str = include_str!("../testdata/quiet.log");

    #[test]
    fn parses_scope_and_kind() {
        let commits = parse(MINOR);
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].kind, Kind::Feat);
        assert_eq!(commits[0].scope.as_deref(), Some("cli"));
        assert_eq!(commits[0].subject, "add dx init (#10)");
        assert_eq!(commits[1].scope, None);
        assert!(commits.iter().all(|c| !c.breaking));
    }

    #[test]
    fn bang_and_footer_are_breaking() {
        let commits = parse(MAJOR);
        assert_eq!(commits.len(), 3);
        assert!(commits[0].breaking);
        assert_eq!(commits[0].scope.as_deref(), Some("api"));
        assert!(!commits[1].breaking);
        assert_eq!(commits[2].kind, Kind::Refactor);
        assert!(commits[2].breaking);
    }

    #[test]
    fn merge_and_plain_subjects_are_other() {
        let commits = parse(QUIET);
        assert_eq!(commits.len(), 5);
        assert_eq!(commits[0].kind, Kind::Other);
        assert_eq!(
            commits[0].subject,
            "Merge pull request #30 from devhusdk/topic"
        );
        assert_eq!(commits[1].kind, Kind::Other);
        assert!(!commits[1].breaking);
    }

    #[test]
    fn lint_names_non_conventional_subjects() {
        let log = "aaaa1111\0feat: add a thing\0\x1ebbbb2222\0update stuff\0\x1e";
        assert_eq!(
            lint(log),
            [("bbbb2222".to_owned(), "update stuff".to_owned())]
        );
    }

    #[test]
    fn every_log_read_leaves_out_merge_commits() {
        assert!(LOG_ARGS.contains(&"--no-merges"));
    }

    #[test]
    fn notes_lists_breaking_first_in_group_order() {
        let notes = notes(&parse(MAJOR), DEFAULT_GROUPS);
        let expected = "\
## Breaking changes

- **api**: drop the v1 flag (#20) (5555555)
- rename module (#22) (7777777)

## Features

- **api**: drop the v1 flag (#20) (5555555)

## Fixes

- **release**: pin cosign (#21) (6666666)

## Refactoring

- rename module (#22) (7777777)
";
        assert_eq!(notes, expected);
    }

    #[test]
    fn notes_skip_kinds_outside_the_groups() {
        let notes = notes(&parse(QUIET), DEFAULT_GROUPS);
        assert!(notes.contains("## Other"));
        assert!(notes.contains("## Documentation"));
        assert!(!notes.contains("chore"));
        assert!(notes.find("Documentation") < notes.find("Other"));
    }
}
