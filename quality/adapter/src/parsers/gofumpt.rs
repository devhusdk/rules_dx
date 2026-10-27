use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_gofumpt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("gofumpt", stdout, code, files, DiffExit::One)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "diff go/tests/fixtures/gofumpt/Sample.go.orig go/tests/fixtures/gofumpt/Sample.go\n--- go/tests/fixtures/gofumpt/Sample.go.orig\n+++ go/tests/fixtures/gofumpt/Sample.go\n@@ -1,3 +1,3 @@\n-func Greet( name string)string{return \"hello \"+name}\n+func Greet(name string) string {\n+\treturn \"hello \" + name\n+}\n";

    #[test]
    fn gofumpt_reports_diff_files_on_exit_one() {
        let findings = parse_gofumpt(
            DIRTY.as_bytes(),
            Some(1),
            &["go/tests/fixtures/gofumpt/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go/tests/fixtures/gofumpt/Sample.go");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean =
            parse_gofumpt(b"", Some(0), &["go/tests/fixtures/gofumpt/Sample.go"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_gofumpt(
            DIRTY.as_bytes(),
            Some(0),
            &["go/tests/fixtures/gofumpt/Sample.go"]
        )
        .is_err());
        assert!(parse_gofumpt(DIRTY.as_bytes(), Some(1), &["other.go"]).is_err());
        assert!(parse_gofumpt(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn gofumpt_syntax_error_is_never_clean() {
        let files = ["go/tests/fixtures/gofumpt/Sample.go"];
        assert!(parse_gofumpt(b"", Some(2), &files).is_err());
        assert!(parse_gofumpt(b"", Some(1), &files).is_err());
        assert!(parse_gofumpt(DIRTY.as_bytes(), Some(2), &files).is_err());
    }
}
