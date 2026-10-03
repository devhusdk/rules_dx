use super::{listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_google_java_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths(
        "google_java_format",
        stdout,
        code,
        files,
        &[],
        Spelling::Exact,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_java_format_reports_paths() {
        let stdout = "/s/Dirty.java\n";
        let findings = parse_google_java_format(stdout.as_bytes(), Some(1), &["/s/Dirty.java"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/Dirty.java");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_google_java_format(b"", Some(0), &["/s/Dirty.java"]).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_google_java_format(b"", Some(1), &["/s/Dirty.java"]).is_err());
        assert!(parse_google_java_format(b"/s/other.java\n", Some(1), &["/s/Dirty.java"]).is_err());
        assert!(parse_google_java_format(&[0xff], Some(1), &["/s/Dirty.java"]).is_err());
    }
}
