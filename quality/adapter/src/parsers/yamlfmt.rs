use super::{listed_paths, FileFinding, ParseError, Spelling};

/// The header `yamlfmt -lint -q` prints above the paths it would rewrite.
const HEADER: &str = "The following files had formatting differences:";

pub fn parse_yamlfmt(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("yamlfmt", stderr, code, files, &[HEADER], Spelling::Exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `yamlfmt -lint -q` output, verbatim from yamlfmt 0.21.0 on stderr.
    const DIRTY: &str =
        "The following files had formatting differences:\n\nSample.yaml\nOther.yaml\n";

    #[test]
    fn yamlfmt_reports_listed_files() {
        let findings = parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["Sample.yaml", "Other.yaml"])
            .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "Sample.yaml");
        assert_eq!(findings[1].file, "Other.yaml");
        let clean = parse_yamlfmt(b"", Some(0), &["Sample.yaml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_yamlfmt(b"", Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamlfmt(&[0xff], Some(0), &["x"]).is_err());
    }

    /// `yamlfmt -lint -q` writes the long side-by-side report to stderr, never a unified diff.
    #[test]
    fn yamlfmt_writes_its_report_to_stderr() {
        let report = b"The following formatting differences were found:\n\nSample.yaml:\n- key:    value  key: value\n                 \n\n";
        assert!(parse_yamlfmt(report, Some(1), &["Sample.yaml"]).is_err());
    }
}
