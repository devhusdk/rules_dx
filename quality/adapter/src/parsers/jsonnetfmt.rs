use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_jsonnetfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("jsonnetfmt", stdout, code, files, DiffExit::ZeroOrOne)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "--- a/jsonnet/Sample.jsonnet\n+++ b/jsonnet/Sample.jsonnet\n@@ -1 +1 @@\n-BADFMT\n+fixed\n";

    #[test]
    fn jsonnetfmt_reports_diff_files() {
        let findings = parse_jsonnetfmt(DIRTY.as_bytes(), Some(1), &["jsonnet/Sample.jsonnet"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "jsonnet/Sample.jsonnet");
        let clean = parse_jsonnetfmt(b"", Some(0), &["jsonnet/Sample.jsonnet"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_jsonnetfmt(b"", Some(1), &["jsonnet/Sample.jsonnet"]).is_err());
        assert!(parse_jsonnetfmt(DIRTY.as_bytes(), Some(1), &["other.libsonnet"]).is_err());
        assert!(parse_jsonnetfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
