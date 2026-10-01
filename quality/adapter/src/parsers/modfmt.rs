use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_modfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("modfmt", stdout, code, files, DiffExit::ZeroOrOne)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "--- a/go.mod\n+++ b/go.mod\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
    #[test]
    fn modfmt_reports_diff_files() {
        let findings = parse_modfmt(DIRTY.as_bytes(), Some(1), &["go.mod"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_modfmt(b"", Some(0), &["go.mod"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_modfmt(b"", Some(1), &["go.mod"]).is_err());
        assert!(parse_modfmt(DIRTY.as_bytes(), Some(1), &["other.mod"]).is_err());
        assert!(parse_modfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
