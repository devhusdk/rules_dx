use super::{
    columned, finding, lines, located, missing, require_findings, FileFinding, ParseError,
};
use crate::ToolSeverity;

pub fn parse_govet(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "govet";
    let mut findings = Vec::new();
    for line in lines(TOOL, stderr)? {
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
            line_no,
            column,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str =
        "go/tests/fixtures/govet/Sample.go:7:2: non-constant format string in call to fmt.Printf\n";

    #[test]
    fn govet_reports_text_diagnostics() {
        let findings = parse_govet(
            DIRTY.as_bytes(),
            Some(1),
            &["go/tests/fixtures/govet/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go/tests/fixtures/govet/Sample.go");
        assert_eq!(
            findings[0].finding.message,
            "non-constant format string in call to fmt.Printf"
        );
        let clean =
            parse_govet(b"", Some(0), &["go/tests/fixtures/govet/Sample.go"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_govet(b"", Some(1), &["go/tests/fixtures/govet/Sample.go"]).is_err());
        assert!(parse_govet(DIRTY.as_bytes(), Some(1), &["other.go"]).is_err());
        assert!(parse_govet(b"nope\n", Some(1), &["x"]).is_err());
        assert!(parse_govet(&[0xff], Some(1), &["x"]).is_err());
    }
}
