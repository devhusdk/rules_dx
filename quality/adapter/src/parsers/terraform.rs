use std::path::Path;

use super::{as_text, code_name, known_relative, FileFinding, Finding, ParseError, ToolSeverity};

pub fn parse_terraform(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "terraform";
    let text = as_text(TOOL, stdout)?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !paths.contains(&trimmed.to_owned()) {
            paths.push(trimmed.to_owned());
        }
    }
    if paths.is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no paths", code_name(code)),
        });
    }
    let mut findings = Vec::with_capacity(paths.len());
    for path in paths {
        let checked = known_relative(TOOL, files, cwd, &path)?;
        let (start, end) = super::point(1, 1);
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: String::new(),
                message: "file is not formatted".to_owned(),
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
            },
        });
    }
    Ok(findings)
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
