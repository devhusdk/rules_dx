use std::path::Path;

use super::{code_name, listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_scalafmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    match code {
        Some(0) | Some(1) => {
            listed_paths("scalafmt", stdout, code, files, &[], Spelling::Below(cwd)).and_then(
                |findings| {
                    if code == Some(0) && !findings.is_empty() {
                        Err(ParseError::Shape {
                            tool: "scalafmt",
                            detail: "exit 0 with listed files".to_owned(),
                        })
                    } else {
                        Ok(findings)
                    }
                },
            )
        }
        other => Err(ParseError::Shape {
            tool: "scalafmt",
            detail: format!("exit {} (a parse, file or config error)", code_name(other)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> &'static Path {
        Path::new("/tmp/dx-scratch")
    }

    fn files() -> [&'static str; 2] {
        [
            "/tmp/dx-scratch/app/Dirty.scala",
            "/tmp/dx-scratch/app/Other.scala",
        ]
    }

    #[test]
    fn scalafmt_lists_dirty_files_below_its_working_directory() {
        let findings = parse_scalafmt(
            b"app/Dirty.scala\napp/Other.scala\n",
            Some(1),
            &files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/app/Dirty.scala");
        assert_eq!(findings[1].file, "/tmp/dx-scratch/app/Other.scala");
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn scalafmt_clean_run_lists_nothing() {
        let clean = parse_scalafmt(b"", Some(0), &files(), scratch()).expect("clean");
        assert!(clean.is_empty());
    }

    #[test]
    fn scalafmt_parse_error_exit_is_never_clean() {
        let err = parse_scalafmt(b"", Some(2), &files(), scratch()).expect_err("parse error");
        assert!(err.to_string().contains("exit 2"));
        assert!(parse_scalafmt(b"app/Dirty.scala\n", Some(3), &files(), scratch()).is_err());
    }

    #[test]
    fn scalafmt_config_and_missing_file_exits_are_errors() {
        assert!(parse_scalafmt(b"", Some(16), &files(), scratch()).is_err());
        assert!(parse_scalafmt(b"", Some(8), &files(), scratch()).is_err());
        assert!(parse_scalafmt(b"", None, &files(), scratch()).is_err());
    }

    #[test]
    fn scalafmt_rejects_inconsistent_output() {
        assert!(parse_scalafmt(b"", Some(1), &files(), scratch()).is_err());
        assert!(parse_scalafmt(b"app/Dirty.scala\n", Some(0), &files(), scratch()).is_err());
        assert!(parse_scalafmt(b"app/Unknown.scala\n", Some(1), &files(), scratch()).is_err());
        assert!(parse_scalafmt(&[0xff], Some(1), &files(), scratch()).is_err());
    }
}
