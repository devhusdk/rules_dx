use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_yamlfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("yamlfmt", stdout, code, files, DiffExit::NonZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "--- a/Sample.yaml\n+++ b/Sample.yaml\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
    #[test]
    fn yamlfmt_reports_diff_files() {
        let findings = parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["Sample.yaml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_yamlfmt(b"", Some(0), &["Sample.yaml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_yamlfmt(b"", Some(1), &["Sample.yaml"]).is_err());
        assert!(parse_yamlfmt(DIRTY.as_bytes(), Some(1), &["other.yaml"]).is_err());
        assert!(parse_yamlfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
