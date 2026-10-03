use std::path::Path;

use super::{listed_paths, FileFinding, ParseError, Spelling};

pub fn parse_terraform(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    listed_paths("terraform", stdout, code, files, &[], Spelling::Below(cwd))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `terraform fmt -check` output, verbatim from terraform 1.16.1, which names each
    /// file by its path from the working directory rather than the one it was handed.
    const DIRTY: &str = "sub/main.tf\nsub/Other.tf\n";

    fn scratch() -> &'static Path {
        Path::new("/tmp/dx-scratch")
    }

    fn files() -> [&'static str; 2] {
        [
            "/tmp/dx-scratch/sub/main.tf",
            "/tmp/dx-scratch/sub/Other.tf",
        ]
    }

    #[test]
    fn terraform_reports_paths_relative_to_its_working_directory() {
        let findings =
            parse_terraform(DIRTY.as_bytes(), Some(3), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
        assert_eq!(findings[1].file, "/tmp/dx-scratch/sub/Other.tf");
        let clean = parse_terraform(b"", Some(0), &files(), scratch()).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_terraform(b"", Some(3), &files(), scratch()).is_err());
        assert!(parse_terraform(&[0xff], Some(0), &["x"], scratch()).is_err());
    }

    /// A file the run was never given is still rejected, relative or absolute.
    #[test]
    fn terraform_cannot_report_a_file_outside_the_run() {
        let err = parse_terraform(b"sub/elsewhere.tf\n", Some(3), &files(), scratch())
            .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_terraform(b"/elsewhere/main.tf\n", Some(3), &files(), scratch())
            .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_terraform(b"../scratch/sub/main.tf\n", Some(3), &files(), scratch())
            .expect_err("climbing out of the run fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }

    /// A checked file already spelled the way the run spelled it matches without the cwd.
    #[test]
    fn terraform_accepts_the_path_the_run_handed_it() {
        let findings = parse_terraform(
            b"/tmp/dx-scratch/sub/main.tf\n",
            Some(3),
            &files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
    }
}
