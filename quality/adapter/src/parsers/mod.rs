pub mod ansible_lint;
pub mod biome;
pub mod buf;
pub mod buildifier;
pub mod cfn_lint;
pub mod check_jsonschema;
pub mod checkmake;
pub mod checkstyle;
pub mod clang_format;
pub mod clang_syntax;
pub mod clang_tidy;
pub mod cmakelint;
pub mod cppcheck;
pub mod credo;
pub mod csharpier;
pub mod cue;
pub mod dart_analyze;
pub mod dart_format;
pub mod deadnix;
pub mod dfmt;
pub mod dialyzer;
pub mod djlint;
pub mod dscanner;
pub mod elvis;
pub mod erlfmt;
pub mod errcheck;
pub mod eslint;
pub mod fantomas;
pub mod flake8;
pub mod flang;
pub mod flatc;
pub mod fortitude;
pub mod fsc;
pub mod fsharplint;
pub mod gersemi;
mod gnu_diagnostics;
pub mod go_analysis;
pub mod gofumpt;
pub mod google_java_format;
pub mod govet;
pub mod gplint;
pub mod hadolint;
pub mod helm;
pub mod hlint;
pub mod jsonnet_lint;
pub mod jsonnetfmt;
pub mod julia_format;
pub mod jvm_diagnostics;
pub mod keep_sorted;
pub mod ktfmt;
pub mod ktlint;
pub mod kubeconform;
pub mod latexindent;
pub mod ldc;
pub mod luals;
pub mod markdown;
pub mod mbake;
pub mod mix_format;
pub mod modfmt;
pub mod nixfmt;
pub mod notebook_outputs;
pub mod nvcc_syntax;
pub mod opa;
pub mod ormolu;
pub mod perlcritic;
pub mod perltidy;
pub mod phpcs;
pub mod phpstan;
pub mod pkl;
pub mod pmd;
pub mod prettier;
pub mod psscriptanalyzer;
pub mod pydoclint;
pub mod pylint;
pub mod qmlformat;
pub mod qmllint;
pub mod redocly;
pub mod regal;
pub mod remark;
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
pub mod sqlfluff;
pub mod standardrb;
pub mod staticcheck;
pub mod statix;
pub mod stylelint;
pub mod stylua;
pub mod taplo;
pub mod terraform;
pub mod tflint;
pub mod tsc;
pub mod ty;
pub mod typstyle;
pub mod vale;
pub mod yamlfmt;
pub mod yamllint;
pub mod zig_ast_check;
pub mod zig_fmt;

pub use ansible_lint::parse_ansible_lint;
pub use biome::{parse_biome_format, parse_biome_lint};
pub use buf::{parse_buf_format, parse_buf_lint};
pub use buildifier::parse_buildifier;
pub use cfn_lint::parse_cfn_lint;
pub use check_jsonschema::{parse_check_jsonschema, CheckJsonschemaTarget};
pub use checkmake::parse_checkmake;
pub use checkstyle::{checkstyle_tab_width, parse_checkstyle};
pub use clang_format::parse_clang_format;
pub use clang_syntax::parse_clang_syntax;
pub use clang_tidy::parse_clang_tidy;
pub use cmakelint::parse_cmakelint;
pub use cppcheck::parse_cppcheck;
pub use credo::parse_credo;
pub use csharpier::parse_csharpier;
pub use cue::{parse_cue, parse_cue_vet};
pub use dart_analyze::{parse_dart_analyze, DartAnalyzeView};
pub use dart_format::parse_dart_format;
pub use deadnix::parse_deadnix;
pub use dfmt::parse_dfmt;
pub use dialyzer::parse_dialyzer;
pub use djlint::{djlint_ran, parse_djlint, parse_djlint_format};
pub use dscanner::parse_dscanner;
pub use elvis::parse_elvis;
pub use erlfmt::parse_erlfmt;
pub use errcheck::parse_errcheck;
pub use eslint::{parse_eslint, parse_graphql_eslint};
pub use fantomas::parse_fantomas;
pub use flake8::parse_flake8;
pub use flang::parse_flang;
pub use flatc::{parse_flatc, FlatcReport};
pub use fortitude::parse_fortitude;
pub use fsc::parse_fsc;
pub use fsharplint::parse_fsharplint;
pub use gersemi::parse_gersemi;
pub use go_analysis::{parse_go_analysis, parse_gotype};
pub use gofumpt::parse_gofumpt;
pub use google_java_format::parse_google_java_format;
pub use govet::parse_govet;
pub use gplint::parse_gplint;
pub use hadolint::parse_hadolint;
pub use helm::{parse_helm_lint, parse_helm_template_error, split_rendered, RenderedFile};
pub use hlint::parse_hlint;
pub use jsonnet_lint::parse_jsonnet_lint;
pub use jsonnetfmt::parse_jsonnetfmt;
pub use julia_format::parse_julia_format;
pub use jvm_diagnostics::parse_jvm_diagnostics;
pub use keep_sorted::{parse_keep_sorted, parse_keep_sorted_lint};
pub use ktfmt::parse_ktfmt;
pub use ktlint::parse_ktlint;
pub use kubeconform::{parse_kubeconform, KubeconformTarget};
pub use latexindent::parse_latexindent;
pub use ldc::parse_ldc;
pub use luals::parse_luals;
pub use markdown::parse_markdown_findings;
pub use mbake::parse_mbake;
pub use mix_format::parse_mix_format;
pub use modfmt::parse_modfmt;
pub use nixfmt::parse_nixfmt;
pub use notebook_outputs::parse_notebook_outputs;
pub use nvcc_syntax::parse_nvcc_syntax;
pub use opa::{parse_opa_check, parse_opa_fmt};
pub use ormolu::parse_ormolu;
pub use perlcritic::parse_perlcritic;
pub use perltidy::parse_perltidy;
pub use phpcs::parse_phpcs;
pub use phpstan::parse_phpstan;
pub use pkl::parse_pkl;
pub use pmd::parse_pmd;
pub use prettier::{parse_prettier_check, parse_prettier_check_with_sources};
pub use psscriptanalyzer::{parse_psscriptanalyzer, parse_psscriptanalyzer_format};
pub use pydoclint::parse_pydoclint;
pub use pylint::parse_pylint;
pub use qmlformat::parse_qmlformat;
pub use qmllint::{parse_qmllint, parse_qmllint_report, QmllintView};
pub use redocly::parse_redocly;
pub use regal::parse_regal;
pub use remark::parse_remark;
pub use roslyn::{parse_csc, parse_roslyn};
pub use rubocop::parse_rubocop;
pub use ruff::{parse_ruff, parse_ruff_format};
pub use rust::{parse_clippy, parse_rustc};
pub use rustfmt::parse_rustfmt;
pub use scalafix::parse_scalafix;
pub use scalafmt::parse_scalafmt;
pub use shellcheck::parse_shellcheck;
pub use shfmt::parse_shfmt;
pub use spotbugs::parse_spotbugs;
pub use sqlfluff::parse_sqlfluff;
pub use standardrb::parse_standardrb;
pub use staticcheck::parse_staticcheck;
pub use statix::parse_statix;
pub use stylelint::parse_stylelint;
pub use stylua::parse_stylua;
pub use taplo::{parse_taplo_format_check, parse_taplo_lint};
pub use terraform::parse_terraform;
pub use tflint::parse_tflint;
pub use tsc::{parse_tsc, parse_tsc_format};
pub use ty::parse_ty;
pub use typstyle::parse_typstyle;
pub use vale::parse_vale;
pub use yamlfmt::parse_yamlfmt;
pub use yamllint::parse_yamllint;
pub use zig_ast_check::parse_zig_ast_check;
pub use zig_fmt::parse_zig_fmt;

use std::path::Path;

use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

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
    #[error("{tool} analysis was incomplete: {detail}")]
    Incomplete { tool: &'static str, detail: String },
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

impl ParseError {
    /// The failure for output outside the pinned grammar.
    fn shape(tool: &'static str, detail: impl Into<String>) -> Self {
        Self::Shape {
            tool,
            detail: detail.into(),
        }
    }
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

/// The checked file a tool named in the host's own slashes.
fn known_native<'a>(
    tool: &'static str,
    files: &[&'a str],
    path: &str,
) -> Result<&'a str, ParseError> {
    if cfg!(windows) {
        known_spelled(tool, files, path)
    } else {
        known(tool, files, path)
    }
}

/// The checked file a tool named, matching the path after dropping `.` and `..` components.
fn known_lexical<'a>(
    tool: &'static str,
    files: &[&'a str],
    path: &str,
) -> Result<&'a str, ParseError> {
    use std::path::{Component, PathBuf};
    let mut clean = PathBuf::new();
    let mut valid = true;
    for part in Path::new(path).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => valid &= clean.pop(),
            other => clean.push(other.as_os_str()),
        }
    }
    files
        .iter()
        .copied()
        .find(|file| valid && Path::new(file) == clean)
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

/// The checked file a line starts with (longest wins), with everything after its colon.
fn located<'a>(
    tool: &'static str,
    line: &'a str,
    files: &[&'a str],
) -> Result<(&'a str, &'a str), ParseError> {
    if let Some(found) = prefixed(line, files) {
        return Ok(found);
    }
    if !line.contains(':') {
        return Err(missing(tool, "location", line));
    }
    let skip = if dx_path::drive_rooted(line) { 2 } else { 0 };
    let end = line[skip..].find(':').map_or(line.len(), |at| at + skip);
    Err(ParseError::UnknownFile {
        tool,
        path: line[..end].to_owned(),
    })
}

/// The checked file a line starts with (longest wins), followed by a colon, and the rest.
fn prefixed<'a>(line: &'a str, files: &[&'a str]) -> Option<(&'a str, &'a str)> {
    files
        .iter()
        .copied()
        .filter(|file| line.strip_prefix(file).is_some_and(|r| r.starts_with(':')))
        .max_by_key(|file| file.len())
        .map(|file| (file, &line[file.len() + 1..]))
}

/// Exactly `row<sep>col`, both numbers trimmed.
fn row_col(text: &str, sep: char) -> Option<(u64, u64)> {
    let (row, col) = text.split_once(sep)?;
    Some((row.trim().parse().ok()?, col.trim().parse().ok()?))
}

/// A `path:row:col` triple split from the right, with a non-empty path.
fn path_row_col(text: &str) -> Option<(&str, u64, u64)> {
    let (rest, col) = text.rsplit_once(':')?;
    let (path, row) = rest.rsplit_once(':')?;
    if path.is_empty() {
        return None;
    }
    Some((path, row.trim().parse().ok()?, col.trim().parse().ok()?))
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

/// A finding at a line and column, spanning nothing and suggesting nothing.
fn finding(
    tool: &'static str,
    file: &str,
    rule_id: String,
    message: String,
    severity: ToolSeverity,
    start: TextPosition,
    encoding: ColumnEncoding,
) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: tool.to_owned(),
            rule_id,
            message,
            severity,
            start,
            end: None,
            suggestions: Vec::new(),
            encoding,
            related: Vec::new(),
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
        return Err(ParseError::shape(
            tool,
            format!("exit {} with no diagnostics", code_name(code)),
        ));
    }
    Ok(())
}

fn point(line: u64, column: u64) -> (TextPosition, Option<TextPosition>) {
    (TextPosition { line, column }, None)
}

fn missing(tool: &'static str, what: &str, line: &str) -> ParseError {
    ParseError::shape(tool, format!("malformed {what}: {line}"))
}

fn code_name(code: Option<i32>) -> String {
    code.map_or_else(|| "signal".to_owned(), |code| code.to_string())
}

const DEV_NULL: &str = "/dev/null";

/// The paths a unified diff says were reformatted, in the order the diff names them.
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
fn git_prefixed(old: Option<&str>, new: Option<&str>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) if new == DEV_NULL => old.starts_with("a/"),
        (Some(old), Some(new)) if old == DEV_NULL => new.starts_with("b/"),
        (Some(old), Some(new)) => old.starts_with("a/") && new.starts_with("b/"),
        _ => false,
    }
}

/// The path one side of a diff header names.
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
    /// gofmt-style `-d` exits exactly 1 once a diff exists; 2 is a parse error.
    One,
}

impl DiffExit {
    /// Whether the tool may exit with `code` once it named files.
    fn allows(self, code: Option<i32>) -> bool {
        match self {
            Self::Zero => code == Some(0),
            Self::NonZero => code != Some(0),
            Self::ZeroOrOne => code == Some(0) || code == Some(1),
            Self::One => code == Some(1),
        }
    }
}

/// Report the offenses in the RuboCop JSON grammar, which `rubocop` and `standardrb` share.
pub(super) fn rubocop_json(
    tool: &'static str,
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stdout)?;
    check_output_size(tool, stderr)?;
    if let Some(crash) = rubocop_crash(stderr) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("cop crashed: {crash}"),
        });
    }
    let stderr = String::from_utf8_lossy(stderr);
    let lines: Vec<String> = stderr
        .lines()
        .map(|line| mix_format::strip_ansi(line).trim().to_owned())
        .collect();
    if !matches!(code, Some(0) | Some(1)) {
        let first = lines
            .iter()
            .find(|line| !line.is_empty())
            .map_or("no stderr", String::as_str);
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} : {first}", code_name(code)),
        });
    }
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
        let checked = known_relative(tool, files, cwd, path)?;
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
                Some("info") => ToolSeverity::Info,
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
                    encoding: ColumnEncoding::Scalars,
                    related: Vec::new(),
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

/// Find the stderr line that shows a RuboCop cop crashed, shared by the check and fix paths.
pub fn rubocop_crash(stderr: &[u8]) -> Option<String> {
    let stderr = String::from_utf8_lossy(stderr);
    let lines: Vec<String> = stderr
        .lines()
        .map(|line| mix_format::strip_ansi(line).trim().to_owned())
        .collect();
    let inspecting = lines.iter().find(|line| {
        line.contains("An error occurred while ") && line.contains(" cop was inspecting ")
    });
    inspecting
        .or_else(|| lines.iter().find(|line| rubocop_error_summary(line)))
        .cloned()
}

/// Whether a stderr line is RuboCop's `N error(s) occurred:` crash summary.
fn rubocop_error_summary(line: &str) -> bool {
    let Some((count, rest)) = line.split_once(' ') else {
        return false;
    };
    !count.is_empty()
        && count.bytes().all(|b| b.is_ascii_digit())
        && matches!(rest, "error occurred:" | "errors occurred:")
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

/// How a tool spells a path it listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Spelling<'a> {
    /// The path the tool wrote, held to the run's own files.
    Exact,
    /// The path the tool wrote, taken from the directory it ran in.
    Below(&'a Path),
}

/// Report one "file is not formatted" finding per path a tool listed, one per line.
pub(super) fn listed_paths(
    tool: &'static str,
    bytes: &[u8],
    code: Option<i32>,
    files: &[&str],
    noise: &[&str],
    spelling: Spelling<'_>,
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(tool, bytes)?;
    let mut paths: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || noise.contains(&trimmed) {
            continue;
        }
        let path = trimmed.strip_prefix("./").unwrap_or(trimmed);
        if !paths.contains(&path) {
            paths.push(path);
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
    let mut findings = Vec::with_capacity(paths.len());
    for path in paths {
        let checked = match spelling {
            Spelling::Exact => known(tool, files, path)?,
            Spelling::Below(cwd) => known_relative(tool, files, cwd, path)?,
        };
        findings.push(unformatted_at(tool, checked, 1, 1, ColumnEncoding::Scalars));
    }
    Ok(findings)
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
    paths
        .iter()
        .map(|path| {
            Ok(unformatted_at(
                tool,
                known(tool, files, path)?,
                1,
                1,
                ColumnEncoding::Scalars,
            ))
        })
        .collect()
}

/// A "file is not formatted" finding at a line and column.
pub(super) fn unformatted_at(
    tool: &'static str,
    file: &str,
    line: u64,
    column: u64,
    encoding: ColumnEncoding,
) -> FileFinding {
    finding(
        tool,
        file,
        String::new(),
        "file is not formatted".to_owned(),
        ToolSeverity::Warning,
        TextPosition { line, column },
        encoding,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        check_output_size, code_name, columned, finding, known_native, lines, located, missing,
        numbered, path_row_col, prefixed, require_findings, row_col, FileFinding, ParseError,
        Spelling, MAX_OUTPUT_BYTES,
    };
    use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

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

    /// A tool may name a file in the host's own slashes.
    #[test]
    fn a_path_in_host_slashes_names_its_checked_file() {
        let files = ["src/a.js"];
        assert_eq!(known_native("biome", &files, "src/a.js"), Ok("src/a.js"));
        assert_eq!(
            known_native("biome", &files, "src\\a.js").is_ok(),
            cfg!(windows)
        );
        assert!(known_native("biome", &files, "src/b.js").is_err());
    }

    #[test]
    fn shape_builds_the_shape_variant() {
        assert_eq!(
            ParseError::shape("t", "d"),
            ParseError::Shape {
                tool: "t",
                detail: "d".to_owned()
            }
        );
    }

    #[test]
    fn row_col_reads_exactly_two_numbers() {
        assert_eq!(row_col("3:7", ':'), Some((3, 7)));
        assert_eq!(row_col(" 3 , 7 ", ','), Some((3, 7)));
        assert_eq!(row_col("3", ':'), None);
        assert_eq!(row_col("a:7", ':'), None);
        assert_eq!(row_col("3:7:", ':'), None);
    }

    #[test]
    fn path_row_col_needs_a_path_line_and_column() {
        assert_eq!(path_row_col("src/a.ex:3:7"), Some(("src/a.ex", 3, 7)));
        assert_eq!(path_row_col(":3:7"), None);
        assert_eq!(path_row_col("src/a.ex:3"), None);
        assert_eq!(path_row_col("src/a.ex"), None);
        assert_eq!(path_row_col(r"C:\a.x:3:7"), Some((r"C:\a.x", 3, 7)));
    }

    #[test]
    fn prefixed_picks_the_longest_checked_file() {
        let files = ["/s/a", "/s/a.b"];
        assert_eq!(prefixed("/s/a.b:1: x", &files), Some(("/s/a.b", "1: x")));
        assert_eq!(prefixed("/s/a:1: x", &files), Some(("/s/a", "1: x")));
        assert_eq!(prefixed("/s/a.bc", &files), None);
        assert_eq!(prefixed("/s/z:1: x", &files), None);
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
    fn located_reads_colon_in_unix_path() {
        assert_eq!(
            located("tsc", "a:b.sh:1:2: x", &["a:b.sh"]).expect("located"),
            ("a:b.sh", "1:2: x")
        );
    }

    #[test]
    fn located_matches_drive_rooted_paths() {
        let files = [r"C:\a", r"C:\a:b"];
        assert_eq!(
            located("tsc", r"C:\a:b:1:2: x", &files).expect("located"),
            (r"C:\a:b", "1:2: x")
        );
        assert_eq!(
            located("tsc", r"C:\a:1:2: x", &files).expect("located"),
            (r"C:\a", "1:2: x")
        );
        assert!(matches!(
            located("tsc", r"C:\x\b.sh:1:1: m", &files),
            Err(ParseError::UnknownFile { path, .. }) if path == r"C:\x\b.sh"
        ));
        assert!(located("tsc", "no-colon-here", &files)
            .expect_err("no location")
            .to_string()
            .contains("malformed location"));
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
    fn finding_carries_the_tool_the_position_and_nothing_else() {
        let found = finding(
            "tsc",
            "/s/a.rs",
            "R1".to_owned(),
            "msg".to_owned(),
            ToolSeverity::Error,
            TextPosition { line: 3, column: 4 },
            ColumnEncoding::Scalars,
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
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
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
            TextPosition { line: 1, column: 1 },
            ColumnEncoding::Scalars,
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
    fn every_sarif_consumer_rejects_a_failed_invocation() {
        let failed = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},"results":[],"invocations":[{"executionSuccessful":false,"toolConfigurationNotifications":[{"level":"error","message":{"text":"missing class"}}]}]}]}"#;
        let files = ["/s/a.java"];
        let results = [
            super::checkstyle::parse_checkstyle(failed, Some(0), &files, 8),
            super::pmd::parse_pmd(failed, Some(0), &files),
            super::spotbugs::parse_spotbugs(failed, Some(0), &files),
            super::ktlint::parse_ktlint(failed, Some(0), &files),
            super::roslyn::parse_roslyn(failed, Some(0), &files),
        ];
        for result in results {
            let err = result.expect_err("incomplete");
            assert!(matches!(err, ParseError::Incomplete { .. }), "{err}");
            assert!(err.to_string().contains("missing class"), "{err}");
        }
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
        assert!(super::sarif::parse_sarif(
            "sarif-test",
            &big,
            Some(1),
            &["/s/a.java"],
            ColumnEncoding::Utf16
        )
        .is_err());
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
            (super::DiffExit::One, Some(1), true),
            (super::DiffExit::One, Some(0), false),
            (super::DiffExit::One, Some(2), false),
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
            super::DiffExit::One,
        ] {
            assert!(super::diff_format("t", b"", Some(0), &["x.go"], exit)
                .expect("clean")
                .is_empty());
        }
    }

    #[test]
    fn listed_paths_reports_each_listed_file_once() {
        let listed = b"\n  x.go  \n./x.go\n\nx.go\n";
        let findings = super::listed_paths("t", listed, Some(1), &["x.go"], &[], Spelling::Exact)
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "x.go");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(
            findings[0].finding.start,
            TextPosition { line: 1, column: 1 }
        );
        assert!(matches!(
            super::listed_paths("t", b"y.go\n", Some(1), &["x.go"], &[], Spelling::Exact),
            Err(super::ParseError::UnknownFile { path, .. }) if path == "y.go"
        ));
    }

    #[test]
    fn listed_paths_skips_lines_that_name_no_file() {
        let listed = b"HEADER: none\nx.go\n";
        let findings = super::listed_paths(
            "t",
            listed,
            Some(1),
            &["x.go"],
            &["HEADER: none"],
            Spelling::Exact,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert!(super::listed_paths(
            "t",
            b"HEADER: none\n",
            Some(0),
            &["x.go"],
            &["HEADER: none"],
            Spelling::Exact
        )
        .expect("clean")
        .is_empty());
    }

    #[test]
    fn listed_paths_gates_an_empty_list_on_the_exit_code() {
        for code in [Some(3), None] {
            let err = super::listed_paths("t", b"", code, &["x.go"], &[], Spelling::Exact)
                .expect_err("a listed tool that named nothing");
            assert_eq!(
                err.to_string(),
                format!(
                    "t output is outside the pinned grammar: exit {} with no paths",
                    code_name(code)
                )
            );
        }
        assert!(matches!(
            super::listed_paths("t", &[0xff], Some(1), &["x.go"], &[], Spelling::Exact),
            Err(super::ParseError::Shape { .. })
        ));
    }

    /// `terraform fmt -check` names a file by its path from the directory it ran in.
    #[test]
    fn listed_paths_reads_a_path_spelled_below_the_working_directory() {
        let cwd = Path::new("/w");
        let files = ["/w/sub/x.tf"];
        let findings = super::listed_paths(
            "t",
            b"sub/x.tf\n",
            Some(3),
            &files,
            &[],
            Spelling::Below(cwd),
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "/w/sub/x.tf");
        assert!(matches!(
            super::listed_paths("t", b"sub/x.tf\n", Some(3), &files, &[], Spelling::Exact),
            Err(super::ParseError::UnknownFile { .. })
        ));
        assert!(matches!(
            super::listed_paths(
                "t",
                b"../w/sub/x.tf\n",
                Some(3),
                &files,
                &[],
                Spelling::Below(cwd)
            ),
            Err(super::ParseError::UnknownFile { .. })
        ));
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
        "/s/a.mdx",
        "/s/a.fbs",
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
        br#"[{"file":"/s/a.mdx","formatted":false}]"#,
        b"/s/a.mdx:5:1: Expected a closing tag\n",
        br#"{"diagnostics":[{"file":"/s/a.qml","line":1,"message":"m"}]}"#,
        br#"{"path":"/s/a.scala","line":1,"column":1,"rule":"r","message":"m","severity":"error"}"#,
        b"path:1:1: E100 message\n",
        b"\nerror:\n  /s/a.fbs:1: 19: error: msg\n\n",
        b"\nerror:\n  /s/a.fbs(1, 19): error: msg\n",
        b"/s/o/a_generated.h: \\\n /s/a.fbs\n",
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
        super::buf::parse_buf_lint(input, code, files, &[])
    }

    fn fuzz_buf_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::buf::parse_buf_format(
            input,
            b"",
            code,
            input,
            files.first().copied().unwrap_or("x.proto"),
        )
    }

    fn fuzz_checkstyle(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::checkstyle::parse_checkstyle(input, code, files, 8)
    }

    fn fuzz_clang_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::clang_format::parse_clang_format(input, code, files)
    }

    fn fuzz_clang_syntax(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::clang_syntax::parse_clang_syntax(input, code, files)
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
        super::cue::parse_cue(input, input, code, files, std::path::Path::new("/"))
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

    fn fuzz_go_analysis(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::go_analysis::parse_go_analysis("govet", input, code, files)
    }

    fn fuzz_gotype(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::go_analysis::parse_gotype("gotype", input, code, files)
    }

    fn fuzz_errcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::errcheck::parse_errcheck(input, code, files)
    }

    fn fuzz_eslint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::eslint::parse_eslint(input, code, files)
    }

    fn fuzz_graphql_eslint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::eslint::parse_graphql_eslint(input, code, files)
    }

    fn fuzz_redocly(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let root = files.first().copied().unwrap_or("/scratch/openapi.yaml");
        super::redocly::parse_redocly(input, code, root, Path::new("/"), files)
    }

    fn fuzz_fantomas(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::fantomas::parse_fantomas(input, code, files, Path::new("/scratch"))
    }

    fn fuzz_flake8(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::flake8::parse_flake8(input, code, files)
    }

    fn fuzz_flatc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let both = super::flatc::parse_flatc(input, input, code, files)?;
        Ok(both.findings)
    }

    fn fuzz_fortitude(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::fortitude::parse_fortitude(input, code, files)
    }

    fn fuzz_fsc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::fsc::parse_fsc(input, code, files)
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

    fn fuzz_helm(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let _ = super::helm::parse_helm_template_error(input, "", files);
        let _ = super::helm::split_rendered(input);
        super::helm::parse_helm_lint(input, code, "", files)
    }

    fn fuzz_jsonnet_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::jsonnet_lint::parse_jsonnet_lint(input, input, code, files)
    }

    fn fuzz_jsonnetfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::jsonnetfmt::parse_jsonnetfmt(input, input, code, files)
    }

    fn fuzz_keep_sorted(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::keep_sorted::parse_keep_sorted(input, code, files)
    }

    fn fuzz_keep_sorted_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::keep_sorted::parse_keep_sorted_lint(input, code, files)
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

    fn fuzz_luals(
        input: &[u8],
        _code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::luals::parse_luals(input, files)
    }

    fn fuzz_markdown_findings(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::markdown::parse_markdown_findings(input, code, files)
    }

    fn fuzz_notebook_outputs(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::notebook_outputs::parse_notebook_outputs(input, code, files)
    }

    fn fuzz_modfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::modfmt::parse_modfmt(input, code, files)
    }

    fn fuzz_nixfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::nixfmt::parse_nixfmt(input, input, code, files)
    }

    fn fuzz_nvcc_syntax(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::nvcc_syntax::parse_nvcc_syntax(input, code, files)
    }

    fn fuzz_ormolu(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ormolu::parse_ormolu(input, input, code, files)
    }

    fn fuzz_cue_vet(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::cue::parse_cue_vet(input, input, code, files, files, std::path::Path::new("/"))
    }

    fn fuzz_opa_fmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::opa::parse_opa_fmt(
            input,
            input,
            code,
            input,
            files.first().copied().unwrap_or(""),
        )
    }

    fn fuzz_opa_check(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::opa::parse_opa_check(input, input, code, files)
    }

    fn fuzz_perlcritic(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::perlcritic::parse_perlcritic(input, code, files)
    }

    fn fuzz_perltidy(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::perltidy::parse_perltidy(
            input,
            input,
            code,
            input,
            files.first().copied().unwrap_or(""),
        )
    }

    fn fuzz_pkl(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::pkl::parse_pkl(input, input, code, files)
    }

    fn fuzz_phpcs(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::phpcs::parse_phpcs(input, code, files)
    }

    fn fuzz_phpstan(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::phpstan::parse_phpstan(input, code, files)
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

    fn fuzz_psscriptanalyzer_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::psscriptanalyzer::parse_psscriptanalyzer_format(input, code, files)
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
        super::qmlformat::parse_qmlformat(input, code, b"", files.first().copied().unwrap_or(""))
    }

    fn fuzz_dfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::dfmt::parse_dfmt(
            input,
            input,
            code,
            input,
            files.first().copied().unwrap_or(""),
        )
    }

    fn fuzz_dscanner(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::dscanner::parse_dscanner(input, code, files)
    }

    fn fuzz_credo(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::credo::parse_credo(input, b"dx-credo: {\"invalid\":[]}\n", code, files)
    }

    fn fuzz_elvis(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::elvis::parse_elvis(input, b"", code, files)
    }

    fn fuzz_flang(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::flang::parse_flang(input, code, files)
    }

    fn fuzz_ldc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ldc::parse_ldc(input, code, files)
    }

    fn fuzz_dialyzer(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::dialyzer::parse_dialyzer(input, code, files)
    }

    fn fuzz_erlfmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::erlfmt::parse_erlfmt(b"", input, code, files)
    }

    fn fuzz_mix_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::mix_format::parse_mix_format(b"", input, code, files, std::path::Path::new("/"))
    }

    fn fuzz_dart_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::dart_format::parse_dart_format(input, input, code, files)
    }

    fn fuzz_dart_analyze(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::dart_analyze::parse_dart_analyze(
            input,
            b"",
            code,
            files,
            "/s/analysis_options.yaml",
            super::dart_analyze::DartAnalyzeView::Lint,
        )
    }

    fn fuzz_julia_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::julia_format::parse_julia_format(input, input, code, files)
    }

    fn fuzz_zig_ast_check(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::zig_ast_check::parse_zig_ast_check(input, input, code, files)
    }

    fn fuzz_zig_fmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::zig_fmt::parse_zig_fmt(input, input, code, files)
    }

    fn fuzz_qmllint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let lint =
            super::qmllint::parse_qmllint(input, code, files, super::qmllint::QmllintView::Lint);
        let types = super::qmllint::parse_qmllint(
            input,
            code,
            files,
            super::qmllint::QmllintView::Typecheck,
        );
        lint.and_then(|mut found| {
            found.extend(types?);
            Ok(found)
        })
    }

    fn fuzz_csc(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::roslyn::parse_csc(input, code, files)
    }

    fn fuzz_jvm_diagnostics(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::jvm_diagnostics::parse_jvm_diagnostics("javac", input, code, files)
    }

    fn fuzz_roslyn(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::roslyn::parse_roslyn(input, code, files)
    }

    fn fuzz_deadnix(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::deadnix::parse_deadnix(input, input, code, files)
    }

    fn fuzz_statix(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::statix::parse_statix(input, input, code, files)
    }

    fn fuzz_ansible_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ansible_lint::parse_ansible_lint(input, input, code, files)
    }

    fn fuzz_cfn_lint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::cfn_lint::parse_cfn_lint(input, input, code, files)
    }

    fn fuzz_kubeconform(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let targets: Vec<super::KubeconformTarget<'_>> = files
            .iter()
            .map(|file| super::KubeconformTarget {
                reported: file,
                file,
                documents: &[],
            })
            .collect();
        super::kubeconform::parse_kubeconform(input, input, code, &targets)
    }

    fn fuzz_check_jsonschema(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        let targets: Vec<super::CheckJsonschemaTarget<'_>> = files
            .iter()
            .map(|file| super::CheckJsonschemaTarget {
                reported: file,
                file,
                bytes: b"{}",
                filetype: crate::json_schema::Filetype::Json,
            })
            .collect();
        super::check_jsonschema::parse_check_jsonschema(input, input, code, &targets)
    }

    fn fuzz_checkmake(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::checkmake::parse_checkmake(input, input, code, files, None)
    }

    fn fuzz_cmakelint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::cmakelint::parse_cmakelint(input, input, code, files)
    }

    fn fuzz_gplint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::gplint::parse_gplint(input, code, files)
    }

    fn fuzz_mbake(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::mbake::parse_mbake(input, input, code, files)
    }

    fn fuzz_gersemi(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::gersemi::parse_gersemi(input, input, code, files)
    }

    fn fuzz_tflint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::tflint::parse_tflint(input, code, files)
    }

    fn fuzz_hadolint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::hadolint::parse_hadolint(input, input, code, files)
    }

    fn fuzz_hlint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::hlint::parse_hlint(input, input, code, files)
    }

    fn fuzz_rubocop(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::rubocop::parse_rubocop(input, input, code, files, Path::new(""))
    }

    fn fuzz_ruff(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ruff::parse_ruff(input, code, files, &[])
    }

    fn fuzz_ruff_format(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::ruff::parse_ruff_format(input, code, files, &[])
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
        super::sarif::parse_sarif("fuzz", input, code, files, ColumnEncoding::Utf16)
    }

    fn fuzz_scalafix(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::scalafix::parse_scalafix(input, b"", code, files)
    }

    fn fuzz_scalafmt(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::scalafmt::parse_scalafmt(input, b"", code, files, std::path::Path::new("/"))
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

    fn fuzz_sqlfluff(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::sqlfluff::parse_sqlfluff(input, input, code, files)
    }

    fn fuzz_standardrb(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::standardrb::parse_standardrb(input, input, code, files, Path::new(""))
    }

    fn fuzz_staticcheck(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::staticcheck::parse_staticcheck(input, code, files)
    }

    fn fuzz_regal(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::regal::parse_regal(input, input, code, files)
    }

    fn fuzz_remark(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::remark::parse_remark(input, input, code, files)
    }

    fn fuzz_stylelint(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::stylelint::parse_stylelint(input, code, files)
    }

    fn fuzz_stylua(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::stylua::parse_stylua(input, input, code, files)
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
        super::terraform::parse_terraform(input, input, code, files, std::path::Path::new("/"))
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

    fn fuzz_latexindent(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::latexindent::parse_latexindent(input, input, code, files)
    }

    fn fuzz_typstyle(
        input: &[u8],
        code: Option<i32>,
        files: &[&str],
    ) -> Result<Vec<super::FileFinding>, ParseError> {
        super::typstyle::parse_typstyle(input, input, code, files, std::path::Path::new("/"))
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
        ("checkmake", fuzz_checkmake),
        ("gersemi", fuzz_gersemi),
        ("checkstyle", fuzz_checkstyle),
        ("cmakelint", fuzz_cmakelint),
        ("tflint", fuzz_tflint),
        ("clang_format", fuzz_clang_format),
        ("clang_syntax", fuzz_clang_syntax),
        ("clang_tidy", fuzz_clang_tidy),
        ("cppcheck", fuzz_cppcheck),
        ("csharpier", fuzz_csharpier),
        ("credo", fuzz_credo),
        ("cue", fuzz_cue),
        ("cue_vet", fuzz_cue_vet),
        ("deadnix", fuzz_deadnix),
        ("dfmt", fuzz_dfmt),
        ("dscanner", fuzz_dscanner),
        ("djlint", fuzz_djlint),
        ("djlint_format", fuzz_djlint_format),
        ("dialyzer", fuzz_dialyzer),
        ("flang", fuzz_flang),
        ("ldc", fuzz_ldc),
        ("elvis", fuzz_elvis),
        ("erlfmt", fuzz_erlfmt),
        ("errcheck", fuzz_errcheck),
        ("go_analysis", fuzz_go_analysis),
        ("gotype", fuzz_gotype),
        ("eslint", fuzz_eslint),
        ("fantomas", fuzz_fantomas),
        ("flake8", fuzz_flake8),
        ("flatc", fuzz_flatc),
        ("fortitude", fuzz_fortitude),
        ("fsc", fuzz_fsc),
        ("fsharplint", fuzz_fsharplint),
        ("gofumpt", fuzz_gofumpt),
        ("google_java_format", fuzz_google_java_format),
        ("graphql_eslint", fuzz_graphql_eslint),
        ("govet", fuzz_govet),
        ("gplint", fuzz_gplint),
        ("hadolint", fuzz_hadolint),
        ("helm", fuzz_helm),
        ("hlint", fuzz_hlint),
        ("jsonnet_lint", fuzz_jsonnet_lint),
        ("jsonnetfmt", fuzz_jsonnetfmt),
        ("dart", fuzz_dart_format),
        ("dart_analyze", fuzz_dart_analyze),
        ("julia_format", fuzz_julia_format),
        ("keep_sorted", fuzz_keep_sorted),
        ("keep_sorted_lint", fuzz_keep_sorted_lint),
        ("ansible_lint", fuzz_ansible_lint),
        ("cfn_lint", fuzz_cfn_lint),
        ("check_jsonschema", fuzz_check_jsonschema),
        ("kubeconform", fuzz_kubeconform),
        ("latexindent", fuzz_latexindent),
        ("mbake", fuzz_mbake),
        ("ktfmt", fuzz_ktfmt),
        ("ktlint", fuzz_ktlint),
        ("luals", fuzz_luals),
        ("parse_markdown_findings", fuzz_markdown_findings),
        ("parse_notebook_outputs", fuzz_notebook_outputs),
        ("mix_format", fuzz_mix_format),
        ("modfmt", fuzz_modfmt),
        ("nixfmt", fuzz_nixfmt),
        ("nvcc_syntax", fuzz_nvcc_syntax),
        ("ormolu", fuzz_ormolu),
        ("opa_fmt", fuzz_opa_fmt),
        ("opa_check", fuzz_opa_check),
        ("perlcritic", fuzz_perlcritic),
        ("perltidy", fuzz_perltidy),
        ("pkl", fuzz_pkl),
        ("phpcs", fuzz_phpcs),
        ("phpstan", fuzz_phpstan),
        ("pmd", fuzz_pmd),
        ("parse_prettier_check", fuzz_prettier_check),
        ("psscriptanalyzer", fuzz_psscriptanalyzer),
        ("psscriptanalyzer_format", fuzz_psscriptanalyzer_format),
        ("pydoclint", fuzz_pydoclint),
        ("pylint", fuzz_pylint),
        ("qmlformat", fuzz_qmlformat),
        ("qmllint", fuzz_qmllint),
        ("redocly", fuzz_redocly),
        ("regal", fuzz_regal),
        ("remark", fuzz_remark),
        ("csc", fuzz_csc),
        ("jvm_diagnostics", fuzz_jvm_diagnostics),
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
        ("sqlfluff", fuzz_sqlfluff),
        ("standardrb", fuzz_standardrb),
        ("staticcheck", fuzz_staticcheck),
        ("statix", fuzz_statix),
        ("stylelint", fuzz_stylelint),
        ("stylua", fuzz_stylua),
        ("taplo_lint", fuzz_taplo_lint),
        ("taplo_format_check", fuzz_taplo_format_check),
        ("terraform", fuzz_terraform),
        ("tsc", fuzz_tsc),
        ("ty", fuzz_ty),
        ("typstyle", fuzz_typstyle),
        ("vale", fuzz_vale),
        ("yamlfmt", fuzz_yamlfmt),
        ("yamllint", fuzz_yamllint),
        ("zig_ast_check", fuzz_zig_ast_check),
        ("zig_fmt", fuzz_zig_fmt),
    ];

    const PARSER_SOURCES: &[(&str, &str)] = &[
        ("biome", include_str!("biome.rs")),
        ("buf", include_str!("buf.rs")),
        ("buildifier", include_str!("buildifier.rs")),
        ("checkmake", include_str!("checkmake.rs")),
        ("gersemi", include_str!("gersemi.rs")),
        ("checkstyle", include_str!("checkstyle.rs")),
        ("cmakelint", include_str!("cmakelint.rs")),
        ("tflint", include_str!("tflint.rs")),
        ("clang_format", include_str!("clang_format.rs")),
        ("clang_syntax", include_str!("clang_syntax.rs")),
        ("clang_tidy", include_str!("clang_tidy.rs")),
        ("cppcheck", include_str!("cppcheck.rs")),
        ("csharpier", include_str!("csharpier.rs")),
        ("credo", include_str!("credo.rs")),
        ("cue", include_str!("cue.rs")),
        ("deadnix", include_str!("deadnix.rs")),
        ("dfmt", include_str!("dfmt.rs")),
        ("dscanner", include_str!("dscanner.rs")),
        ("djlint", include_str!("djlint.rs")),
        ("dialyzer", include_str!("dialyzer.rs")),
        ("flang", include_str!("flang.rs")),
        ("ldc", include_str!("ldc.rs")),
        ("elvis", include_str!("elvis.rs")),
        ("erlfmt", include_str!("erlfmt.rs")),
        ("errcheck", include_str!("errcheck.rs")),
        ("go_analysis", include_str!("go_analysis.rs")),
        ("gotype", include_str!("go_analysis.rs")),
        ("eslint", include_str!("eslint.rs")),
        ("fantomas", include_str!("fantomas.rs")),
        ("flake8", include_str!("flake8.rs")),
        ("flatc", include_str!("flatc.rs")),
        ("fortitude", include_str!("fortitude.rs")),
        ("fsc", include_str!("fsc.rs")),
        ("fsharplint", include_str!("fsharplint.rs")),
        ("gofumpt", include_str!("gofumpt.rs")),
        ("google_java_format", include_str!("google_java_format.rs")),
        ("govet", include_str!("govet.rs")),
        ("gplint", include_str!("gplint.rs")),
        ("hadolint", include_str!("hadolint.rs")),
        ("helm", include_str!("helm.rs")),
        ("hlint", include_str!("hlint.rs")),
        ("jsonnet_lint", include_str!("jsonnet_lint.rs")),
        ("jsonnetfmt", include_str!("jsonnetfmt.rs")),
        ("dart", include_str!("dart_format.rs")),
        ("dart_analyze", include_str!("dart_analyze.rs")),
        ("julia_format", include_str!("julia_format.rs")),
        ("keep_sorted", include_str!("keep_sorted.rs")),
        ("ansible_lint", include_str!("ansible_lint.rs")),
        ("cfn_lint", include_str!("cfn_lint.rs")),
        ("kubeconform", include_str!("kubeconform.rs")),
        ("latexindent", include_str!("latexindent.rs")),
        ("ktfmt", include_str!("ktfmt.rs")),
        ("ktlint", include_str!("ktlint.rs")),
        ("luals", include_str!("luals.rs")),
        ("markdown", include_str!("markdown.rs")),
        ("mbake", include_str!("mbake.rs")),
        ("notebook_outputs", include_str!("notebook_outputs.rs")),
        ("mix_format", include_str!("mix_format.rs")),
        ("modfmt", include_str!("modfmt.rs")),
        ("nixfmt", include_str!("nixfmt.rs")),
        ("nvcc_syntax", include_str!("nvcc_syntax.rs")),
        ("ormolu", include_str!("ormolu.rs")),
        ("opa", include_str!("opa.rs")),
        ("perlcritic", include_str!("perlcritic.rs")),
        ("perltidy", include_str!("perltidy.rs")),
        ("phpcs", include_str!("phpcs.rs")),
        ("phpstan", include_str!("phpstan.rs")),
        ("pkl", include_str!("pkl.rs")),
        ("pmd", include_str!("pmd.rs")),
        ("prettier", include_str!("prettier.rs")),
        ("psscriptanalyzer", include_str!("psscriptanalyzer.rs")),
        ("pydoclint", include_str!("pydoclint.rs")),
        ("pylint", include_str!("pylint.rs")),
        ("qmlformat", include_str!("qmlformat.rs")),
        ("qmllint", include_str!("qmllint.rs")),
        ("redocly", include_str!("redocly.rs")),
        ("regal", include_str!("regal.rs")),
        ("remark", include_str!("remark.rs")),
        ("csc", include_str!("roslyn.rs")),
        ("jvm_diagnostics", include_str!("jvm_diagnostics.rs")),
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
        ("sqlfluff", include_str!("sqlfluff.rs")),
        ("standardrb", include_str!("standardrb.rs")),
        ("staticcheck", include_str!("staticcheck.rs")),
        ("statix", include_str!("statix.rs")),
        ("stylelint", include_str!("stylelint.rs")),
        ("stylua", include_str!("stylua.rs")),
        ("check_jsonschema", include_str!("check_jsonschema.rs")),
        ("taplo", include_str!("taplo.rs")),
        ("terraform", include_str!("terraform.rs")),
        ("tsc", include_str!("tsc.rs")),
        ("ty", include_str!("ty.rs")),
        ("typstyle", include_str!("typstyle.rs")),
        ("vale", include_str!("vale.rs")),
        ("yamlfmt", include_str!("yamlfmt.rs")),
        ("yamllint", include_str!("yamllint.rs")),
        ("zig_ast_check", include_str!("zig_ast_check.rs")),
        ("zig_fmt", include_str!("zig_fmt.rs")),
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
            declared.len() >= 60,
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
