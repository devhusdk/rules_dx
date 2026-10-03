use super::{listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_ktfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("ktfmt", stdout, code, files, &[], Spelling::Exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ktfmt_reports_paths() {
        let stdout = "/s/Dirty.kt\n";
        let findings = parse_ktfmt(stdout.as_bytes(), Some(1), &["/s/Dirty.kt"]).expect("parsed");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_ktfmt(b"", Some(0), &["/s/Dirty.kt"]).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_ktfmt(b"", Some(1), &["/s/Dirty.kt"]).is_err());
        assert!(parse_ktfmt(b"/s/other.kt\n", Some(1), &["/s/Dirty.kt"]).is_err());
    }
}
