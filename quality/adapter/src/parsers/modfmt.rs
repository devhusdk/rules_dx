use super::{listed_paths, FileFinding, ParseError};

pub fn parse_modfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("modfmt", stdout, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `modfmt -c -l` names each file it would rewrite, on stdout, and exits 1.
    const DIRTY: &str = "go.mod\n";

    #[test]
    fn modfmt_reports_listed_files() {
        let findings = parse_modfmt(DIRTY.as_bytes(), Some(1), &["go.mod"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go.mod");
        let clean = parse_modfmt(b"", Some(0), &["go.mod"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_modfmt(b"", Some(1), &["go.mod"]).is_err());
        assert!(parse_modfmt(DIRTY.as_bytes(), Some(1), &["other.mod"]).is_err());
        assert!(parse_modfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
