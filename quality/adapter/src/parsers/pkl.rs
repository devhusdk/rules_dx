use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_pkl(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("pkl", stdout, code, files, DiffExit::ZeroOrOne)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str =
        "--- a/pkl/Sample.pkl\n+++ b/pkl/Sample.pkl\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
    #[test]
    fn pkl_reports_diff_files() {
        let findings = parse_pkl(DIRTY.as_bytes(), Some(1), &["pkl/Sample.pkl"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_pkl(b"", Some(0), &["pkl/Sample.pkl"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_pkl(b"", Some(1), &["pkl/Sample.pkl"]).is_err());
        assert!(parse_pkl(DIRTY.as_bytes(), Some(1), &["other.pkl"]).is_err());
        assert!(parse_pkl(&[0xff], Some(0), &["x"]).is_err());
    }
}
