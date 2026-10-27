use super::{check_output_size, sarif::parse_sarif, FileFinding, ParseError};
use crate::ColumnEncoding;

pub fn parse_spotbugs(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("spotbugs", stdout)?;
    parse_sarif("spotbugs", stdout, code, files, ColumnEncoding::Utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[{"ruleId":"NP_NULL_ON_SOME_PATH","level":"warning","message":{"text":"Possible null pointer dereference."},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/Dirty.java"},"region":{"startLine":5,"startColumn":5,"endLine":5,"endColumn":9}}}]}]}]}"#;

    #[test]
    fn spotbugs_reports_sarif_ranges() {
        let findings =
            parse_spotbugs(SARIF.as_bytes(), Some(1), &["/s/Dirty.java"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "NP_NULL_ON_SOME_PATH");
        let clean =
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[]}]}"#;
        let clean = parse_spotbugs(clean.as_bytes(), Some(0), &["/s/Dirty.java"]).expect("clean");
        assert!(clean.is_empty());
        let missing = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[],"invocations":[{"executionSuccessful":false,"toolConfigurationNotifications":[{"level":"error","message":{"text":"Classes needed for analysis were missing: [example.Dependency]"}}]}]}]}"#;
        let err = parse_spotbugs(missing.as_bytes(), Some(0), &["/s/Dirty.java"])
            .expect_err("incomplete");
        assert!(err.to_string().contains("example.Dependency"), "{err}");
        assert!(parse_spotbugs(b"not json", Some(1), &["/s/Dirty.java"]).is_err());
    }
}
