use super::{diff_format, DiffExit, FileFinding, ParseError};

pub fn parse_scalafmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    diff_format("scalafmt", stdout, code, files, DiffExit::Unpinned)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "--- a/scala/tests/fixtures/scalafmt/Sample.scala\n+++ b/scala/tests/fixtures/scalafmt/Sample.scala\n@@ -1,3 +1,3 @@\n-object Sample {  def greet = 1 }\n+object Sample {\n+  def greet = 1\n+}\n";

    #[test]
    fn scalafmt_reports_diff_files() {
        let findings = parse_scalafmt(
            DIRTY.as_bytes(),
            Some(1),
            &["scala/tests/fixtures/scalafmt/Sample.scala"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].file,
            "scala/tests/fixtures/scalafmt/Sample.scala"
        );
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_scalafmt(
            b"",
            Some(0),
            &["scala/tests/fixtures/scalafmt/Sample.scala"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_scalafmt(
            b"",
            Some(1),
            &["scala/tests/fixtures/scalafmt/Sample.scala"]
        )
        .is_err());
        assert!(parse_scalafmt(DIRTY.as_bytes(), Some(1), &["other.scala"]).is_err());
        assert!(parse_scalafmt(&[0xff], Some(1), &["x"]).is_err());
    }
}
