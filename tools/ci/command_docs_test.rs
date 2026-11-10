//! Every command page under docs/cli/commands documents the commands, flags and exit codes that `dx <command> --help` prints and no other flag, outside tools/ci/command_docs_allowlist.txt.

use std::collections::{BTreeMap, BTreeSet};

use dx_plan::args::{HelpFlag, HelpSurface, help_surface};
use serde_json as _;
use starlark_syntax as _;
use yaml_serde as _;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/command_docs_allowlist.txt";

/// The kinds an allowlist line may name.
const KINDS: [&str; 4] = [
    "missing-exit-code",
    "missing-page",
    "undocumented-flag",
    "unknown-flag",
];

/// Programs whose flags a page may show without dx having them.
const OTHER_PROGRAMS: [&str; 2] = ["bazel", "bazelisk"];

/// The inline code spans and fenced code lines of one page.
fn code_text(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in page.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            out.push(line.trim().to_owned());
        } else {
            out.extend(line.split('`').skip(1).step_by(2).map(|span| span.replace("\\|", "|")));
        }
    }
    out
}

/// The `--flag` and `-f` words of one code text, up to a bare `--`; none when it runs another program.
fn flags_in(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let words: Vec<&str> = code
        .split(|letter: char| letter.is_whitespace() || "|[]()<>,;'\"".contains(letter))
        .filter(|word| !word.is_empty())
        .collect();
    if words.first().is_some_and(|word| OTHER_PROGRAMS.contains(word)) {
        return out;
    }
    for word in words {
        if word == "--" {
            break;
        }
        let name = word.split('=').next().unwrap_or(word);
        let body = name.trim_start_matches('-');
        let dashes = name.len() - body.len();
        let named = body.starts_with(|letter: char| letter.is_ascii_lowercase())
            && body.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-');
        if named && (dashes == 2 || (dashes == 1 && body.len() == 1)) {
            out.push(name.to_owned());
        }
    }
    out
}

/// The text under each heading of `page` that names exit codes.
fn exit_code_sections(page: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut fenced = false;
    for line in page.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if line.starts_with('#') && !fenced {
            inside = line.to_lowercase().contains("exit code");
        } else if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn has_number(text: &str, number: &str) -> bool {
    text.split(|letter: char| !letter.is_ascii_digit())
        .any(|word| word == number)
}

/// Every spelling of one flag.
fn spellings(flag: &HelpFlag) -> impl Iterator<Item = &str> {
    std::iter::once(&flag.name).chain(&flag.aliases).map(String::as_str)
}

/// Every gap between `surface` and `pages` as one allowlist line each.
fn gaps(surface: &[HelpSurface], pages: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut page_flags: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let every_flag: BTreeSet<&str> = surface
        .iter()
        .flat_map(|entry| entry.flags.iter().flat_map(spellings))
        .collect();
    for entry in surface {
        let command = entry.path.join(" ");
        let Some((name, page)) = entry.page.and_then(|name| pages.get(name).map(|page| (name, page))) else {
            out.insert(format!("{} missing-page {command}", entry.page.unwrap_or("-")));
            continue;
        };
        let code = code_text(page);
        let spelled = if entry.path.len() == 1 {
            format!("dx {command}")
        } else {
            command.clone()
        };
        if !entry.path.is_empty() && !code.iter().any(|text| text.contains(&spelled)) {
            out.insert(format!("{name} missing-page {command}"));
        }
        let documented: BTreeSet<String> = code.iter().flat_map(|text| flags_in(text)).collect();
        if !entry.path.is_empty() {
            let known = page_flags.entry(name).or_default();
            known.extend(entry.flags.iter().flat_map(spellings));
        }
        for flag in &entry.flags {
            if !spellings(flag).any(|spelling| documented.contains(spelling)) {
                out.insert(format!("{name} undocumented-flag {}", flag.name));
            }
        }
        let sections = exit_code_sections(page);
        for exit_code in &entry.exit_codes {
            if !has_number(&sections, exit_code) {
                out.insert(format!("{name} missing-exit-code {exit_code}"));
            }
        }
    }
    for (name, page) in pages {
        let known = page_flags.get(name.as_str()).unwrap_or(&every_flag);
        for flag in code_text(page).iter().flat_map(|text| flags_in(text)) {
            if !known.contains(flag.as_str()) && !HELP_FLAGS.contains(&flag.as_str()) {
                out.insert(format!("{name} unknown-flag {flag}"));
            }
        }
    }
    out
}

/// Reads `<page> <kind> <flag or command>` lines.
fn parse_allowlist(text: &str) -> (BTreeSet<String>, Vec<String>) {
    let mut lines = BTreeSet::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        let fields: Vec<&str> = line.splitn(3, ' ').collect();
        match fields.as_slice() {
            [page, kind, what] if !page.is_empty() && KINDS.contains(kind) && !what.is_empty() => {
                lines.insert(line.to_owned());
            }
            _ => problems.push(format!("malformed line: {line:?}")),
        }
    }
    (lines, problems)
}

/// Every way the gaps and the allowlist disagree.
fn differences(found: &BTreeSet<String>, allowed: &BTreeSet<String>) -> Vec<String> {
    let new = found
        .difference(allowed)
        .map(|line| format!("new gap, fix the page or the grammar: {line}"));
    let spare = allowed
        .difference(found)
        .map(|line| format!("no such gap left, delete the line: {line}"));
    new.chain(spare).collect()
}

/// The spellings every command takes.
const HELP_FLAGS: [&str; 2] = ["--help", "-h"];

fn read_runfile(key: &str) -> String {
    let path = dx_testing::rlocation(key).unwrap_or_else(|error| panic!("{key}: {error}"));
    dx_testing::read_text(&path)
}

#[test]
fn command_pages_match_help_outside_the_allowlist() {
    let pages: BTreeMap<String, String> = dx_testing::env_text("DX_COMMAND_DOCS")
        .split_whitespace()
        .map(|key| (key.rsplit('/').next().unwrap_or(key).to_owned(), read_runfile(key)))
        .collect();
    let surface = help_surface();
    assert!(surface.len() >= 25 && pages.len() >= 15, "{} commands, {} pages", surface.len(), pages.len());
    let found = gaps(&surface, &pages);
    let (allowed, mut problems) = parse_allowlist(&read_runfile(dx_testing::env_text("DX_ALLOWLIST").trim()));
    problems.extend(differences(&found, &allowed));
    if problems.is_empty() {
        return;
    }
    if let Some(dir) = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR") {
        let text: String = found.iter().map(|line| format!("{line}\n")).collect();
        dx_testing::write_text(&std::path::Path::new(&dir).join("command_docs_allowlist.txt"), &text);
    }
    panic!(
        "{ALLOWLIST} and the pages disagree ({} problems):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &[&str], page: Option<&'static str>, flags: &[&str], exit_codes: &[&str]) -> HelpSurface {
        HelpSurface {
            path: path.iter().map(|part| (*part).to_owned()).collect(),
            page,
            flags: flags
                .iter()
                .map(|flag| HelpFlag {
                    name: (*flag).to_owned(),
                    aliases: if *flag == "--output" { vec!["-o".to_owned()] } else { Vec::new() },
                })
                .collect(),
            exit_codes: exit_codes.iter().map(|code| (*code).to_owned()).collect(),
        }
    }

    fn pages(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
            .collect()
    }

    fn found(surface: &[HelpSurface], entries: &[(&str, &str)]) -> Vec<String> {
        gaps(surface, &pages(entries)).into_iter().collect()
    }

    const PAGE: &str = "# `dx tidy`\n\n```sh\ndx tidy --deep -- --keep_going\nbazel test --config=ci\n```\n\n| `-o, --output text\\|json` | Format. |\n\nRun `tidy sweep` to sweep. Plain --loose text.\n\n## Exit codes\n\n- `0` ok\n- `12` no\n\n## Next\n\n`2`\n";

    #[test]
    fn a_complete_page_has_no_gap() {
        let surface = [
            entry(&["tidy"], Some("tidy.md"), &["--deep", "--output"], &["0"]),
            entry(&["tidy", "sweep"], Some("tidy.md"), &["--output"], &[]),
        ];
        assert_eq!(found(&surface, &[("tidy.md", PAGE)]), [] as [&str; 0]);
    }

    #[test]
    fn a_flag_the_page_leaves_out_is_detected() {
        let surface = [entry(&["tidy"], Some("tidy.md"), &["--deep", "--output", "--loose", "--keep_going"], &[])];
        assert_eq!(
            found(&surface, &[("tidy.md", PAGE)]),
            ["tidy.md undocumented-flag --keep_going", "tidy.md undocumented-flag --loose"]
        );
    }

    #[test]
    fn a_flag_no_command_of_the_page_has_is_detected() {
        let surface = [
            entry(&["tidy"], Some("tidy.md"), &["--output"], &[]),
            entry(&["other"], Some("other.md"), &["--deep"], &[]),
        ];
        let other = "`dx other --deep`\n";
        let guide = "`dx tidy --deep --gone`, `bazelisk --batch`\n";
        assert_eq!(
            found(&surface, &[("tidy.md", PAGE), ("other.md", other), ("guide.md", guide)]),
            ["guide.md unknown-flag --gone", "tidy.md unknown-flag --deep"]
        );
    }

    #[test]
    fn a_short_spelling_documents_a_flag_and_the_root_entry_names_no_command() {
        let surface = [
            entry(&[], Some("guide.md"), &["--output", "--version"], &[]),
            entry(&["tidy"], Some("tidy.md"), &["--deep"], &[]),
        ];
        assert_eq!(
            found(&surface, &[("guide.md", "`-o json` and `--deep`\n")]),
            ["guide.md undocumented-flag --version", "tidy.md missing-page tidy"]
        );
    }

    #[test]
    fn a_command_without_a_page_is_detected() {
        let surface = [
            entry(&["tidy"], Some("gone.md"), &[], &[]),
            entry(&["sweep"], None, &[], &[]),
            entry(&["other"], Some("tidy.md"), &[], &[]),
            entry(&["tidy", "scrub"], Some("tidy.md"), &[], &[]),
        ];
        assert_eq!(
            found(&surface, &[("tidy.md", PAGE)]),
            [
                "- missing-page sweep",
                "gone.md missing-page tidy",
                "tidy.md missing-page other",
                "tidy.md missing-page tidy scrub",
                "tidy.md unknown-flag --deep",
                "tidy.md unknown-flag --output",
                "tidy.md unknown-flag -o",
            ]
        );
    }

    #[test]
    fn an_exit_code_outside_the_exit_codes_section_is_detected() {
        let surface = [entry(&["tidy"], Some("tidy.md"), &["--deep", "--output"], &["0", "1", "2", "12"])];
        assert_eq!(
            found(&surface, &[("tidy.md", PAGE)]),
            ["tidy.md missing-exit-code 1", "tidy.md missing-exit-code 2"]
        );
    }

    #[test]
    fn a_spare_malformed_or_unsorted_allowlist_line_fails() {
        let (allowed, problems) = parse_allowlist("a.md missing-page tidy scrub\nb.md unknown-flag --x\n");
        assert!(problems.is_empty(), "{problems:?}");
        let gaps: BTreeSet<String> = ["b.md unknown-flag --x".to_owned(), "c.md unknown-flag --y".to_owned()].into();
        assert_eq!(
            differences(&gaps, &allowed),
            [
                "new gap, fix the page or the grammar: c.md unknown-flag --y",
                "no such gap left, delete the line: a.md missing-page tidy scrub",
            ]
        );
        let (_, problems) = parse_allowlist("b.md unknown-flag --x\na.md other --x\na.md unknown-flag\n");
        assert_eq!(problems.len(), 3, "{problems:?}");
    }
}
