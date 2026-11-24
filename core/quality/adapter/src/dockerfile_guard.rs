//! Dockerfile patterns that the strict tier fails on.

use std::collections::{BTreeMap, BTreeSet};

use crate::parsers::FileFinding;
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "hadolint";

/// A `# hadolint ignore=` pragma with no `# reason` after its rule list.
pub const REASONLESS_IGNORE: &str = "guard/reasonless_ignore";
/// A `FROM` image with no `@sha256:` digest.
pub const UNPINNED_IMAGE: &str = "guard/unpinned_image";
/// `curl -k`, `curl --insecure` or `wget --no-check-certificate` in `RUN`.
pub const TLS_OFF: &str = "guard/tls_off";

const DIGEST: &str = "@sha256:";

/// One instruction with its continuation lines joined.
struct Instruction {
    line: u64,
    text: String,
}

/// Returns a finding for each banned pattern in one Dockerfile.
#[must_use]
pub fn dockerfile_guard(file: &str, source: &str) -> Vec<FileFinding> {
    let mut findings = Vec::new();
    let (instructions, ignores) = split(source);
    for (line, column) in ignores {
        findings.push(finding(
            file,
            line,
            column,
            REASONLESS_IGNORE,
            "a hadolint ignore needs a reason: add `# <why>` after the rule list".to_owned(),
        ));
    }
    let mut args = BTreeMap::new();
    let mut stages = BTreeSet::new();
    let mut seen_from = false;
    for instruction in &instructions {
        let mut words = instruction.text.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        let rest: Vec<&str> = words.collect();
        match keyword.to_ascii_uppercase().as_str() {
            // Only an ARG before the first FROM can name a base image.
            "ARG" if !seen_from => {
                for arg in &rest {
                    if let Some((name, value)) = arg.split_once('=') {
                        args.insert(name.to_owned(), unquote(value).to_owned());
                    }
                }
            }
            "FROM" => {
                seen_from = true;
                let mut operands = rest.iter().filter(|word| !word.starts_with("--"));
                let Some(image) = operands.next() else {
                    continue;
                };
                if !pinned(image, &args, &stages) {
                    findings.push(finding(
                        file,
                        instruction.line,
                        1,
                        UNPINNED_IMAGE,
                        format!(
                            "`FROM {image}` has no `{DIGEST}` digest; pin the base image by digest"
                        ),
                    ));
                }
                if let (Some(word), Some(alias)) = (operands.next(), operands.next())
                    && word.eq_ignore_ascii_case("as")
                {
                    stages.insert(alias.to_ascii_lowercase());
                }
            }
            "RUN" => {
                if let Some(flag) = tls_off(&rest) {
                    findings.push(finding(
                        file,
                        instruction.line,
                        1,
                        TLS_OFF,
                        format!("`{flag}` turns off TLS verification; fix the certificate instead"),
                    ));
                }
            }
            _ => {}
        }
    }
    findings
}

/// Splits the source into instructions and returns the line and column of each reasonless ignore.
fn split(source: &str) -> (Vec<Instruction>, Vec<(u64, u64)>) {
    let escape = escape_char(source);
    let mut instructions = Vec::new();
    let mut ignores = Vec::new();
    let mut current: Option<Instruction> = None;
    let mut heredocs: Vec<String> = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let line = u64::try_from(index).unwrap_or(u64::MAX - 1) + 1;
        let trimmed = raw.trim_start();
        if let Some(end) = heredocs.first() {
            if trimmed.trim_end() == end {
                heredocs.remove(0);
            }
            if let Some(last) = instructions.last_mut() {
                push_text(last, raw);
            }
            continue;
        }
        if let Some(comment) = trimmed.strip_prefix('#') {
            if reasonless_ignore(comment) {
                let column = u64::try_from(raw.len() - trimmed.len()).unwrap_or(u64::MAX - 1) + 1;
                ignores.push((line, column));
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let body = raw.trim_end();
        let (body, continues) = match body.strip_suffix(escape) {
            Some(head) => (head, true),
            None => (body, false),
        };
        let instruction = current.get_or_insert_with(|| Instruction {
            line,
            text: String::new(),
        });
        push_text(instruction, body);
        if !continues && let Some(done) = current.take() {
            heredocs = heredoc_ends(&done.text);
            instructions.push(done);
        }
    }
    instructions.extend(current);
    (instructions, ignores)
}

fn push_text(instruction: &mut Instruction, text: &str) {
    if !instruction.text.is_empty() {
        instruction.text.push(' ');
    }
    instruction.text.push_str(text);
}

/// The `# escape=` parser directive, which must come before any other line.
fn escape_char(source: &str) -> char {
    for line in source.lines() {
        let Some(directive) = line.trim().strip_prefix('#') else {
            break;
        };
        let Some((key, value)) = directive.split_once('=') else {
            break;
        };
        if key.trim().eq_ignore_ascii_case("escape") {
            return value.trim().chars().next().unwrap_or('\\');
        }
    }
    '\\'
}

/// The terminators of the heredocs an instruction opens, such as `EOF` in `RUN <<EOF`.
fn heredoc_ends(text: &str) -> Vec<String> {
    text.split("<<")
        .skip(1)
        .filter_map(|tail| {
            let tail = tail.strip_prefix('-').unwrap_or(tail);
            let word: String = tail
                .trim_start_matches(['"', '\''])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            (!word.is_empty()).then_some(word)
        })
        .collect()
}

/// Whether a comment body, after `#`, is a hadolint ignore with no `# reason` after its rules.
fn reasonless_ignore(comment: &str) -> bool {
    let Some(rest) = comment.trim_start().strip_prefix("hadolint") else {
        return false;
    };
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("global").map_or(rest, str::trim_start);
    let Some(rules) = rest.strip_prefix("ignore=") else {
        return false;
    };
    let after = rules.trim_start_matches(|c: char| !c.is_whitespace() && c != '#');
    after
        .trim_start()
        .strip_prefix('#')
        .is_none_or(|reason| reason.trim().is_empty())
}

/// Whether a FROM image is `scratch`, an earlier stage, or carries a digest once global ARGs expand.
fn pinned(image: &str, args: &BTreeMap<String, String>, stages: &BTreeSet<String>) -> bool {
    let image = expand(image, args);
    let lower = image.to_ascii_lowercase();
    lower == "scratch"
        || stages.contains(&lower)
        || (!image.contains('$') && image.contains(DIGEST))
}

/// Replaces `$NAME`, `${NAME}` and `${NAME:-default}` with ARG defaults; unknown names stay.
fn expand(word: &str, args: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = word;
    while let Some((head, tail)) = rest.split_once('$') {
        out.push_str(head);
        let (name, fallback, after) = match tail
            .strip_prefix('{')
            .and_then(|inner| inner.split_once('}'))
        {
            Some((body, after)) => {
                let (name, fallback) = body
                    .split_once(":-")
                    .map_or((body, None), |(name, value)| (name, Some(value)));
                (name, fallback, after)
            }
            None => {
                let after =
                    tail.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
                let name = tail.strip_suffix(after).unwrap_or(tail);
                (name, None, after)
            }
        };
        match args.get(name).map(String::as_str).or(fallback) {
            Some(value) if !value.is_empty() => out.push_str(value),
            // Keep the reference as written, so an unresolved image still shows a `$`.
            _ => {
                out.push('$');
                out.push_str(tail.strip_suffix(after).unwrap_or(tail));
            }
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn unquote(value: &str) -> &str {
    value.trim_matches(['"', '\''])
}

/// The first flag that turns off TLS verification in a curl or wget call, if any.
fn tls_off(words: &[&str]) -> Option<String> {
    let mut command: Option<&str> = None;
    for raw in words {
        let word = raw.trim_matches(['"', '\'', '[', ']', ',']);
        let ends_command = word.ends_with(';');
        let word = word.trim_end_matches(';');
        if matches!(word, "&&" | "||" | "|" | "") {
            command = None;
            continue;
        }
        let name = word.rsplit('/').next().unwrap_or(word);
        if name == "curl" || name == "wget" {
            command = Some(name);
        } else {
            let insecure = match command {
                Some("curl") => {
                    word == "--insecure"
                        || (word.starts_with('-') && !word.starts_with("--") && word.contains('k'))
                }
                Some(_) => word == "--no-check-certificate",
                None => false,
            };
            if let (true, Some(tool)) = (insecure, command) {
                return Some(format!("{tool} {word}"));
            }
        }
        if ends_command {
            command = None;
        }
    }
    None
}

fn finding(file: &str, line: u64, column: u64, rule: &str, message: String) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message,
            severity: ToolSeverity::Error,
            start: TextPosition { line, column },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Utf8,
            related: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str =
        "busybox:1.36@sha256:73aaf090f3d85aa34ee199857f03fa3a95c8ede2ffd4cc2cdb5b94e566b11662";

    fn rules(source: &str) -> Vec<(String, u64)> {
        dockerfile_guard("Dockerfile", source)
            .into_iter()
            .map(|f| (f.finding.rule_id, f.finding.start.line))
            .collect()
    }

    fn one(rule: &str, line: u64) -> Vec<(String, u64)> {
        vec![(rule.to_owned(), line)]
    }

    #[test]
    fn clean_source_has_no_findings() {
        let source = format!(
            "# hadolint ignore=DL3018 # apk pins drift weekly\n\
             FROM {PINNED} AS build\n\
             RUN apk add curl && curl -fsSL https://example.com/a -o /a\n\
             FROM build\n\
             FROM scratch\n\
             COPY --from=build /a /a\n"
        );
        assert_eq!(rules(&source), []);
    }

    #[test]
    fn an_ignore_needs_a_reason() {
        assert_eq!(
            rules(&format!("# hadolint ignore=DL3007\nFROM {PINNED}\n")),
            one(REASONLESS_IGNORE, 1)
        );
        assert_eq!(
            rules(&format!(
                "FROM {PINNED}\n  # hadolint global ignore=DL3007,DL3008 #\n"
            )),
            one(REASONLESS_IGNORE, 2)
        );
        assert_eq!(
            rules(&format!(
                "# hadolint global ignore=DL3007 # base moves with releases\nFROM {PINNED}\n"
            )),
            []
        );
        assert_eq!(
            rules(&format!("# hadolint shell=bash\nFROM {PINNED}\n")),
            []
        );
    }

    #[test]
    fn a_base_image_needs_a_digest() {
        assert_eq!(rules("FROM busybox:1.36\n"), one(UNPINNED_IMAGE, 1));
        assert_eq!(
            rules("FROM --platform=linux/amd64 busybox:latest AS base\n"),
            one(UNPINNED_IMAGE, 1)
        );
        assert_eq!(rules("from \\\n  busybox\n"), one(UNPINNED_IMAGE, 1));
    }

    #[test]
    fn global_args_expand_in_from() {
        assert_eq!(rules(&format!("ARG BASE={PINNED}\nFROM ${{BASE}}\n")), []);
        assert_eq!(rules(&format!("FROM ${{BASE:-{PINNED}}}\n")), []);
        assert_eq!(
            rules("ARG BASE=busybox\nFROM $BASE\n"),
            one(UNPINNED_IMAGE, 2)
        );
        assert_eq!(rules("ARG BASE\nFROM ${BASE}\n"), one(UNPINNED_IMAGE, 2));
    }

    #[test]
    fn insecure_downloads_fail() {
        let run = |cmd: &str| rules(&format!("FROM {PINNED}\nRUN {cmd}\n"));
        assert_eq!(run("curl -k https://x"), one(TLS_OFF, 2));
        assert_eq!(
            run("apk add curl && curl -fsSLk https://x"),
            one(TLS_OFF, 2)
        );
        assert_eq!(run("/usr/bin/curl --insecure https://x"), one(TLS_OFF, 2));
        assert_eq!(
            run("wget --no-check-certificate https://x"),
            one(TLS_OFF, 2)
        );
        assert_eq!(run("[\"curl\", \"-k\", \"https://x\"]"), one(TLS_OFF, 2));
        assert_eq!(
            rules(&format!(
                "FROM {PINNED}\nRUN set -e; \\\n    curl \\\n    -k https://x\n"
            )),
            one(TLS_OFF, 2)
        );
        assert_eq!(run("curl -fsSL https://x; ls -k"), []);
        assert_eq!(run("tar -xkf a.tar"), []);
    }

    #[test]
    fn heredoc_bodies_are_not_instructions() {
        let source =
            format!("FROM {PINNED}\nRUN <<EOF\nFROM busybox\ncurl -k https://x\nEOF\nRUN true\n");
        assert_eq!(rules(&source), one(TLS_OFF, 2));
    }

    #[test]
    fn the_escape_directive_changes_the_continuation() {
        let source = format!("# escape=`\nFROM {PINNED}\nRUN curl `\n  -k https://x\n");
        assert_eq!(rules(&source), one(TLS_OFF, 3));
    }
}
