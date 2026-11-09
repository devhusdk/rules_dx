use super::{FileFinding, ParseError, as_text, code_name, finding, known_native, unformatted_at};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "helmfmt";
const UNFORMATTED: &str = "[UNFORMATTED] ";
const ERROR: &str = "[ERROR]  ";
const CONFIG_WARNING: &str = "Warning: Error parsing config";
const SYNTAX: &str = "Invalid syntax ";

/// `helmfmt --check` exits 0 when every file is formatted and exits 1 for unformatted files and
/// template errors alike. Exit 1 is read from stderr records: `[UNFORMATTED] <path>` and
/// `[ERROR]  [Invalid syntax ]<path>: <message>`, followed by a blank line and a count line. Anything
/// else is a shape error and never reads as clean.
pub fn parse_helmfmt(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    if err.lines().any(|line| line.starts_with(CONFIG_WARNING)) {
        return Err(ParseError::shape(TOOL, "config rejected"));
    }
    match code {
        Some(0) if !err.is_empty() => Err(ParseError::shape(TOOL, "exit 0 with stderr")),
        Some(0) if out.contains(UNFORMATTED) || out.contains(ERROR) => {
            Err(ParseError::shape(TOOL, "exit 0 with a record"))
        }
        Some(0) => Ok(Vec::new()),
        Some(1) if !out.is_empty() => Err(ParseError::shape(TOOL, "unexpected stdout")),
        Some(1) => records(err, files),
        _ => Err(ParseError::shape(TOOL, format!("exit {}", code_name(code)))),
    }
}

fn records(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in stderr.lines() {
        if line.trim().is_empty() || is_count(line) {
            continue;
        }
        if let Some(path) = line.strip_prefix(UNFORMATTED) {
            let file = known_native(TOOL, files, path)?;
            if !findings.iter().any(|f| f.file == file) {
                findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
            }
        } else if let Some(rest) = line.strip_prefix(ERROR) {
            findings.push(failure(rest.strip_prefix(SYNTAX).unwrap_or(rest), files)?);
        } else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognized stderr line: {line}"),
            ));
        }
    }
    if findings.is_empty() {
        return Err(ParseError::shape(TOOL, "exit 1 without a record"));
    }
    Ok(findings)
}

/// `<n> file(s) need formatting, <m> error(s)`.
fn is_count(line: &str) -> bool {
    line.contains(" file(s) need formatting, ") && line.ends_with(" error(s)")
}

fn failure(record: &str, files: &[&str]) -> Result<FileFinding, ParseError> {
    let located = files.iter().find_map(|file| {
        let message = record.strip_prefix(file)?.strip_prefix(": ")?;
        Some((*file, message))
    });
    let Some((file, message)) = located else {
        return Err(ParseError::shape(
            TOOL,
            format!("error without a known file: {record}"),
        ));
    };
    Ok(finding(
        TOOL,
        file,
        String::new(),
        message.to_owned(),
        ToolSeverity::Error,
        TextPosition {
            line: template_line(message),
            column: 1,
        },
        ColumnEncoding::Scalars,
    ))
}

/// The line a Go template error names: `template: <name>:<line>: <message>`.
fn template_line(message: &str) -> u64 {
    message
        .split_once("template: ")
        .and_then(|(_, rest)| rest.split_once(':'))
        .and_then(|(_, rest)| rest.split_once(':'))
        .and_then(|(row, _)| row.parse::<u64>().ok())
        .map_or(1, |row| row.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/s/a.yaml", "/s/b.yaml", "/s/c.tpl"];
    const SYNTAX_ERROR: &str = "[ERROR]  Invalid syntax /s/b.yaml: invalid template syntax: template: validation:2: unexpected \"}\" in operand\n";
    const COUNT: &str = "\n1 file(s) need formatting, 1 error(s)\n";

    fn run(err: &str, code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_helmfmt(b"", err.as_bytes(), code, &FILES)
    }

    #[test]
    fn helmfmt_config_parse_warning_is_a_tool_error() {
        let warning = "Warning: Error parsing config /s/.helmfmt: invalid character\n";
        assert!(run(warning, Some(0)).is_err());
        assert!(run(&format!("{warning}[UNFORMATTED] /s/a.yaml\n"), Some(1)).is_err());
    }

    #[test]
    fn helmfmt_clean_is_empty() {
        let clean = b"All 3 file(s) are properly formatted\n";
        assert!(
            parse_helmfmt(clean, b"", Some(0), &FILES)
                .expect("ok")
                .is_empty()
        );
        assert!(run("", Some(0)).expect("ok").is_empty());
    }

    #[test]
    fn helmfmt_reports_unformatted_files() {
        let findings = run(
            "[UNFORMATTED] /s/a.yaml\n[UNFORMATTED] /s/c.tpl\n\n2 file(s) need formatting, 0 error(s)\n",
            Some(1),
        )
        .expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.yaml");
        assert_eq!(findings[1].file, "/s/c.tpl");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn helmfmt_locates_a_syntax_error() {
        let findings = run(&format!("{SYNTAX_ERROR}{COUNT}"), Some(1)).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/b.yaml");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 2);
        assert!(findings[0].finding.message.contains("unexpected"));
    }

    #[test]
    fn helmfmt_reads_an_io_error_without_the_syntax_prefix() {
        let findings = run(
            "[ERROR]  /s/a.yaml: open /s/a.yaml: permission denied\n",
            Some(1),
        )
        .expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 1);
    }

    #[test]
    fn helmfmt_reads_mixed_records() {
        let err = format!("[UNFORMATTED] /s/a.yaml\n{SYNTAX_ERROR}{COUNT}");
        let findings = run(&err, Some(1)).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn helmfmt_never_reports_a_failure_as_clean() {
        assert!(run("", Some(1)).is_err());
        assert!(run("", Some(2)).is_err());
        assert!(run("", None).is_err());
        assert!(run(COUNT, Some(1)).is_err());
        assert!(run("boom\n", Some(1)).is_err());
        assert!(run("warning\n", Some(0)).is_err());
        assert!(run("[UNFORMATTED] /s/a.yaml\n", Some(0)).is_err());
        assert!(run("[UNFORMATTED] /s/other.yaml\n", Some(1)).is_err());
        assert!(run("[ERROR]  Invalid syntax /s/other.yaml: x\n", Some(1)).is_err());
        assert!(run("stray\n[UNFORMATTED] /s/a.yaml\n", Some(1)).is_err());
        assert!(parse_helmfmt(b"x\n", b"[UNFORMATTED] /s/a.yaml\n", Some(1), &FILES).is_err());
        assert!(parse_helmfmt(b"[UNFORMATTED] /s/a.yaml\n", b"", Some(0), &FILES).is_err());
        assert!(parse_helmfmt(&[0xff], b"", Some(0), &FILES).is_err());
    }
}
