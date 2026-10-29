use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{as_text, code_name, finding, FileFinding, ParseError};

const TOOL: &str = "ocaml";

/// One diagnostic being read: where it starts and the lines of its message so far.
struct Pending {
    file: String,
    line: u64,
    column: u64,
    message: Vec<String>,
}

/// `ocamlc|ocamlopt -stop-after typing -error-style short` output.
///
/// Each diagnostic is a `File "p", line N, characters A-B:` header (or `lines N-M,`)
/// followed by an `Error:`, `Warning N [name]:` or `Alert name:` message that continues on
/// indented lines. Columns are 0-based bytes. Fails closed: a diagnostic in a file that is
/// not owned, a nonzero exit without an error for an owned file, and exit 0 with an error
/// are never a clean run.
pub fn parse_ocaml_compile(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, output)?;
    let mut pending: Option<Pending> = None;
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some((path, row, col)) = header(line) {
            flush(&mut pending, &mut found);
            let Some(owned) = files
                .iter()
                .copied()
                .find(|file| *file == path || path.strip_prefix("./") == Some(*file))
            else {
                return Err(ParseError::UnknownFile {
                    tool: TOOL,
                    path: path.to_owned(),
                });
            };
            pending = Some(Pending {
                file: owned.to_owned(),
                line: row,
                column: col,
                message: Vec::new(),
            });
        } else if let Some(open) = pending.as_mut() {
            let continuation = line.starts_with(char::is_whitespace) || line.starts_with("Hint:");
            if open.message.is_empty() || (continuation && !line.trim().is_empty()) {
                open.message.push(line.trim_end().to_owned());
            } else {
                flush(&mut pending, &mut found);
            }
        }
    }
    flush(&mut pending, &mut found);
    let errors = found
        .iter()
        .filter(|f| f.finding.severity == ToolSeverity::Error)
        .count();
    match code {
        Some(0) if errors == 0 => Ok(found),
        Some(0) => Err(ParseError::shape(
            TOOL,
            "exit 0 with error diagnostics (want a nonzero exit)",
        )),
        Some(_) if errors > 0 => Ok(found),
        other => {
            let first = text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("no output");
            Err(ParseError::shape(
                TOOL,
                format!(
                    "exit {} without a diagnostic for a checked file: {first}",
                    code_name(other)
                ),
            ))
        }
    }
}

/// `File "p", line N, characters A-B:` or `File "p", lines N-M, characters A-B:`.
fn header(line: &str) -> Option<(&str, u64, u64)> {
    let rest = line.strip_prefix("File \"")?;
    let (path, rest) = rest.split_once("\", line")?;
    let rest = rest.strip_prefix('s').unwrap_or(rest).trim_start();
    let rest = rest.strip_suffix(':')?;
    let (lines, chars) = match rest.split_once(", characters ") {
        Some((lines, chars)) => (lines, Some(chars)),
        None => (rest, None),
    };
    let row = lines.split('-').next()?.trim().parse::<u64>().ok()?;
    let col = match chars {
        Some(chars) => chars.split('-').next()?.trim().parse::<u64>().ok()? + 1,
        None => 1,
    };
    Some((path, row, col))
}

fn flush(pending: &mut Option<Pending>, found: &mut Vec<FileFinding>) {
    let Some(done) = pending.take() else {
        return;
    };
    let joined = done.message.join(" ");
    let joined = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some((severity, rule, message)) = classify(&joined) else {
        return;
    };
    found.push(finding(
        TOOL,
        &done.file,
        rule,
        message,
        severity,
        TextPosition {
            line: done.line,
            column: done.column,
        },
        ColumnEncoding::Utf8,
    ));
}

/// Splits `Error: m`, `Error (warning 26 [unused-var]): m`, `Warning 26 [unused-var]: m`
/// and `Alert deprecated: m` into severity, rule and message.
fn classify(text: &str) -> Option<(ToolSeverity, String, String)> {
    if let Some(message) = text.strip_prefix("Error:") {
        return Some((
            ToolSeverity::Error,
            "compile".to_owned(),
            message.trim().to_owned(),
        ));
    }
    if let Some(rest) = text.strip_prefix("Error (") {
        let (what, message) = rest.split_once("):")?;
        return Some((
            ToolSeverity::Error,
            rule_of(what),
            message.trim().to_owned(),
        ));
    }
    if let Some(rest) = text.strip_prefix("Warning ") {
        let (what, message) = rest.split_once(':')?;
        return Some((
            ToolSeverity::Warning,
            rule_of(&format!("warning {what}")),
            message.trim().to_owned(),
        ));
    }
    if let Some(rest) = text.strip_prefix("Alert ") {
        let (name, message) = rest.split_once(':')?;
        return Some((
            ToolSeverity::Warning,
            format!("alert-{}", name.trim()),
            message.trim().to_owned(),
        ));
    }
    None
}

/// The bracketed warning name, or `warning-N`, or `alert-name` for `alert name`.
fn rule_of(what: &str) -> String {
    if let (Some(open), Some(close)) = (what.find('['), what.rfind(']')) {
        if open < close {
            return what[open + 1..close].to_owned();
        }
    }
    let mut words = what.split_whitespace();
    match (words.next(), words.next()) {
        (Some(kind), Some(id)) => format!("{kind}-{id}"),
        _ => "compile".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["ocaml/a.ml", "ocaml/a.mli"];

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_ocaml_compile(b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn a_type_error_is_reported_with_a_one_based_column_and_joined_message() {
        let err = "File \"ocaml/a.ml\", line 2, characters 12-15:\nError: This constant has type string but an expression was expected of type\n         int\n";
        let found = parse_ocaml_compile(err.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "ocaml/a.ml");
        assert_eq!(found[0].finding.rule_id, "compile");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 13);
        assert_eq!(
            found[0].finding.message,
            "This constant has type string but an expression was expected of type int"
        );
    }

    #[test]
    fn a_multi_line_span_starts_at_its_first_line() {
        let err = "File \"ocaml/a.mli\", lines 3-5, characters 2-10:\nError: Signature mismatch\n";
        let found = parse_ocaml_compile(err.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found[0].file, "ocaml/a.mli");
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 3);
    }

    #[test]
    fn warnings_and_warnings_as_errors_carry_the_warning_name() {
        let err = "File \"ocaml/a.ml\", line 1, characters 4-5:\nWarning 26 [unused-var]: unused variable x.\nFile \"ocaml/a.ml\", line 2, characters 0-1:\nError (warning 32 [unused-value-declaration]): unused value y.\n";
        let found = parse_ocaml_compile(err.as_bytes(), Some(2), &FILES).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.rule_id, "unused-var");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[1].finding.rule_id, "unused-value-declaration");
        assert_eq!(found[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn warnings_on_a_clean_exit_are_findings() {
        let err = "File \"ocaml/a.ml\", line 1, characters 4-5:\nWarning 26 [unused-var]: unused variable x.\n";
        let found = parse_ocaml_compile(err.as_bytes(), Some(0), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn alerts_are_warnings() {
        let err = "File \"ocaml/a.ml\", line 1, characters 0-3:\nAlert deprecated: module Foo\nUse Bar.\n";
        let found = parse_ocaml_compile(err.as_bytes(), Some(0), &FILES).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "alert-deprecated");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn failures_are_errors() {
        assert!(parse_ocaml_compile(b"", Some(2), &FILES).is_err());
        assert!(parse_ocaml_compile(b"", None, &FILES).is_err());
        assert!(parse_ocaml_compile(b"Error: Unbound module X\n", Some(2), &FILES).is_err());
        let warn = "File \"ocaml/a.ml\", line 1, characters 4-5:\nWarning 26 [unused-var]: unused variable x.\n";
        assert!(parse_ocaml_compile(warn.as_bytes(), Some(2), &FILES).is_err());
        let err = "File \"ocaml/a.ml\", line 1, characters 0-1:\nError: bad\n";
        assert!(parse_ocaml_compile(err.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_ocaml_compile(&[0xff], Some(2), &FILES).is_err());
    }

    #[test]
    fn a_diagnostic_in_a_foreign_file_is_rejected() {
        let err =
            "File \"bazel-out/k8/bin/ocaml/__obazl/A.ml\", line 2, characters 0-1:\nError: bad\n";
        let error = parse_ocaml_compile(err.as_bytes(), Some(2), &FILES).expect_err("foreign");
        assert!(matches!(error, ParseError::UnknownFile { .. }), "{error}");
        assert!(error.to_string().contains("__obazl/A.ml"), "{error}");
    }
}
