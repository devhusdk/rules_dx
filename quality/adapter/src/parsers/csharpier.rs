use super::{listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_csharpier(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("csharpier", stdout, code, files, &[], Spelling::Exact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csharpier_reports_unformatted_paths() {
        let stdout = "csharp/tests/fixtures/csharpier/Sample.cs\n";
        let findings = parse_csharpier(
            stdout.as_bytes(),
            Some(1),
            &["csharp/tests/fixtures/csharpier/Sample.cs"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].file,
            "csharp/tests/fixtures/csharpier/Sample.cs"
        );
        let clean = parse_csharpier(b"", Some(0), &["csharp/tests/fixtures/csharpier/Sample.cs"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_csharpier(b"", Some(1), &["csharp/tests/fixtures/csharpier/Sample.cs"]).is_err()
        );
        assert!(parse_csharpier(stdout.as_bytes(), Some(1), &["other.cs"]).is_err());
        assert!(parse_csharpier(&[0xff], Some(1), &["x"]).is_err());
    }

    /// `csharpier` names each file from the directory it ran in, so the path carries a `./`.
    #[test]
    fn csharpier_accepts_a_path_spelled_below_the_dot() {
        let findings = parse_csharpier(
            b"./csharpier/Sample.cs\n",
            Some(1),
            &["csharpier/Sample.cs"],
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "csharpier/Sample.cs");
    }
}
