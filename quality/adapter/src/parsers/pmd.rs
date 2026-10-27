use super::{check_output_size, sarif::parse_sarif, FileFinding, ParseError};
use crate::ColumnEncoding;

pub fn parse_pmd(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("pmd", stdout)?;
    parse_sarif("pmd", stdout, code, files, ColumnEncoding::Utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"PMD"}},"results":[{"ruleId":"UnusedLocalVariable","level":"warning","message":{"text":"Avoid unused variables."},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/Dirty.java"},"region":{"startLine":4,"startColumn":9,"endLine":4,"endColumn":12}}}]}]}]}"#;

    #[test]
    fn pmd_reports_sarif_ranges() {
        let findings = parse_pmd(SARIF.as_bytes(), Some(1), &["/s/Dirty.java"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "UnusedLocalVariable");
        let end = findings[0].finding.end.expect("extent");
        assert_eq!((end.line, end.column), (4, 12));
        let clean =
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"PMD"}},"results":[]}]}"#;
        let clean = parse_pmd(clean.as_bytes(), Some(0), &["/s/Dirty.java"]).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_pmd(b"not json", Some(1), &["/s/Dirty.java"]).is_err());
    }
}
