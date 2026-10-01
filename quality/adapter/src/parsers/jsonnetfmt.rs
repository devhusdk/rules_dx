use super::{listed_paths, FileFinding, ParseError};

pub fn parse_jsonnetfmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("jsonnetfmt", stdout, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `jsonnetfmt --test` names each file it would rewrite and exits 2.
    const DIRTY: &str = "jsonnet/Sample.jsonnet\n";

    #[test]
    fn jsonnetfmt_reports_listed_files() {
        let findings = parse_jsonnetfmt(DIRTY.as_bytes(), Some(2), &["jsonnet/Sample.jsonnet"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "jsonnet/Sample.jsonnet");
        let clean = parse_jsonnetfmt(b"", Some(0), &["jsonnet/Sample.jsonnet"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_jsonnetfmt(b"", Some(2), &["jsonnet/Sample.jsonnet"]).is_err());
        assert!(parse_jsonnetfmt(DIRTY.as_bytes(), Some(2), &["other.libsonnet"]).is_err());
        assert!(parse_jsonnetfmt(&[0xff], Some(0), &["x"]).is_err());
    }
}
