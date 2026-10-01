use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_terraform(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("terraform", stdout, code, files, DiffExit::NonZero)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "--- a/main.tf\n+++ b/main.tf\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";
    #[test]
    fn terraform_reports_diff_files() {
        let findings = parse_terraform(DIRTY.as_bytes(), Some(3), &["main.tf"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_terraform(b"", Some(0), &["main.tf"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_terraform(b"", Some(3), &["main.tf"]).is_err());
        assert!(parse_terraform(DIRTY.as_bytes(), Some(3), &["other.tf"]).is_err());
        assert!(parse_terraform(&[0xff], Some(0), &["x"]).is_err());
    }
}
