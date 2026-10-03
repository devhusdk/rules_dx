pub mod biome;
pub mod buf;
pub mod buildifier;
pub mod checkstyle;
pub mod clang_format;
pub mod clang_tidy;
pub mod cppcheck;
pub mod csharpier;
pub mod cue;
pub mod djlint;
pub mod errcheck;
pub mod error_prone;
pub mod eslint;
pub mod fantomas;
pub mod flake8;
pub mod fsharplint;
pub mod gofumpt;
pub mod google_java_format;
pub mod govet;
pub mod jsonnetfmt;
pub mod keep_sorted;
pub mod ktfmt;
pub mod ktlint;
pub mod markdown;
pub mod modfmt;
pub mod pkl;
pub mod pmd;
pub mod prettier;
pub mod psscriptanalyzer;
pub mod pydoclint;
pub mod pylint;
pub mod qmlformat;
pub mod qmllint;
pub mod roslyn;
pub mod rubocop;
pub mod ruff;
pub mod rust;
pub mod rustfmt;
pub mod sarif;
pub mod scalafix;
pub mod scalafmt;
pub mod shellcheck;
pub mod shfmt;
pub mod spotbugs;
pub mod standardrb;
pub mod staticcheck;
pub mod stylelint;
pub mod taplo;
pub mod terraform;
pub mod tsc;
pub mod ty;
pub mod vale;
pub mod yamlfmt;
pub mod yamllint;

pub use biome::{parse_biome_format, parse_biome_lint};
pub use buf::{parse_buf_format, parse_buf_lint};
pub use buildifier::parse_buildifier;
pub use checkstyle::parse_checkstyle;
pub use clang_format::parse_clang_format;
pub use clang_tidy::parse_clang_tidy;
pub use cppcheck::parse_cppcheck;
pub use csharpier::parse_csharpier;
pub use cue::parse_cue;
pub use djlint::{parse_djlint, parse_djlint_format};
pub use errcheck::parse_errcheck;
pub use error_prone::parse_error_prone;
pub use eslint::parse_eslint;
pub use fantomas::parse_fantomas;
pub use flake8::parse_flake8;
pub use fsharplint::parse_fsharplint;
pub use gofumpt::parse_gofumpt;
pub use google_java_format::parse_google_java_format;
pub use govet::parse_govet;
pub use jsonnetfmt::parse_jsonnetfmt;
pub use keep_sorted::parse_keep_sorted;
pub use ktfmt::parse_ktfmt;
pub use ktlint::parse_ktlint;
pub use markdown::parse_markdown_findings;
pub use modfmt::parse_modfmt;
pub use pkl::parse_pkl;
pub use pmd::parse_pmd;
pub use prettier::parse_prettier_check;
pub use psscriptanalyzer::parse_psscriptanalyzer;
pub use pydoclint::parse_pydoclint;
pub use pylint::parse_pylint;
pub use qmlformat::parse_qmlformat;
pub use qmllint::parse_qmllint;
pub use roslyn::parse_roslyn;
pub use rubocop::parse_rubocop;
pub use ruff::{parse_ruff, parse_ruff_format};
pub use rust::{parse_clippy, parse_rustc};
pub use rustfmt::parse_rustfmt;
pub use scalafix::parse_scalafix;
pub use scalafmt::parse_scalafmt;
pub use shellcheck::parse_shellcheck;
pub use shfmt::parse_shfmt;
pub use spotbugs::parse_spotbugs;
pub use standardrb::parse_standardrb;
pub use staticcheck::parse_staticcheck;
pub use stylelint::parse_stylelint;
pub use taplo::{parse_taplo_format_check, parse_taplo_lint};
pub use terraform::parse_terraform;
pub use tsc::parse_tsc;
pub use ty::parse_ty;
pub use vale::parse_vale;
pub use yamlfmt::parse_yamlfmt;
pub use yamllint::parse_yamllint;

use std::path::Path;

use crate::{Finding, TextPosition, ToolSeverity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFinding {
    pub file: String,
    pub finding: Finding,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{tool} output is not the pinned JSON grammar: {detail}")]
    Json { tool: &'static str, detail: String },
    #[error("{tool} output is outside the pinned grammar: {detail}")]
    Shape { tool: &'static str, detail: String },
    #[error("{tool} reported an unchecked file: {path}")]
    UnknownFile { tool: &'static str, path: String },
    #[error("vale needs a usable config: {detail}")]
    ValeConfig { detail: String },
    #[error("{tool} output exceeds max size {limit} bytes (got {bytes})")]
    TooLarge {
        tool: &'static str,
        bytes: usize,
        limit: usize,
    },
}

pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

pub fn check_output_size(tool: &'static str, bytes: &[u8]) -> Result<(), ParseError> {
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(ParseError::TooLarge {
            tool,
            bytes: bytes.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }
    Ok(())
}

fn known<'a>(tool: &'static str, files: &[&'a str], path: &str) -> Result<&'a str, ParseError> {
    files
        .iter()
        .find(|file| **file == path)
        .copied()
        .ok_or_else(|| ParseError::UnknownFile {
            tool,
            path: path.to_owned(),
        })
}

/// The checked file a tool named, accepting a path spelled from the tool's working directory.
///
/// `terraform fmt -check` names each file it would rewrite by its path from the directory it
/// ran in, while the runner hands every tool an absolute path. A reported path therefore
/// matches when it equals a checked path outright or when it is that path below `cwd`. Only
/// paths below `cwd` can match, so a reported path that climbs out of the run names no file
/// the run was given.
fn known_relative<'a>(
    tool: &'static str,
    files: &[&'a str],
    cwd: &Path,
    path: &str,
) -> Result<&'a str, ParseError> {
    files
        .iter()
        .copied()
        .find(|file| {
            let file = Path::new(file);
            file == Path::new(path)
                || file
                    .strip_prefix(cwd)
                    .is_ok_and(|rest| rest == Path::new(path))
        })
        .ok_or_else(|| ParseError::UnknownFile {
            tool,
            path: path.to_owned(),
        })
}

/// The checked file a tool named, accepting either separator spelling.
///
/// `pydoclint` reports every path through `PurePath.as_posix()`, so on Windows it
/// answers `C:/tmp/a.py` for the `C:\tmp\a.py` the runner handed it. A reported
/// path matches when it equals a checked path once both are spelled with `/`.
/// Comparing both sides that way stays exact: a path the run was never given
/// still matches nothing.
fn known_spelled<'a>(
    tool: &'static str,
    files: &[&'a str],
    path: &str,
) -> Result<&'a str, ParseError> {
    let wanted = dx_path::posix(Path::new(path));
    files
        .iter()
        .copied()
        .find(|file| dx_path::posix(Path::new(file)) == wanted)
        .ok_or_else(|| ParseError::UnknownFile {
            tool,
            path: path.to_owned(),
        })
}

/// The lines a tool wrote, blank ones dropped and each one trimmed.
fn lines<'a>(tool: &'static str, raw: &'a [u8]) -> Result<Vec<&'a str>, ParseError> {
    Ok(as_text(tool, raw)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect())
}

/// The checked file a line names, with everything after the first colon.
fn located<'a>(
    tool: &'static str,
    line: &'a str,
    files: &[&'a str],
) -> Result<(&'a str, &'a str), ParseError> {
    let (path, rest) = line
        .split_once(':')
        .ok_or_else(|| missing(tool, "location", line))?;
    Ok((known(tool, files, path)?, rest))
}

/// The line and column of a diagnostic that spells both, with its tail.
fn columned<'a>(
    tool: &'static str,
    line: &str,
    rest: &'a str,
) -> Result<(u64, u64, &'a str), ParseError> {
    let mut parts = rest.splitn(3, ':');
    let line_no = numbered(tool, "line", line, parts.next())?;
    let column = numbered(tool, "column", line, parts.next())?;
    let tail = parts
        .next()
        .ok_or_else(|| missing(tool, "message", line))?
        .trim();
    Ok((line_no, column, tail))
}

/// A number a diagnostic spelled, or the shape failure naming the field it missed.
fn numbered(
    tool: &'static str,
    what: &str,
    line: &str,
    text: Option<&str>,
) -> Result<u64, ParseError> {
    text.ok_or_else(|| missing(tool, what, line))?
        .trim()
        .parse()
        .map_err(|_| missing(tool, what, line))
}

/// The rule and message of a `[rule] message` tail that carries both.
fn bracketed(tail: &str) -> (String, String) {
    match tail.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((rule, message)) if !rule.is_empty() && !message.trim().is_empty() => {
            (rule.trim().to_owned(), message.trim().to_owned())
        }
        _ => (String::new(), tail.to_owned()),
    }
}

/// A finding at a line and column, spanning nothing and suggesting nothing.
fn finding(
    tool: &'static str,
    file: &str,
    rule_id: String,
    message: String,
    severity: ToolSeverity,
    line: u64,
    column: u64,
) -> FileFinding {
    let (start, end) = point(line, column);
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: tool.to_owned(),
            rule_id,
            message,
            severity,
            start,
            end,
            suggestions: Vec::new(),
        },
    }
}

/// An exit that reported nothing is a shape failure, whatever the code said.
fn require_findings(
    tool: &'static str,
    findings: &[FileFinding],
    code: Option<i32>,
) -> Result<(), ParseError> {
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
    Ok(())
}

fn point(line: u64, column: u64) -> (TextPosition, Option<TextPosition>) {
    (TextPosition { line, column }, None)
}

fn missing(tool: &'static str, what: &str, line: &str) -> ParseError {
    ParseError::Shape {
        tool,
        detail: format!("malformed {what}: {line}"),
    }
}

fn code_name(code: Option<i32>) -> String {
    code.map_or_else(|| "signal".to_owned(), |code| code.to_string())
}

const DEV_NULL: &str = "/dev/null";

/// The paths a unified diff says were reformatted, in the order the diff names them.
///
/// Real producers disagree on the header. `git diff` writes `--- a/x` / `+++ b/x`;
/// `gofumpt -d`, `shfmt -d`, `cue fmt --diff` and `buf format --diff` all write
/// `--- x.orig` / `+++ x`, and `buf` adds a tab-separated timestamp. A header is a
/// `---`/`+++` pair, and only the new side names a file that exists after the run,
/// so that side wins and the old side covers a deletion. A leading `a/` or `b/` is a
/// `git diff` prefix only when the pair uses both; otherwise it is a real directory.
/// A trailing `.orig` likewise belongs to the producer that writes one, so it comes
/// off the old side only, and never off a `git diff` pair.
fn diff_paths(text: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        let old = lines[index].strip_prefix("--- ");
        index += 1;
        if let Some((old_count, new_count)) = hunk_counts(lines[index - 1]) {
            index = hunk_end(&lines, index, old_count, new_count);
            continue;
        }
        let Some(old) = old else {
            continue;
        };
        let new = index < lines.len() && lines[index].starts_with("+++ ");
        let new = if new {
            index += 1;
            Some(&lines[index - 1][4..])
        } else {
            None
        };
        let git = git_prefixed(Some(old), new);
        let path = match new {
            Some(new) => diff_path(new, git, false).or_else(|| diff_path(old, git, !git)),
            None => diff_path(old, git, !git),
        };
        if let Some(path) = path {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

/// The old-side and new-side line counts a `@@` header declares, absent counts meaning 1.
///
/// A unified diff may name the enclosing section after the counts, as `git diff` does for
/// every Go, C, Java or Ruby hunk: `@@ -8,5 +8,5 @@ func main() {`. Everything past the
/// closing ` @@` is that name, so the counts end at the ` @@` and not at the end of the line.
fn hunk_counts(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let new = rest.split_once(" @@").map_or(rest, |(new, _)| new);
    Some((range_count(old)?, range_count(new)?))
}

fn range_count(range: &str) -> Option<usize> {
    match range.split_once(',') {
        Some((_, count)) => count.parse().ok(),
        None => Some(1),
    }
}

/// The index just past the hunk that starts at `index` with the given side counts.
///
/// The counts are what end a hunk, not the line prefixes: a removed line whose own
/// text starts with `-` or `--- ` is body, and the next file's header must not be eaten.
fn hunk_end(lines: &[&str], mut index: usize, old_count: usize, new_count: usize) -> usize {
    let mut old_seen = 0;
    let mut new_seen = 0;
    while index < lines.len() && (old_seen < old_count || new_seen < new_count) {
        match lines[index].as_bytes().first() {
            Some(b' ') => {
                old_seen += 1;
                new_seen += 1;
            }
            Some(b'-') => old_seen += 1,
            Some(b'+') => new_seen += 1,
            Some(b'\\') => {}
            _ => break,
        }
        index += 1;
    }
    index
}
/// Whether the `a/` and `b/` in a header are `git diff` prefixes rather than directories.
///
/// `git diff` writes `a/x` and `b/x`, but a repo may hold a real `a/` directory, and the
/// `.orig` producers name the path itself: `gofumpt -d a/x.go` writes `a/x.go.orig`. So the
/// prefixes are git's only when the pair uses `a/` and `b/` together, or when the side that
/// names a real file is prefixed and the other side is the `/dev/null` of a deletion or a
/// creation. A lone `---` line proves neither, so it keeps its path.
fn git_prefixed(old: Option<&str>, new: Option<&str>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) if new == DEV_NULL => old.starts_with("a/"),
        (Some(old), Some(new)) if old == DEV_NULL => new.starts_with("b/"),
        (Some(old), Some(new)) => old.starts_with("a/") && new.starts_with("b/"),
        _ => false,
    }
}

/// The path one side of a diff header names.
///
/// `git` marks an `a/`/`b/` prefix as `git diff`'s, and `orig` marks the `<path>.orig` copy
/// that `gofumpt`, `shfmt`, `cue`, `djlint` and `buf` leave for the pre-run text.
fn diff_path(raw: &str, git: bool, orig: bool) -> Option<String> {
    let path = raw.split('\t').next().unwrap_or(raw).trim();
    let path = if git {
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(path)
    } else {
        path
    };
    let path = if orig {
        path.strip_suffix(".orig").unwrap_or(path)
    } else {
        path
    };
    if path.is_empty() || path == DEV_NULL {
        return None;
    }
    Some(path.to_owned())
}

/// The exit codes a diff-reporting formatter is allowed to exit with when it named files.
#[derive(Debug, Clone, Copy)]
pub enum DiffExit {
    /// `-d`-style tools print a diff and still exit 0.
    Zero,
    /// `--check`-style tools exit non-zero once the diff exists.
    NonZero,
    /// `--check --diff` tools exit 1, but 0 is also pinned.
    ZeroOrOne,
    /// The pinned tool pins no exit code beyond the clean case.
    Unpinned,
}

impl DiffExit {
    /// Whether the tool may exit with `code` once it named files.
    fn allows(self, code: Option<i32>) -> bool {
        match self {
            Self::Zero => code == Some(0),
            Self::NonZero => code != Some(0),
            Self::ZeroOrOne => code == Some(0) || code == Some(1),
            Self::Unpinned => true,
        }
    }
}

/// Report the offenses in the RuboCop JSON grammar, which `rubocop` and `standardrb` share.
///
/// `standardrb` wraps RuboCop and its `--format json` is RuboCop's own JSON formatter, so both
/// tools name every offense with the same `cop_name`, `severity` and `location` fields.
pub(super) fn rubocop_json(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stdout)?;
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool,
            detail: err.to_string(),
        })?;
    let entries = value
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ParseError::Shape {
            tool,
            detail: "missing files".to_owned(),
        })?;
    let mut findings = Vec::new();
    for entry in entries {
        let path = entry
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::Shape {
                tool,
                detail: "missing path".to_owned(),
            })?;
        let checked = known(tool, files, path)?;
        let offenses = entry
            .get("offenses")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ParseError::Shape {
                tool,
                detail: "missing offenses".to_owned(),
            })?;
        for off in offenses {
            let line = off
                .get("location")
                .and_then(|l| l.get("line"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ParseError::Shape {
                    tool,
                    detail: "missing line".to_owned(),
                })?;
            let col = off
                .get("location")
                .and_then(|l| l.get("column"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ParseError::Shape {
                    tool,
                    detail: "missing column".to_owned(),
                })?;
            let message = off
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if message.is_empty() {
                return Err(ParseError::Shape {
                    tool,
                    detail: "missing message".to_owned(),
                });
            }
            let rule = off
                .get("cop_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let severity = match off.get("severity").and_then(|v| v.as_str()) {
                Some("error") | Some("fatal") => ToolSeverity::Error,
                Some("warning") | Some("convention") | Some("refactor") => ToolSeverity::Warning,
                other => {
                    return Err(ParseError::Shape {
                        tool,
                        detail: format!("unknown severity {other:?}"),
                    })
                }
            };
            if line == 0 || col == 0 {
                return Err(ParseError::Shape {
                    tool,
                    detail: "positions must be nonzero".to_owned(),
                });
            }
            let (start, end) = point(line, col);
            findings.push(FileFinding {
                file: checked.to_owned(),
                finding: Finding {
                    tool_id: tool.to_owned(),
                    rule_id: rule,
                    message,
                    severity,
                    start,
                    end,
                    suggestions: Vec::new(),
                },
            });
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no diagnostics", code_name(code)),
        });
    }
    Ok(findings)
}

/// Report one "file is not formatted" finding per path in a unified diff.
fn diff_format(
    tool: &'static str,
    bytes: &[u8],
    code: Option<i32>,
    files: &[&str],
    exit: DiffExit,
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(tool, bytes)?;
    let paths = diff_paths(text);
    if paths.is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no diff markers", code_name(code)),
        });
    }
    if !exit.allows(code) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with diff markers", code_name(code)),
        });
    }
    unformatted(tool, files, paths)
}

/// Report one "file is not formatted" finding per path a tool listed, one per line.
///
/// `jsonnetfmt --test`, `pkl format --diff-name-only` and `modfmt -c -l` name the files
/// they would reformat instead of printing a diff. `yamlfmt -lint -q` writes the same list
/// to stderr. The exit code is not pinned: `modfmt -l` lists and exits 0, while `pkl`
/// exits 11, so only an empty list needs one.
pub(super) fn listed_paths(
    tool: &'static str,
    bytes: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(tool, bytes)?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !paths.contains(&trimmed.to_owned()) {
            paths.push(trimmed.to_owned());
        }
    }
    if paths.is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no paths", code_name(code)),
        });
    }
    unformatted(tool, files, paths)
}

/// The bytes as UTF-8 text, or a shape error naming the tool.
fn as_text<'a>(tool: &'static str, bytes: &'a [u8]) -> Result<&'a str, ParseError> {
    check_output_size(tool, bytes)?;
    std::str::from_utf8(bytes).map_err(|err| ParseError::Shape {
        tool,
        detail: err.to_string(),
    })
}

/// One "file is not formatted" finding per path, each path held to the run's own file set.
fn unformatted(
    tool: &'static str,
    files: &[&str],
    paths: Vec<String>,
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::with_capacity(paths.len());
    for path in paths {
        let checked = known(tool, files, &path)?;
        let (start, end) = point(1, 1);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: tool.to_owned(),
                rule_id: String::new(),
                message: "file is not formatted".to_owned(),
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::{
        bracketed, check_output_size, columned, finding, lines, located, missing, numbered,
        require_findings, FileFinding, ParseError, MAX_OUTPUT_BYTES,
    };
    use crate::{Finding, ToolSeverity};

    #[test]
    fn lines_drops_blank_lines_and_trims_the_rest() {
        assert_eq!(
            lines("tsc", b"  a.rs:1:1: x  \n\n\t\n b.rs:2:2: y\n").expect("read"),
            vec!["a.rs:1:1: x", "b.rs:2:2: y"]
        );
        assert!(lines("tsc", b"\n \n").expect("read").is_empty());
        assert!(matches!(
            lines("tsc", &[0xff]),
            Err(ParseError::Shape { tool: "tsc", .. })
        ));
        let big = vec![b'x'; MAX_OUTPUT_BYTES + 1];
        assert!(matches!(
            lines("tsc", &big),
            Err(ParseError::TooLarge { tool: "tsc", .. })
        ));
    }

    #[test]
    fn located_splits_the_path_and_holds_it_to_the_run() {
        let files = ["/s/a.rs"];
        assert_eq!(
            located("tsc", "/s/a.rs:1:2: x", &files).expect("located"),
            ("/s/a.rs", "1:2: x")
        );
        assert!(matches!(
            located("tsc", "/s/b.rs:1:2: x", &files),
            Err(ParseError::UnknownFile { path, .. }) if path == "/s/b.rs"
        ));
        assert!(located("tsc", "no-colon-here", &files)
            .expect_err("no location")
            .to_string()
            .contains("malformed location: no-colon-here"));
    }

    #[test]
    fn columned_splits_the_line_the_column_and_the_tail() {
        assert_eq!(
            columned(
                "tsc",
                "/s/a.rs:3:4: tail: with colons",
                "3:4: tail: with colons"
            )
            .expect("columned"),
            (3, 4, "tail: with colons")
        );
        assert_eq!(
            columned("tsc", "/s/a.rs: 3 :4:x", " 3 :4:x").expect("columned"),
            (3, 4, "x")
        );
        for (rest, field) in [
            ("x:4:tail", "line"),
            ("3:x:tail", "column"),
            ("3:4", "message"),
        ] {
            assert!(
                columned("tsc", "/s/a.rs:3:4:t", rest)
                    .expect_err("malformed")
                    .to_string()
                    .contains(&format!("malformed {field}")),
                "{rest} must fail as a missing {field}"
            );
        }
    }

    #[test]
    fn numbered_names_the_field_it_could_not_read() {
        assert_eq!(
            numbered("tsc", "line", "l", Some(" 12 ")).expect("read"),
            12
        );
        assert_eq!(
            numbered("tsc", "line", "l", Some(""))
                .expect_err("empty")
                .to_string(),
            missing("tsc", "line", "l").to_string()
        );
        assert_eq!(
            numbered("tsc", "line", "l", None)
                .expect_err("absent")
                .to_string(),
            missing("tsc", "line", "l").to_string()
        );
        assert_eq!(
            numbered("tsc", "line", "l", Some("x"))
                .expect_err("text")
                .to_string(),
            missing("tsc", "line", "l").to_string()
        );
    }

    #[test]
    fn bracketed_splits_a_rule_only_when_both_halves_are_there() {
        assert_eq!(
            bracketed("[R1] msg here"),
            ("R1".to_owned(), "msg here".to_owned())
        );
        assert_eq!(
            bracketed(" msg here "),
            (String::new(), " msg here ".to_owned())
        );
        assert_eq!(bracketed("[R1]"), (String::new(), "[R1]".to_owned()));
        assert_eq!(bracketed("[R1]   "), (String::new(), "[R1]   ".to_owned()));
        assert_eq!(bracketed("[] msg"), (String::new(), "[] msg".to_owned()));
    }

    #[test]
    fn finding_carries_the_tool_the_position_and_nothing_else() {
        let found = finding(
            "tsc",
            "/s/a.rs",
            "R1".to_owned(),
            "msg".to_owned(),
            ToolSeverity::Error,
            3,
            4,
        );
        assert_eq!(found.file, "/s/a.rs");
        assert_eq!(
            found.finding,
            Finding {
                tool_id: "tsc".to_owned(),
                rule_id: "R1".to_owned(),
                message: "msg".to_owned(),
                severity: ToolSeverity::Error,
                start: crate::TextPosition { line: 3, column: 4 },
                end: None,
                suggestions: Vec::new(),
            }
        );
    }

    #[test]
    fn require_findings_rejects_only_a_silent_non_zero_exit() {
        assert!(require_findings("tsc", &[], Some(0)).is_ok());
        assert!(require_findings("tsc", &[], None).is_err());
        assert!(require_findings("tsc", &[], Some(1)).is_err());
        let one: Vec<FileFinding> = vec![finding(
            "tsc",
            "/s/a.rs",
            String::new(),
            "m".to_owned(),
            ToolSeverity::Warning,
            1,
            1,
        )];
        assert!(require_findings("tsc", &one, Some(1)).is_ok());
        assert!(require_findings("tsc", &[], Some(2))
            .expect_err("silent exit")
            .to_string()
            .contains("exit 2 with no diagnostics"));
    }

    #[test]
    fn error_display_is_stable() {
        assert!(ParseError::Json {
            tool: "vale",
            detail: "x".to_owned()
        }
        .to_string()
        .contains("vale"));
        assert!(ParseError::UnknownFile {
            tool: "taplo",
            path: "/s/x".to_owned()
        }
        .to_string()
        .contains("/s/x"));
        assert!(ParseError::ValeConfig {
            detail: "E100".to_owned()
        }
        .to_string()
        .contains("E100"));
        assert!(ParseError::TooLarge {
            tool: "sarif",
            bytes: MAX_OUTPUT_BYTES + 1,
            limit: MAX_OUTPUT_BYTES,
        }
        .to_string()
        .contains("max size"));
    }

    #[test]
    fn output_size_guard_rejects_oversized() {
        assert!(check_output_size("tsc", b"ok").is_ok());
        assert!(check_output_size("tsc", &vec![b'x'; MAX_OUTPUT_BYTES]).is_ok());
        let big = vec![b'x'; MAX_OUTPUT_BYTES + 1];
        assert_eq!(
            check_output_size("tsc", &big),
            Err(ParseError::TooLarge {
                tool: "tsc",
                bytes: big.len(),
                limit: MAX_OUTPUT_BYTES,
            })
        );
        assert!(super::tsc::parse_tsc(&big, Some(2), &["/s/a.ts"]).is_err());
        assert!(super::sarif::parse_sarif("sarif-test", &big, Some(1), &["/s/a.java"]).is_err());
    }

    #[test]
    fn diff_headers_resolve_every_real_producer_shape() {
        // git diff, and the `a/` prefix the fixtures were written with.
        assert_eq!(
            super::diff_paths("--- a/x.go\n+++ b/x.go\n@@ -1 +1 @@\n-a\n+b\n"),
            ["x.go"]
        );
        // gofumpt -d, shfmt -d, cue fmt --diff: the OLD side is "<path>.orig".
        assert_eq!(
            super::diff_paths(
                "diff x.go.orig x.go\n--- x.go.orig\n+++ x.go\n@@ -1 +1 @@\n-a\n+b\n"
            ),
            ["x.go"]
        );
        // buf format --diff: `.orig` plus a tab-separated timestamp, from `diff -u`.
        assert_eq!(
            super::diff_paths(
                "diff -u x.proto.orig x.proto\n--- x.proto.orig\t2024-01-02 03:04:05.0 +0000\n+++ x.proto\t2024-01-02 03:04:06.0 +0000\n@@ -1 +1 @@\n-a\n+b\n"
            ),
            ["x.proto"]
        );
        // A hunk may remove a line whose own text starts with `-- `; that is body, not a header.
        assert_eq!(
            super::diff_paths(
                "--- a/q.sql\n+++ b/q.sql\n@@ -1,2 +1,2 @@\n CREATE TABLE t (id int);\n--- drop the old table\n+-- drop the legacy table\n"
            ),
            ["q.sql"]
        );
        // `git diff` names the enclosing section after the counts, and every Go, C, Java,
        // Ruby or Python hunk has one. The counts still end the hunk there.
        assert_eq!(
            super::diff_paths(
                "--- a/k.go\n+++ b/k.go\n@@ -5,5 +5,5 @@ import \"fmt\"\n func main() {\n \tfmt.Println(\"a\")\n \tfmt.Println(\"b\")\n--- keep\n+--- other\n }\n"
            ),
            ["k.go"]
        );
        // A second file after a hunk is still found.
        assert_eq!(
            super::diff_paths(
                "--- a/x.go.orig\n+++ x.go\n@@ -1 +1 @@\n-a\n+b\n--- a/y.go.orig\n+++ y.go\n@@ -1 +1 @@\n-c\n+d\n"
            ),
            ["x.go", "y.go"]
        );
        // A blank line inside a hunk ends it, so the next header is a header and not body.
        assert_eq!(
            super::diff_paths(
                "--- a/x.go\n+++ b/x.go\n@@ -1,2 +1,2 @@\n-a\n\n+b\n--- a/y.go\n+++ b/y.go\n@@ -1 +1 @@\n-c\n+d\n"
            ),
            ["x.go", "y.go"]
        );
        // `/dev/null` on the new side is a deletion, so the old side names the file.
        assert_eq!(
            super::diff_paths("--- a/gone.go\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n"),
            ["gone.go"]
        );
        // A deletion from a producer that also names the old side `<path>.orig`.
        assert_eq!(
            super::diff_paths("--- gone.go.orig\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n"),
            ["gone.go"]
        );
        // Whichever side is not `/dev/null` names the file, whichever side that is.
        assert_eq!(
            super::diff_paths("--- /dev/null\n+++ b/new.go\n@@ -0,0 +1 @@\n+a\n"),
            ["new.go"]
        );
        // A bare `.orig` reduces to nothing rather than an empty path.
        assert!(super::diff_paths("--- .orig\n@@ -1 +1 @@\n-a\n+b\n").is_empty());
        // A `git diff` pair names the file, so a real `.orig` file is a path like any other.
        assert_eq!(
            super::diff_paths("--- a/.orig\n+++ b/.orig\n@@ -1 +1 @@\n-a\n+b\n"),
            [".orig"]
        );
    }

    #[test]
    fn diff_headers_keep_a_file_whose_own_name_ends_in_orig() {
        // `git diff` of a file that really is called `x.orig`: stripping the suffix would name
        // `x`, a file the run was never given.
        assert_eq!(
            super::diff_paths("--- a/patch.orig\n+++ b/patch.orig\n@@ -1 +1 @@\n-a\n+b\n"),
            ["patch.orig"]
        );
        // A `git diff` deletion of that file keeps the name too.
        assert_eq!(
            super::diff_paths("--- a/patch.orig\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n"),
            ["patch.orig"]
        );
        // `gofumpt -d patch.orig` keeps its own copy at `patch.orig.orig`, so the new side is
        // the file and the old side reduces to it.
        assert_eq!(
            super::diff_paths(
                "diff patch.orig.orig patch.orig\n--- patch.orig.orig\n+++ patch.orig\n@@ -1 +1 @@\n-a\n+b\n"
            ),
            ["patch.orig"]
        );
        // A `.orig` producer's deletion still resolves, because that side is the copy.
        assert_eq!(
            super::diff_paths("--- patch.orig.orig\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n"),
            ["patch.orig"]
        );
        // The producers that write `<path>.orig` write it on the old side only, so the new side
        // of a plain pair is already the file.
        assert_eq!(
            super::diff_paths("--- run.sh.orig\n+++ run.sh\n@@ -1 +1 @@\n-a\n+b\n"),
            ["run.sh"]
        );
    }

    #[test]
    fn diff_headers_keep_a_real_a_directory_that_is_not_a_git_prefix() {
        // `gofumpt -d a/x.go` names the path, so `a/` is a directory here, and both sides
        // carry it: stripping it would name `x.go`, a file the run was never given.
        assert_eq!(
            super::diff_paths(
                "diff a/x.go.orig a/x.go\n--- a/x.go.orig\n+++ a/x.go\n@@ -1 +1 @@\n-a\n+b\n"
            ),
            ["a/x.go"]
        );
        // One side alone is not a `git diff` header either.
        assert_eq!(
            super::diff_paths("--- a/x.go\n@@ -1 +1 @@\n-a\n+b\n"),
            ["a/x.go"]
        );
        assert_eq!(
            super::diff_paths("--- a/x.go.orig\n+++ a/x.go\n@@ -1 +1 @@\n-a\n+b\n"),
            ["a/x.go"]
        );
        // A `b/` directory the same way, from the `.orig` side.
        assert_eq!(
            super::diff_paths("--- b/x.go.orig\n+++ b/x.go\n@@ -1 +1 @@\n-a\n+b\n"),
            ["b/x.go"]
        );
        // The git shape still strips: only a pair using `a/` and `b/` is a prefix.
        assert_eq!(
            super::diff_paths("--- a/a/x.go\n+++ b/a/x.go\n@@ -1 +1 @@\n-a\n+b\n"),
            ["a/x.go"]
        );
    }

    #[test]
    fn diff_format_rejects_a_header_naming_a_file_the_run_never_gave() {
        // The old reader stripped `a/` and named `x.go`, so a real `a/x.go` passed.
        let diff = b"--- a/x.go.orig\n+++ a/x.go\n@@ -1 +1 @@\n-a\n+b\n";
        let err = super::diff_format("t", diff, Some(0), &["x.go"], super::DiffExit::Zero)
            .expect_err("a/x.go was never checked");
        assert!(matches!(err, super::ParseError::UnknownFile { path, .. } if path == "a/x.go"));
        let findings = super::diff_format("t", diff, Some(0), &["a/x.go"], super::DiffExit::Zero)
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "a/x.go");
    }

    #[test]
    fn diff_format_reports_each_named_file_once() {
        let diff = b"--- a/x.go.orig\n+++ x.go\n@@ -1 +1 @@\n-a\n+b\n";
        let findings = super::diff_format("t", diff, Some(0), &["x.go"], super::DiffExit::Zero)
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "x.go");
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn diff_format_holds_each_tool_to_its_own_exit_code() {
        let diff = b"--- a/x.go.orig\n+++ x.go\n@@ -1 +1 @@\n-a\n+b\n";
        for (exit, code, ok) in [
            (super::DiffExit::Zero, Some(0), true),
            (super::DiffExit::Zero, Some(1), false),
            (super::DiffExit::NonZero, Some(1), true),
            (super::DiffExit::NonZero, Some(0), false),
            (super::DiffExit::ZeroOrOne, Some(0), true),
            (super::DiffExit::ZeroOrOne, Some(1), true),
            (super::DiffExit::ZeroOrOne, Some(3), false),
            (super::DiffExit::Unpinned, Some(0), true),
            (super::DiffExit::Unpinned, Some(1), true),
        ] {
            let name = format!("{exit:?}");
            let got = super::diff_format("t", diff, code, &["x.go"], exit).is_ok();
            assert_eq!(got, ok, "{name} at {code:?}");
        }
        // A clean exit with no diff is fine for every policy.
        for exit in [
            super::DiffExit::Zero,
            super::DiffExit::NonZero,
            super::DiffExit::ZeroOrOne,
            super::DiffExit::Unpinned,
        ] {
            assert!(super::diff_format("t", b"", Some(0), &["x.go"], exit)
                .expect("clean")
                .is_empty());
        }
    }

    fn xorshift(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    const FUZZ_FILES: &[&str] = &[
        "/s/a.ts",
        "/s/a.java",
        "/s/a.js",
        "/s/a.bzl",
        "/s/x.toml",
        "/s/x.rs",
        "/s/a.py",
        "/s/x.proto",
        "/s/Hello.java",
        "/s/dirty.toml",
        "/s/a.c",
        "/s/a.cc",
        "/s/a.h",
        "/s/a.cs",
        "/s/a.fs",
        "/s/a.go",
        "/s/a.scala",
        "/s/a.kt",
        "/s/a.kts",
        "/s/a.rb",
        "/s/a.ex",
        "/s/a.exs",
        "/s/a.sh",
        "/s/a.ps1",
        "/s/a.qml",
        "/s/a.yaml",
        "/s/a.yml",
        "/s/a.tf",
        "/s/a.cue",
        "/s/a.libsonnet",
        "/s/a.pkl",
        "/s/a.css",
        "/s/a.html",
        "/s/a.txt",
        "/s/a.pp",
    ];

    const FUZZ_SEEDS: &[&[u8]] = &[
        b"",
        b"{}\n",
        b"[]\n",
        b"null\n",
        b"not json",
        b"/s/a.ts(1,1): error TS1234: msg\n",
        br#"{"version":"2.1.0","runs":[]}"#,
        br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"T"}},"results":[{"ruleId":"r","level":"error","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#,
        br#"[{"filePath":"/s/a.js","messages":[{"ruleId":"x","severity":2,"message":"m","line":1,"column":1}]}]"#,
        br#"{"success":false,"files":[{"filename":"/s/a.bzl","formatted":false,"valid":true,"warnings":[]}]}"#,
        br#"{"path":"/s/x.proto","start_line":1,"start_column":1,"type":"T","message":"m"}"#,
        br#"{"diagnostics":[{"severity":"error","message":"m","category":"c","location":{"path":"/s/a.js","start":{"line":1,"column":1},"end":{"line":1,"column":2}}}],"command":"lint"}"#,
        br#"{"files":[{"path":"/s/a.fs","status":"needs-formatting"}]}"#,
        br#"[{"type":"error","symbol":"s","message":"m","message-id":"E1","line":1,"column":0,"path":"/s/a.py"}]"#,
        br#"{"files":[{"path":"/s/a.rb","offenses":[{"message":"m","severity":"convention","cop_name":"c","location":{"line":1,"column":1}}]}]}"#,
        br#"[{"code":"S1000","severity":"warning","location":{"file":"/s/a.go","line":1,"column":1},"message":"m"}]"#,
        br#"[{"source":"/s/a.css","warnings":[{"line":1,"column":2,"rule":"r","text":"m","severity":"error"}]}]"#,
        br#"{"diagnostics":[{"file":"/s/a.qml","line":1,"message":"m"}]}"#,
        br#"{"path":"/s/a.scala","line":1,"column":1,"rule":"r","message":"m","severity":"error"}"#,
        b"path:1:1: E100 message\n",
        b"error: bad\n  \xe2\x94\x8c\xe2\x94\x80 /s/x.toml:1:5\n",
        b"Diff in /s/x.rs:1:\n-fn  main(){}\n+fn main() {}\n",
        b"--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-a\n+b\n",
        b"--- /dev/null\n+++ b/x.py\n@@ -0,0 +1 @@\n+a\n",
        b"/s/a.c:4:3: warning: msg [readability-else-after-return]\n",
        b"/s/a.c:4:3: note: msg\n",
        b"/s/a.go:10:5: unchecked error\n",
        b"/s/a.bzl:3: out of order\n",
        b"/s/a.py:1:1: E501 line too long\n",
        b"/s/a.py:1:1: C901 too complex\n",
        b"/s/a.yaml:3:1: msg here\n",
        b"/s/a.sh:1:1: msg here\n",
        b"/s/a.ps1:1:1: [Rule.Name] msg\n",
        b"/s/a.py\n    4: DOC101: msg\n",
        b"/s/a.py\n",
        b"/s/Hello.java:3: error: [DeadException] msg\n1 error\n",
        b"/s/a.c\n",
        b"./a.cs\n",
        b"<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><location file=\"/s/a.c\" line=\"1\"/></error></errors></results>",
        b"<results version=\"2\"></results>",
        b"{\n",
        b"\xff\xfe\x00",
    ];

    type FuzzFn = fn(&[u8], Option<i32>, &[&str]) -> Result<Vec<super::FileFinding>, ParseError>;

    fn fuzz_biome_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::biome::parse_biome_lint(input, code, files)
    }

    fn fuzz_biome_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::biome::parse_biome_format(input, code, files)
    }

    fn fuzz_buf_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::buf::parse_buf_lint(input, code, files)
    }

    fn fuzz_buf_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::buf::parse_buf_format(input, code, files)
    }

    fn fuzz_checkstyle(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::checkstyle::parse_checkstyle(input, code, files)
    }

    fn fuzz_clang_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::clang_format::parse_clang_format(input, code, files)
    }

    fn fuzz_clang_tidy(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::clang_tidy::parse_clang_tidy(input, code, files)
    }

    fn fuzz_cppcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::cppcheck::parse_cppcheck(input, code, files)
    }

    fn fuzz_csharpier(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::csharpier::parse_csharpier(input, code, files)
    }

    fn fuzz_cue(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::cue::parse_cue(input, code, files)
    }

    fn fuzz_djlint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::djlint::parse_djlint(input, code, files)
    }

    fn fuzz_djlint_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::djlint::parse_djlint_format(input, code, files)
    }

    fn fuzz_errcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::errcheck::parse_errcheck(input, code, files)
    }

    fn fuzz_error_prone(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::error_prone::parse_error_prone(input, input, code, files)
    }

    fn fuzz_eslint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::eslint::parse_eslint(input, code, files)
    }

    fn fuzz_fantomas(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::fantomas::parse_fantomas(input, code, files)
    }

    fn fuzz_flake8(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::flake8::parse_flake8(input, code, files)
    }

    fn fuzz_fsharplint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::fsharplint::parse_fsharplint(input, code, files)
    }

    fn fuzz_gofumpt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::gofumpt::parse_gofumpt(input, code, files)
    }

    fn fuzz_google_java_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::google_java_format::parse_google_java_format(input, code, files)
    }

    fn fuzz_govet(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::govet::parse_govet(input, code, files)
    }

    fn fuzz_jsonnetfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::jsonnetfmt::parse_jsonnetfmt(input, code, files)
    }

    fn fuzz_keep_sorted(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::keep_sorted::parse_keep_sorted(input, code, files)
    }

    fn fuzz_ktfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ktfmt::parse_ktfmt(input, code, files)
    }

    fn fuzz_ktlint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ktlint::parse_ktlint(input, code, files)
    }

    fn fuzz_markdown_findings(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::markdown::parse_markdown_findings(input, code, files)
    }

    fn fuzz_modfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::modfmt::parse_modfmt(input, code, files)
    }

    fn fuzz_pkl(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::pkl::parse_pkl(input, code, files)
    }

    fn fuzz_pmd(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::pmd::parse_pmd(input, code, files)
    }

    fn fuzz_prettier_check(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::prettier::parse_prettier_check(input, code, files)
    }

    fn fuzz_psscriptanalyzer(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::psscriptanalyzer::parse_psscriptanalyzer(input, code, files)
    }

    fn fuzz_pydoclint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::pydoclint::parse_pydoclint(input, code, files)
    }

    fn fuzz_pylint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::pylint::parse_pylint(input, code, files)
    }

    fn fuzz_qmlformat(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::qmlformat::parse_qmlformat(input, code, files)
    }

    fn fuzz_qmllint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::qmllint::parse_qmllint(input, code, files)
    }

    fn fuzz_roslyn(
        input: &[u8],
        _code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::roslyn::parse_roslyn(input, files)
    }

    fn fuzz_rubocop(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::rubocop::parse_rubocop(input, code, files)
    }

    fn fuzz_ruff(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ruff::parse_ruff(input, code, files)
    }

    fn fuzz_ruff_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ruff::parse_ruff_format(input, code, files)
    }

    fn fuzz_clippy(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::rust::parse_clippy(input, code, files)
    }

    fn fuzz_rustc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::rust::parse_rustc(input, code, files)
    }

    fn fuzz_buildifier(
        input: &[u8],
        _code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::buildifier::parse_buildifier(input, input, files)
    }

    fn fuzz_rustfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::rustfmt::parse_rustfmt(input, input, code, files)
    }

    fn fuzz_sarif(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::sarif::parse_sarif("fuzz", input, code, files)
    }

    fn fuzz_scalafix(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::scalafix::parse_scalafix(input, code, files)
    }

    fn fuzz_scalafmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::scalafmt::parse_scalafmt(input, code, files)
    }

    fn fuzz_shellcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::shellcheck::parse_shellcheck(input, code, files)
    }

    fn fuzz_shfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::shfmt::parse_shfmt(input, code, files)
    }

    fn fuzz_spotbugs(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::spotbugs::parse_spotbugs(input, code, files)
    }

    fn fuzz_standardrb(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::standardrb::parse_standardrb(input, code, files)
    }

    fn fuzz_staticcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::staticcheck::parse_staticcheck(input, code, files)
    }

    fn fuzz_stylelint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::stylelint::parse_stylelint(input, code, files)
    }

    fn fuzz_taplo_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::taplo::parse_taplo_lint(input, code, files)
    }

    fn fuzz_taplo_format_check(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::taplo::parse_taplo_format_check(input, code, files)
    }

    fn fuzz_terraform(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::terraform::parse_terraform(input, code, files, std::path::Path::new("/"))
    }

    fn fuzz_tsc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::tsc::parse_tsc(input, code, files)
    }

    fn fuzz_ty(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ty::parse_ty(input, code, files)
    }

    fn fuzz_vale(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::vale::parse_vale(input, code, files)
    }

    fn fuzz_yamlfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::yamlfmt::parse_yamlfmt(input, code, files)
    }

    fn fuzz_yamllint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::yamllint::parse_yamllint(input, code, files)
    }

    const FUZZ_PARSERS: &[(&str, FuzzFn)] = &[
        ("biome_lint", fuzz_biome_lint),
        ("biome_format", fuzz_biome_format),
        ("buf_lint", fuzz_buf_lint),
        ("buf_format", fuzz_buf_format),
        ("checkstyle", fuzz_checkstyle),
        ("clang_format", fuzz_clang_format),
        ("clang_tidy", fuzz_clang_tidy),
        ("cppcheck", fuzz_cppcheck),
        ("csharpier", fuzz_csharpier),
        ("cue", fuzz_cue),
        ("djlint", fuzz_djlint),
        ("djlint_format", fuzz_djlint_format),
        ("errcheck", fuzz_errcheck),
        ("error_prone", fuzz_error_prone),
        ("eslint", fuzz_eslint),
        ("fantomas", fuzz_fantomas),
        ("flake8", fuzz_flake8),
        ("fsharplint", fuzz_fsharplint),
        ("gofumpt", fuzz_gofumpt),
        ("google_java_format", fuzz_google_java_format),
        ("govet", fuzz_govet),
        ("jsonnetfmt", fuzz_jsonnetfmt),
        ("keep_sorted", fuzz_keep_sorted),
        ("ktfmt", fuzz_ktfmt),
        ("ktlint", fuzz_ktlint),
        ("parse_markdown_findings", fuzz_markdown_findings),
        ("modfmt", fuzz_modfmt),
        ("pkl", fuzz_pkl),
        ("pmd", fuzz_pmd),
        ("parse_prettier_check", fuzz_prettier_check),
        ("psscriptanalyzer", fuzz_psscriptanalyzer),
        ("pydoclint", fuzz_pydoclint),
        ("pylint", fuzz_pylint),
        ("qmlformat", fuzz_qmlformat),
        ("qmllint", fuzz_qmllint),
        ("roslyn", fuzz_roslyn),
        ("rubocop", fuzz_rubocop),
        ("ruff", fuzz_ruff),
        ("ruff_format", fuzz_ruff_format),
        ("clippy", fuzz_clippy),
        ("rustc", fuzz_rustc),
        ("buildifier", fuzz_buildifier),
        ("rustfmt", fuzz_rustfmt),
        ("sarif", fuzz_sarif),
        ("scalafix", fuzz_scalafix),
        ("scalafmt", fuzz_scalafmt),
        ("shellcheck", fuzz_shellcheck),
        ("shfmt", fuzz_shfmt),
        ("spotbugs", fuzz_spotbugs),
        ("standardrb", fuzz_standardrb),
        ("staticcheck", fuzz_staticcheck),
        ("stylelint", fuzz_stylelint),
        ("taplo_lint", fuzz_taplo_lint),
        ("taplo_format_check", fuzz_taplo_format_check),
        ("terraform", fuzz_terraform),
        ("tsc", fuzz_tsc),
        ("ty", fuzz_ty),
        ("vale", fuzz_vale),
        ("yamlfmt", fuzz_yamlfmt),
        ("yamllint", fuzz_yamllint),
    ];

    const PARSER_SOURCES: &[(&str, &str)] = &[
        ("biome", include_str!("biome.rs")),
        ("buf", include_str!("buf.rs")),
        ("buildifier", include_str!("buildifier.rs")),
        ("checkstyle", include_str!("checkstyle.rs")),
        ("clang_format", include_str!("clang_format.rs")),
        ("clang_tidy", include_str!("clang_tidy.rs")),
        ("cppcheck", include_str!("cppcheck.rs")),
        ("csharpier", include_str!("csharpier.rs")),
        ("cue", include_str!("cue.rs")),
        ("djlint", include_str!("djlint.rs")),
        ("errcheck", include_str!("errcheck.rs")),
        ("error_prone", include_str!("error_prone.rs")),
        ("eslint", include_str!("eslint.rs")),
        ("fantomas", include_str!("fantomas.rs")),
        ("flake8", include_str!("flake8.rs")),
        ("fsharplint", include_str!("fsharplint.rs")),
        ("gofumpt", include_str!("gofumpt.rs")),
        ("google_java_format", include_str!("google_java_format.rs")),
        ("govet", include_str!("govet.rs")),
        ("jsonnetfmt", include_str!("jsonnetfmt.rs")),
        ("keep_sorted", include_str!("keep_sorted.rs")),
        ("ktfmt", include_str!("ktfmt.rs")),
        ("ktlint", include_str!("ktlint.rs")),
        ("markdown", include_str!("markdown.rs")),
        ("modfmt", include_str!("modfmt.rs")),
        ("pkl", include_str!("pkl.rs")),
        ("pmd", include_str!("pmd.rs")),
        ("prettier", include_str!("prettier.rs")),
        ("psscriptanalyzer", include_str!("psscriptanalyzer.rs")),
        ("pydoclint", include_str!("pydoclint.rs")),
        ("pylint", include_str!("pylint.rs")),
        ("qmlformat", include_str!("qmlformat.rs")),
        ("qmllint", include_str!("qmllint.rs")),
        ("roslyn", include_str!("roslyn.rs")),
        ("rubocop", include_str!("rubocop.rs")),
        ("ruff", include_str!("ruff.rs")),
        ("rust", include_str!("rust.rs")),
        ("rustfmt", include_str!("rustfmt.rs")),
        ("sarif", include_str!("sarif.rs")),
        ("scalafix", include_str!("scalafix.rs")),
        ("scalafmt", include_str!("scalafmt.rs")),
        ("shellcheck", include_str!("shellcheck.rs")),
        ("shfmt", include_str!("shfmt.rs")),
        ("spotbugs", include_str!("spotbugs.rs")),
        ("standardrb", include_str!("standardrb.rs")),
        ("staticcheck", include_str!("staticcheck.rs")),
        ("stylelint", include_str!("stylelint.rs")),
        ("taplo", include_str!("taplo.rs")),
        ("terraform", include_str!("terraform.rs")),
        ("tsc", include_str!("tsc.rs")),
        ("ty", include_str!("ty.rs")),
        ("vale", include_str!("vale.rs")),
        ("yamlfmt", include_str!("yamlfmt.rs")),
        ("yamllint", include_str!("yamllint.rs")),
    ];

    fn declared_parser_names() -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (module, source) in PARSER_SOURCES {
            let mut rest: &str = source;
            while let Some(at) = rest.find("pub fn parse_") {
                rest = &rest[at + "pub fn ".len()..];
                let end = rest
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(rest.len());
                found.push(((*module).to_owned(), rest[..end].to_owned()));
                rest = &rest[end..];
            }
        }
        found
    }

    #[test]
    fn every_declared_parser_is_in_the_fuzz_table() {
        let declared = declared_parser_names();
        assert!(
            declared.len() >= 59,
            "expected the full parser set, got {declared:?}"
        );
        let bare: Vec<String> = declared
            .iter()
            .flat_map(|(module, name)| {
                [
                    module.clone(),
                    name.clone(),
                    name.trim_start_matches("parse_").to_owned(),
                ]
            })
            .collect();
        let mut missing: Vec<String> = declared
            .iter()
            .filter(|(_, name)| !bare.iter().any(|known| known == name))
            .map(|(_, name)| name.clone())
            .collect();
        missing.dedup();
        let covered: Vec<&str> = bare.iter().map(String::as_str).collect();
        let mut absent: Vec<String> = FUZZ_PARSERS
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .filter(|name| !covered.contains(&name.as_str()))
            .collect();
        absent.dedup();
        missing.extend(absent.iter().cloned());
        missing.sort();
        assert!(
            missing.is_empty(),
            "declared parsers and FUZZ_PARSERS rows that do not match: {missing:?}"
        );
        let mut stale: Vec<&str> = FUZZ_PARSERS
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !covered.contains(name))
            .collect();
        stale.sort_unstable();
        assert!(
            stale.is_empty(),
            "FUZZ_PARSERS rows naming no declared parser: {stale:?}"
        );
    }

    #[test]
    fn fuzz_parsers_never_panic_on_arbitrary_bytes() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for round in 0..200 {
            let seed = FUZZ_SEEDS[round % FUZZ_SEEDS.len()];
            let mut input = seed.to_vec();
            match xorshift(&mut state) % 3 {
                0 => {
                    let keep = (xorshift(&mut state) as usize) % (input.len() + 1);
                    input.truncate(keep);
                }
                1 => {
                    if !input.is_empty() {
                        let at = (xorshift(&mut state) as usize) % input.len();
                        input[at] ^= (xorshift(&mut state) & 0xFF) as u8;
                    }
                }
                _ => {
                    let extra = (xorshift(&mut state) % 32) as usize;
                    for _ in 0..extra {
                        input.push((xorshift(&mut state) & 0xFF) as u8);
                    }
                }
            }
            for (name, parse) in FUZZ_PARSERS {
                for code in [None, Some(0), Some(1), Some(2)] {
                    if let Err(error) = parse(&input, code, FUZZ_FILES) {
                        assert!(
                            !error.to_string().is_empty(),
                            "{name} produced an empty error for {input:?}"
                        );
                    }
                }
            }
        }
    }
}
