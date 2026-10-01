use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_standardrb(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("standardrb", stdout, code, files, DiffExit::NonZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "--- a/Sample.rb\n+++ b/Sample.rb\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
    #[test]
    fn standardrb_reports_diff_files() {
        let findings = parse_standardrb(DIRTY.as_bytes(), Some(1), &["Sample.rb"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_standardrb(b"", Some(0), &["Sample.rb"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_standardrb(b"", Some(1), &["Sample.rb"]).is_err());
        assert!(parse_standardrb(DIRTY.as_bytes(), Some(1), &["other.rb"]).is_err());
        assert!(parse_standardrb(&[0xff], Some(0), &["x"]).is_err());
    }
}
