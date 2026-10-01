use super::sarif::parse_sarif;
use super::{FileFinding, ParseError};

pub fn parse_roslyn(sarif: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    parse_sarif("roslyn", sarif, Some(0), files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextPosition, ToolSeverity};

    const SINGLE: &str = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "csc"}}, "results": [{"ruleId": "CA1822", "level": "warning", "message": {"text": "Member 'Greet' does not access instance data"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "csharp/tests/fixtures/roslyn/Sample.cs"}, "region": {"startLine": 7, "startColumn": 19, "endLine": 7, "endColumn": 24}}}]}]}]}"#;

    #[test]
    fn roslyn_reports_sarif_results() {
        let findings = parse_roslyn(
            SINGLE.as_bytes(),
            &["csharp/tests/fixtures/roslyn/Sample.cs"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "CA1822");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.start,
            TextPosition {
                line: 7,
                column: 19
            }
        );
        let end = findings[0].finding.end.expect("extent");
        assert_eq!((end.line, end.column), (7, 24));
        let clean = parse_roslyn(
            r#"{"version": "2.1.0", "runs": []}"#.as_bytes(),
            &["csharp/tests/fixtures/roslyn/Sample.cs"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_roslyn(SINGLE.as_bytes(), &["other.cs"]).is_err());
        assert!(parse_roslyn(b"not json", &["x"]).is_err());
        assert!(parse_roslyn(&[0xff], &["x"]).is_err());
        let no_rule = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"","locations":[]}]}]}"#;
        assert!(parse_roslyn(no_rule.as_bytes(), &["x"]).is_err());
    }

    #[test]
    fn roslyn_resolves_base_ids_and_every_location() {
        let log = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"originalUriBaseIds":{"%SRCROOT%":{"uri":"file:///src/"}},"results":[{"ruleId":"CA1822","level":"warning","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"Sample.cs","uriBaseId":"%SRCROOT%"},"region":{"startLine":7,"startColumn":19}}},{"physicalLocation":{"artifactLocation":{"uri":"Other.cs"},"region":{"startLine":9,"startColumn":1}}}]}]}]}"#;
        let findings =
            parse_roslyn(log.as_bytes(), &["/src/Sample.cs", "/src/Other.cs"]).expect("parsed");
        let files: Vec<&str> = findings.iter().map(|found| found.file.as_str()).collect();
        assert_eq!(files, vec!["/src/Sample.cs", "/src/Other.cs"]);
        assert_eq!(
            findings[1].finding.start,
            TextPosition { line: 9, column: 1 }
        );
    }
}
