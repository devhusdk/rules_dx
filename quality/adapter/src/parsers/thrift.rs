use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "thrift";

/// What one `thrift -strict -v --gen json` run reported about one schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThriftReport {
    /// Errors and warnings the compiler printed for the checked files.
    pub findings: Vec<FileFinding>,
    /// Every file the compiler scanned for includes, in order, including the
    /// checked file itself.
    pub dependencies: Vec<String>,
}

struct Tagged<'a> {
    path: &'a str,
    line: u64,
    rest: &'a str,
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Reads `[KIND:path:line] rest` for the given `KIND`.
fn tagged<'a>(line: &'a str, kind: &str) -> Option<Tagged<'a>> {
    let body = line
        .strip_prefix('[')?
        .strip_prefix(kind)?
        .strip_prefix(':')?;
    let (head, rest) = body.split_once(']')?;
    let (path, row) = head.rsplit_once(':')?;
    Some(Tagged {
        path,
        line: number(row)?,
        rest: rest.trim(),
    })
}

fn finding(file: &str, severity: ToolSeverity, line: u64, message: &str) -> FileFinding {
    let rule = if severity == ToolSeverity::Error {
        "error"
    } else {
        "warning"
    };
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message: message.to_owned(),
            severity,
            start: TextPosition {
                line: line.max(1),
                column: 1,
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Utf8,
            related: Vec::new(),
        },
    }
}

/// The `Type "X" not defined` line the compiler prints on stdout (exit 1).
fn undefined_type(line: &str) -> bool {
    line.strip_prefix("Type \"")
        .and_then(|rest| rest.strip_suffix("\" not defined"))
        .is_some_and(|name| !name.is_empty())
}

/// Parses stdout and stderr of one `thrift -strict -v --gen json` run over one
/// schema.
///
/// Warnings go to stdout, errors to stderr. Findings in included schemas
/// belong to their own targets, so only the checked files report here; an
/// error in an include still fails the run through a finding on the first
/// checked file. Output the grammar cannot read is never clean: exit 0 allows
/// warnings only, exit 1 needs an error line (a usage error also exits 1 and
/// has none), and anything else, including a signal, is a shape error.
pub fn parse_thrift(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<ThriftReport, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let out =
        std::str::from_utf8(stdout).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let err =
        std::str::from_utf8(stderr).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let first = files
        .first()
        .ok_or_else(|| ParseError::shape(TOOL, "no checked file".to_owned()))?;
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut dependencies = Vec::new();
    let mut errors = 0usize;
    let mut own_errors = 0usize;
    let mut foreign_error: Option<String> = None;

    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Parsing ") && trimmed.ends_with(" for types")
            || trimmed.starts_with("Program: ")
            || trimmed.starts_with("Generating \"")
        {
            continue;
        }
        if let Some(path) = trimmed
            .strip_prefix("Scanning ")
            .and_then(|rest| rest.strip_suffix(" for includes"))
        {
            dependencies.push(path.to_owned());
            continue;
        }
        if undefined_type(trimmed) {
            errors += 1;
            own_errors += 1;
            findings.push(finding(first, ToolSeverity::Error, 1, trimmed));
            continue;
        }
        let Some(found) = tagged(trimmed, "WARNING") else {
            return Err(ParseError::shape(
                TOOL,
                format!("unreadable line: {trimmed}"),
            ));
        };
        if found.rest.is_empty() {
            return Err(ParseError::shape(
                TOOL,
                format!("malformed finding: {trimmed}"),
            ));
        }
        let Some(file) = files.iter().find(|file| **file == found.path) else {
            continue;
        };
        // The include pass and the type pass print the same warning.
        let duplicate = findings.iter().any(|known| {
            known.file == **file
                && known.finding.severity == ToolSeverity::Warning
                && known.finding.start.line == found.line.max(1)
                && known.finding.message == found.rest
        });
        if !duplicate {
            findings.push(finding(file, ToolSeverity::Warning, found.line, found.rest));
        }
    }

    let mut pending: Option<(Tagged<'_>, String)> = None;
    let mut flush = |pending: &mut Option<(Tagged<'_>, String)>,
                     findings: &mut Vec<FileFinding>|
     -> Result<(), ParseError> {
        let Some((at, message)) = pending.take() else {
            return Ok(());
        };
        let message = message.trim().to_owned();
        if message.is_empty() {
            return Err(ParseError::shape(
                TOOL,
                format!("error without a message at {}:{}", at.path, at.line),
            ));
        }
        errors += 1;
        if let Some(file) = files.iter().find(|file| **file == at.path) {
            own_errors += 1;
            findings.push(finding(file, ToolSeverity::Error, at.line, &message));
        } else if foreign_error.is_none() {
            foreign_error = Some(format!("{}:{}: {message}", at.path, at.line));
        }
        Ok(())
    };
    for line in err.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(found) = tagged(trimmed, "FAILURE") {
            flush(&mut pending, &mut findings)?;
            // A failure never has a continuation line, and `exit` follows it.
            let message = found.rest.to_owned();
            pending = Some((found, message));
            flush(&mut pending, &mut findings)?;
            continue;
        }
        if let Some(found) = tagged(trimmed, "ERROR") {
            flush(&mut pending, &mut findings)?;
            // The message follows on the next line; the rest is the last token.
            pending = Some((found, String::new()));
            continue;
        }
        match pending.as_mut() {
            Some((_, message)) => {
                if !message.is_empty() {
                    message.push(' ');
                }
                message.push_str(trimmed);
            }
            None => {
                return Err(ParseError::shape(
                    TOOL,
                    format!("unreadable line: {trimmed}"),
                ));
            }
        }
    }
    flush(&mut pending, &mut findings)?;

    if code == Some(1) && own_errors == 0 {
        let Some(detail) = foreign_error else {
            return Err(ParseError::shape(
                TOOL,
                "exit 1 with no error finding".to_owned(),
            ));
        };
        findings.push(finding(
            first,
            ToolSeverity::Error,
            1,
            &format!("blocked by an error in an included schema: {detail}"),
        ));
    }
    if code == Some(0) && errors > 0 {
        return Err(ParseError::shape(
            TOOL,
            format!("{errors} error(s) with exit 0"),
        ));
    }
    if code == Some(0) && dependencies.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            "exit 0 without any scanned file".to_owned(),
        ));
    }
    Ok(ThriftReport {
        findings,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/pkg/a.thrift";

    fn clean() -> Vec<u8> {
        b"Scanning /s/pkg/a.thrift for includes\n\
Scanning /s/pkg/b.thrift for includes\n\
Parsing /s/pkg/b.thrift for types\n\
Parsing /s/pkg/a.thrift for types\n\
Program: /s/pkg/a.thrift\n\
Generating \"json\"\n"
            .to_vec()
    }

    #[test]
    fn clean_run_lists_every_scanned_file() {
        let report = parse_thrift(&clean(), b"", Some(0), &[FILE]).expect("clean");
        assert!(report.findings.is_empty());
        assert_eq!(report.dependencies, ["/s/pkg/a.thrift", "/s/pkg/b.thrift"]);
    }

    #[test]
    fn warning_is_a_finding_and_is_reported_once() {
        let mut out = b"Scanning /s/pkg/a.thrift for includes\n\
[WARNING:/s/pkg/a.thrift:3] 64-bit constant \"5000000000\" may not work in all languages.\n\n\
Parsing /s/pkg/a.thrift for types\n\
[WARNING:/s/pkg/a.thrift:3] 64-bit constant \"5000000000\" may not work in all languages.\n\n\
Program: /s/pkg/a.thrift\n"
            .to_vec();
        out.extend_from_slice(b"Generating \"json\"\n");
        let report = parse_thrift(&out, b"", Some(0), &[FILE]).expect("warning");
        assert_eq!(report.findings.len(), 1);
        let found = &report.findings[0];
        assert_eq!(found.file, FILE);
        assert_eq!(found.finding.severity, ToolSeverity::Warning);
        assert_eq!(found.finding.start, TextPosition { line: 3, column: 1 });
        assert!(found.finding.message.starts_with("64-bit constant"));
    }

    #[test]
    fn syntax_error_is_a_finding() {
        let out = b"Scanning /s/pkg/a.thrift for includes\n";
        let err = b"[ERROR:/s/pkg/a.thrift:2] (last token was '')\nsyntax error\n\
[FAILURE:/s/pkg/a.thrift:2] Parser error during include pass.";
        let report = parse_thrift(out, err, Some(1), &[FILE]).expect("findings");
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].finding.message, "syntax error");
        assert_eq!(report.findings[0].finding.start.line, 2);
        assert_eq!(report.findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn undefined_type_is_an_error_on_the_checked_file() {
        let mut out = clean();
        out.extend_from_slice(b"Type \"Nope\" not defined\n");
        let report = parse_thrift(&out, b"", Some(1), &[FILE]).expect("findings");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].finding.message,
            "Type \"Nope\" not defined"
        );
        assert_eq!(report.findings[0].file, FILE);
    }

    #[test]
    fn missing_include_failure_is_a_finding() {
        let err = b"[FAILURE:/s/pkg/a.thrift:1] Could not find include file missing.thrift";
        let report = parse_thrift(b"", err, Some(1), &[FILE]).expect("findings");
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0]
            .finding
            .message
            .contains("missing.thrift"));
    }

    #[test]
    fn error_in_an_include_blocks_the_checked_file() {
        let err = b"[ERROR:/s/pkg/b.thrift:4] (last token was '}')\nbad field\n";
        let report = parse_thrift(b"", err, Some(1), &[FILE]).expect("findings");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].file, FILE);
        assert!(report.findings[0]
            .finding
            .message
            .contains("/s/pkg/b.thrift:4: bad field"));
    }

    #[test]
    fn unreadable_stdout_is_a_shape_error() {
        let mut out = clean();
        out.extend_from_slice(b"something new\n");
        assert!(parse_thrift(&out, b"", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn stray_stderr_with_exit_zero_is_a_shape_error() {
        assert!(parse_thrift(&clean(), b"surprise\n", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn error_with_exit_zero_is_a_shape_error() {
        let err = b"[ERROR:/s/pkg/a.thrift:1] (last token was 'x')\nboom\n";
        assert!(parse_thrift(&clean(), err, Some(0), &[FILE]).is_err());
    }

    #[test]
    fn usage_error_with_exit_one_is_not_clean() {
        let err = b"Missing file name\nUsage: thrift [options] file\n\nUse thrift -help for a list of options\n";
        assert!(parse_thrift(b"", err, Some(1), &[FILE]).is_err());
        assert!(parse_thrift(b"", b"", Some(1), &[FILE]).is_err());
    }

    #[test]
    fn empty_exit_zero_run_is_not_clean() {
        assert!(parse_thrift(b"", b"", Some(0), &[FILE]).is_err());
    }

    #[test]
    fn process_errors_are_shape_errors() {
        assert!(parse_thrift(b"", b"", None, &[FILE]).is_err());
        assert!(parse_thrift(b"", b"", Some(2), &[FILE]).is_err());
    }
}
