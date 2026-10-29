use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "xmllint";

/// How many lines after a finding belong to it: the source line and the caret line.
const CONTEXT_LINES: usize = 2;

struct Header {
    file: String,
    line: u64,
    rule: String,
    severity: ToolSeverity,
    message: String,
}

/// `<file>:<line>: <domain> error|warning : <message>`. The file is found by prefix against the
/// names that were passed, never by splitting on `:`, so a Windows drive letter is safe. Only the
/// line structure is read, never the wording of the message.
fn header(files: &[&str], line: &str) -> Option<Header> {
    let file = files
        .iter()
        .copied()
        .filter(|file| {
            line.strip_prefix(*file)
                .is_some_and(|rest| rest.starts_with(':'))
        })
        .max_by_key(|file| file.len())?;
    let rest = &line[file.len() + 1..];
    let tail = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    let number = &rest[..rest.len() - tail.len()];
    let tail = tail.strip_prefix(": ")?;
    let (head, message) = tail.split_once(" : ")?;
    let (domain, severity) = if let Some(domain) = head.strip_suffix(" error") {
        (domain, ToolSeverity::Error)
    } else if let Some(domain) = head.strip_suffix(" warning") {
        (domain, ToolSeverity::Warning)
    } else {
        return None;
    };
    if domain.is_empty() {
        return None;
    }
    Some(Header {
        file: file.to_owned(),
        line: number.parse().ok()?,
        rule: domain.replace(' ', "_"),
        severity,
        message: message.to_owned(),
    })
}

/// `I/O warning : failed to load "<file>": <reason>` names a passed file xmllint could not read.
fn unreadable(files: &[&str], line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("I/O warning : failed to load \"")?;
    let file = files
        .iter()
        .copied()
        .filter(|file| {
            rest.strip_prefix(*file)
                .is_some_and(|tail| tail.starts_with("\":"))
        })
        .max_by_key(|file| file.len())?;
    Some((file.to_owned(), rest[file.len() + 2..].trim().to_owned()))
}

fn finding(
    file: String,
    line: u64,
    rule: String,
    severity: ToolSeverity,
    message: String,
) -> FileFinding {
    FileFinding {
        file,
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule,
            message,
            severity,
            start: TextPosition {
                line: line.max(1),
                column: 1,
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

/// `xmllint --noout` prints nothing and exits 0 for a well-formed file. Problems go to stderr as
/// a header line followed by the source line and a caret line. The exit code is not a reliable
/// verdict (a parse error exits 4 and a namespace error exits 0), so the findings decide; the
/// exit may only be 0, 1 or 4, a non-zero exit needs a finding, and a clean exit needs empty
/// stderr. Any line that is not a header, an unreadable-file notice or the context of a header
/// is an error here and never a clean result.
pub fn parse_xmllint(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stderr)?;
    if !matches!(code, Some(0 | 1 | 4)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let text = std::str::from_utf8(stderr)
        .map_err(|err| ParseError::shape(TOOL, format!("stderr is not UTF-8: {err}")))?;
    let mut findings = Vec::new();
    let mut context_left = 0;
    for raw in text.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(head) = header(files, line) {
            findings.push(finding(
                head.file,
                head.line,
                head.rule,
                head.severity,
                head.message,
            ));
            context_left = CONTEXT_LINES;
        } else if let Some((file, reason)) = unreadable(files, line) {
            findings.push(finding(
                file,
                1,
                "io".to_owned(),
                ToolSeverity::Error,
                format!("cannot read file: {reason}"),
            ));
            context_left = 0;
        } else if context_left > 0 {
            context_left -= 1;
        } else {
            return Err(ParseError::shape(
                TOOL,
                format!("unrecognised output line: {line}"),
            ));
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with no findings", code_name(code)),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/s/xml/a.xml", "/s/xml/b.xml", "C:\\s\\xml\\c.xml"];

    const MISMATCH: &str = "/s/xml/a.xml:1: parser error : Opening and ending tag mismatch: b line 1 and a\n<a><b></a>\n          ^\n";

    #[test]
    fn xmllint_is_clean_on_exit_zero_and_empty_stderr() {
        assert!(parse_xmllint(b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn xmllint_locates_a_parser_error_despite_exit_four() {
        let found = parse_xmllint(MISMATCH.as_bytes(), Some(4), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/xml/a.xml");
        assert_eq!(found[0].finding.tool_id, "xmllint");
        assert_eq!(found[0].finding.rule_id, "parser");
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start, TextPosition { line: 1, column: 1 });
        assert!(found[0].finding.message.starts_with("Opening and ending"));
    }

    #[test]
    fn xmllint_reports_each_fixture_shape() {
        let entity =
            "/s/xml/a.xml:1: parser error : Entity 'foo' not defined\n<a>&foo;</a>\n        ^\n";
        let extra = "/s/xml/a.xml:1: parser error : Extra content at the end of the document\n<a/><b/>\n    ^\n";
        for text in [entity, extra] {
            let found = parse_xmllint(text.as_bytes(), Some(4), &FILES).expect("parsed");
            assert_eq!(found.len(), 1);
        }
        // A namespace error exits 0 and is still a finding.
        let namespace = "/s/xml/a.xml:1: namespace error : Namespace prefix r for z on a is not defined\n<a r:z=\"2\"/>\n            ^\n";
        let found = parse_xmllint(namespace.as_bytes(), Some(0), &FILES).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "namespace");
        let warning = "/s/xml/a.xml:3: parser warning : odd\nx\n^\n";
        let found = parse_xmllint(warning.as_bytes(), Some(0), &FILES).expect("parsed");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 3);
    }

    #[test]
    fn xmllint_handles_a_batch_with_a_missing_and_a_broken_file() {
        let text = format!(
            "{MISMATCH}I/O warning : failed to load \"/s/xml/b.xml\": No such file or directory\n"
        );
        let found = parse_xmllint(text.as_bytes(), Some(4), &FILES).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].file, "/s/xml/b.xml");
        assert_eq!(found[1].finding.rule_id, "io");
    }

    #[test]
    fn xmllint_matches_windows_paths_and_crlf() {
        let text = "C:\\s\\xml\\c.xml:2: parser error : bad\r\n<a>\r\n ^\r\n";
        let found = parse_xmllint(text.as_bytes(), Some(4), &FILES).expect("parsed");
        assert_eq!(found[0].file, "C:\\s\\xml\\c.xml");
        assert_eq!(found[0].finding.start.line, 2);
    }

    #[test]
    fn xmllint_fails_closed() {
        // Exit and output disagree.
        assert!(parse_xmllint(b"", Some(4), &FILES).is_err());
        assert!(parse_xmllint(b"", Some(1), &FILES).is_err());
        assert!(parse_xmllint(MISMATCH.as_bytes(), None, &FILES).is_err());
        // Tool errors are never a clean result.
        for code in [2, 3, 5, 6, 7, 8, 9, 10, 11, 127] {
            assert!(parse_xmllint(b"", Some(code), &FILES).is_err());
            assert!(parse_xmllint(MISMATCH.as_bytes(), Some(code), &FILES).is_err());
        }
        assert!(parse_xmllint(b"", None, &FILES).is_err());
        // Unknown text, a finding for a file that was not passed, and non UTF-8.
        assert!(parse_xmllint(b"Usage : xmllint\n", Some(0), &FILES).is_err());
        assert!(
            parse_xmllint(b"/s/other.xml:1: parser error : x\na\n^\n", Some(4), &FILES).is_err()
        );
        assert!(parse_xmllint(&[0xff, 0xfe], Some(4), &FILES).is_err());
        assert!(parse_xmllint(MISMATCH.as_bytes(), Some(4), &["/s/other.xml"]).is_err());
        // Clean exit with stderr noise.
        assert!(parse_xmllint(b"warning\n", Some(0), &FILES).is_err());
    }
}
