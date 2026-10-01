use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_cue(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("cue", stdout, code, files, DiffExit::ZeroOrOne)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str =
        "--- cue/Sample.cue.orig\n+++ cue/Sample.cue\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";

    #[test]
    fn cue_reports_diff_files() {
        let findings = parse_cue(DIRTY.as_bytes(), Some(1), &["cue/Sample.cue"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cue/Sample.cue");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_cue(b"", Some(0), &["cue/Sample.cue"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_cue(b"", Some(1), &["cue/Sample.cue"]).is_err());
        assert!(parse_cue(DIRTY.as_bytes(), Some(1), &["other.cue"]).is_err());
        assert!(parse_cue(&[0xff], Some(0), &["x"]).is_err());
    }
}
