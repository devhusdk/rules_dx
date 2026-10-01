use super::{listed_paths, FileFinding, ParseError};

pub fn parse_pkl(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("pkl", stdout, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pkl format --diff-name-only` names each file it would rewrite and exits 11.
    const DIRTY: &str = "pkl/Sample.pkl\n";

    #[test]
    fn pkl_reports_listed_files() {
        let findings = parse_pkl(DIRTY.as_bytes(), Some(11), &["pkl/Sample.pkl"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "pkl/Sample.pkl");
        let clean = parse_pkl(b"", Some(0), &["pkl/Sample.pkl"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_pkl(b"", Some(11), &["pkl/Sample.pkl"]).is_err());
        assert!(parse_pkl(DIRTY.as_bytes(), Some(11), &["other.pkl"]).is_err());
        assert!(parse_pkl(&[0xff], Some(0), &["x"]).is_err());
    }
}
