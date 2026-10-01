use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_clang_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("clang_format", stdout, code, files, DiffExit::Unpinned)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "--- a/cc/tests/fixtures/clang_format/Sample.c\n+++ b/cc/tests/fixtures/clang_format/Sample.c\n@@ -1,3 +1,3 @@\n-int greet( const char*name){return 0;}\n+int greet(const char *name) {\n+  return 0;\n+}\n";

    #[test]
    fn clang_format_reports_diff_files() {
        let findings = parse_clang_format(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/clang_format/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_format/Sample.c");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_clang_format(b"", Some(0), &["cc/tests/fixtures/clang_format/Sample.c"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_clang_format(b"", Some(1), &["cc/tests/fixtures/clang_format/Sample.c"]).is_err()
        );
        assert!(parse_clang_format(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_clang_format(&[0xff], Some(1), &["x"]).is_err());
    }
}
