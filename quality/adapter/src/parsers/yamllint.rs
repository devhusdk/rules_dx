use super::{
    columned, finding, lines, located, missing, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

/// `yamllint -f parsable` prints `<path>:<line>:<column>: [<level>] <message> (<rule>)`.
pub fn parse_yamllint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "yamllint";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let (line_no, column, tail) = columned(TOOL, line, rest)?;
        let (severity, text) = if let Some(text) = tail.strip_prefix("[error] ") {
            (ToolSeverity::Error, text)
        } else if let Some(text) = tail.strip_prefix("[warning] ") {
            (ToolSeverity::Warning, text)
        } else {
            return Err(missing(TOOL, "level", line));
        };
        let (message, rule_id) = match text
            .strip_suffix(')')
            .and_then(|body| body.rsplit_once(" ("))
        {
            Some((message, rule)) if !rule.is_empty() => (message, rule),
            _ => (text, ""),
        };
        if message.trim().is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        findings.push(finding(
            TOOL,
            file,
            rule_id.to_owned(),
            message.trim().to_owned(),
            severity,
            TextPosition {
                line: line_no,
                column,
            },
            ColumnEncoding::Scalars,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "Sample.yaml:1:1: [warning] missing document start \"---\" (document-start)\nSample.yaml:2:7: [error] too few spaces after comma (commas)\nSample.yaml:2:2: [error] syntax error: expected ',', but got ':' (syntax)\n";
    #[test]
    fn yamllint_reports_level_message_and_rule() {
        let findings = parse_yamllint(DIRTY.as_bytes(), Some(1), &["Sample.yaml"]).expect("parsed");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].finding.rule_id, "document-start");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.message,
            "missing document start \"---\""
        );
        assert_eq!(findings[1].finding.rule_id, "commas");
        assert_eq!(findings[1].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].finding.start.column, 7);
        assert_eq!(findings[2].finding.rule_id, "syntax");
        let warned = parse_yamllint(
            DIRTY.lines().next().unwrap().as_bytes(),
            Some(0),
            &["Sample.yaml"],
        )
        .expect("warnings exit 0");
        assert_eq!(warned.len(), 1);
    }
    #[test]
    fn yamllint_rejects_output_outside_the_grammar() {
        let clean = parse_yamllint(b"", Some(0), &["Sample.yaml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_yamllint(b"", Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamllint(DIRTY.as_bytes(), Some(1), &["other.yaml"]).is_err());
        assert!(parse_yamllint(
            b"Sample.yaml:1:1: [info] x (r)\n",
            Some(1),
            &["Sample.yaml"]
        )
        .is_err());
        assert!(parse_yamllint(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn yamllint_reads_drive_rooted_paths() {
        let file = "C:\\tmp\\a.yaml";
        let findings = parse_yamllint(
            "C:\\tmp\\a.yaml:3:1: [warning] missing document start \"---\" (document-start)\n"
                .as_bytes(),
            Some(1),
            &[file],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, file);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 1);
    }
}
