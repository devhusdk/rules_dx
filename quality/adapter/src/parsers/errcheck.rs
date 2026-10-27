use super::{
    columned, finding, lines, located, missing, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

pub fn parse_errcheck(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "errcheck";
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let (file, rest) = located(TOOL, line, files)?;
        let (line_no, column, message) = columned(TOOL, line, rest)?;
        if message.is_empty() {
            return Err(missing(TOOL, "message", line));
        }
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message.to_owned(),
            ToolSeverity::Warning,
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

    const DIRTY: &str =
        "go/tests/fixtures/errcheck/Sample.go:7:2: unchecked error: f.WriteString(\"hello\")\n";

    #[test]
    fn errcheck_reports_text_diagnostics() {
        let findings = parse_errcheck(
            DIRTY.as_bytes(),
            Some(1),
            &["go/tests/fixtures/errcheck/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go/tests/fixtures/errcheck/Sample.go");
        assert_eq!(
            findings[0].finding.message,
            "unchecked error: f.WriteString(\"hello\")"
        );
        let clean = parse_errcheck(b"", Some(0), &["go/tests/fixtures/errcheck/Sample.go"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_errcheck(b"", Some(1), &["go/tests/fixtures/errcheck/Sample.go"]).is_err());
        assert!(parse_errcheck(DIRTY.as_bytes(), Some(1), &["other.go"]).is_err());
        assert!(parse_errcheck(b"nope\n", Some(1), &["x"]).is_err());
        assert!(parse_errcheck(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn errcheck_reads_drive_rooted_paths() {
        let file = "C:\\tmp\\a.go";
        let findings = parse_errcheck(
            "C:\\tmp\\a.go:3:1: unchecked error: f.Close()\n".as_bytes(),
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
