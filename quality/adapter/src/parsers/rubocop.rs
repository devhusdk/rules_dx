use super::{rubocop_json, FileFinding, ParseError};

pub fn parse_rubocop(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    rubocop_json("rubocop", stdout, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = r#"{"files": [{"path": "Sample.rb", "offenses": [{"severity": "convention", "message": "Use double quotes", "cop_name": "Style/StringLiterals", "location": {"line": 3, "column": 1}}]}]}"#;
    const CLEAN: &str = r#"{"files": [{"path": "Sample.rb", "offenses": []}]}"#;
    #[test]
    fn rubocop_reports_json_offenses() {
        let findings = parse_rubocop(DIRTY.as_bytes(), Some(1), &["Sample.rb"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "Style/StringLiterals");
        let clean = parse_rubocop(CLEAN.as_bytes(), Some(0), &["Sample.rb"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_rubocop(b"", Some(1), &["Sample.rb"]).is_err());
        assert!(parse_rubocop(DIRTY.as_bytes(), Some(1), &["other.rb"]).is_err());
        assert!(parse_rubocop(&[0xff], Some(0), &["x"]).is_err());
    }
}
