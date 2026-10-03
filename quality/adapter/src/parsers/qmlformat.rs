use super::{listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_qmlformat(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("qmlformat", stdout, code, files, &[], Spelling::Exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qmlformat_reports_unformatted_paths() {
        let stdout = "qml/Main.qml\n";
        let findings =
            parse_qmlformat(stdout.as_bytes(), Some(1), &["qml/Main.qml"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "qml/Main.qml");
        let clean = parse_qmlformat(b"", Some(0), &["qml/Main.qml"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_qmlformat(b"", Some(1), &["qml/Main.qml"]).is_err());
        assert!(parse_qmlformat(stdout.as_bytes(), Some(1), &["other.qml"]).is_err());
        assert!(parse_qmlformat(&[0xff], Some(1), &["x"]).is_err());
    }

    /// `qmlformat` names each file from the directory it ran in, so the path carries a `./`.
    #[test]
    fn qmlformat_accepts_a_path_spelled_below_the_dot() {
        let findings =
            parse_qmlformat(b"./qml/Main.qml\n", Some(1), &["qml/Main.qml"]).expect("parsed");
        assert_eq!(findings[0].file, "qml/Main.qml");
    }
}
