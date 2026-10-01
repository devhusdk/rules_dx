use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_shfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("shfmt", stdout, code, files, DiffExit::ZeroOrOne)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIRTY: &str = "--- run.sh.orig\n+++ run.sh\n@@ -1 +1 @@\n-BADFMT\n+fixed";
    #[test]
    fn shfmt_reports_diff_files() {
        let findings = parse_shfmt(DIRTY.as_bytes(), Some(0), &["run.sh"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean = parse_shfmt(b"", Some(0), &["run.sh"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_shfmt(b"", Some(1), &["run.sh"]).is_err());
        assert!(parse_shfmt(DIRTY.as_bytes(), Some(0), &["other.sh"]).is_err());
        assert!(parse_shfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
