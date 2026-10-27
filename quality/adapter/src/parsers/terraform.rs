use crate::{ColumnEncoding, TextPosition};
use std::path::Path;

use super::{
    as_text, code_name, finding, known_relative, listed_paths, FileFinding, ParseError, Spelling,
};
use crate::ToolSeverity;

const TOOL: &str = "terraform";

/// Exit 0 is clean, 3 lists unformatted files, and 2 reports located syntax errors.
pub fn parse_terraform(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    match code {
        Some(0) | Some(3) => listed_paths(TOOL, stdout, code, files, &[], Spelling::Below(cwd)),
        Some(2) => syntax_errors(stderr, files, cwd),
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unexpected exit {}", code_name(code)),
        }),
    }
}

fn syntax_errors(
    stderr: &[u8],
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(summary) = line.strip_prefix("Error: ") else {
            continue;
        };
        let Some((path, number)) = lines[index + 1..]
            .iter()
            .take_while(|next| !next.starts_with("Error: "))
            .find_map(|next| located(next))
        else {
            continue;
        };
        let file = known_relative(TOOL, files, cwd, path)?;
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            summary.trim().to_owned(),
            ToolSeverity::Error,
            TextPosition {
                line: number,
                column: 1,
            },
            ColumnEncoding::Scalars,
        ));
    }
    if findings.is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "exit 2 with no located errors".to_owned(),
        });
    }
    Ok(findings)
}

/// The path and line from a `  on <path> line <n>[, in <block>]:` header.
fn located(line: &str) -> Option<(&str, u64)> {
    let rest = line.strip_prefix("  on ")?;
    rest.match_indices(" line ").find_map(|(at, marker)| {
        let tail = &rest[at + marker.len()..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        let after = &tail[digits.len()..];
        if !(after.is_empty() || after.starts_with(',') || after.starts_with(':')) {
            return None;
        }
        Some((&rest[..at], digits.parse().ok()?))
    })
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
            parse_terraform(DIRTY.as_bytes(), b"", Some(3), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
        assert_eq!(findings[1].file, "/tmp/dx-scratch/sub/Other.tf");
        let clean = parse_terraform(b"", b"", Some(0), &files(), scratch()).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_terraform(b"", b"", Some(3), &files(), scratch()).is_err());
        assert!(parse_terraform(&[0xff], b"", Some(0), &["x"], scratch()).is_err());
    }

    /// A file the run was never given is still rejected, relative or absolute.
    #[test]
    fn terraform_cannot_report_a_file_outside_the_run() {
        let err = parse_terraform(b"sub/elsewhere.tf\n", b"", Some(3), &files(), scratch())
            .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_terraform(b"/elsewhere/main.tf\n", b"", Some(3), &files(), scratch())
            .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_terraform(
            b"../scratch/sub/main.tf\n",
            b"",
            Some(3),
            &files(),
            scratch(),
        )
        .expect_err("climbing out of the run fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }

    /// A checked file already spelled the way the run spelled it matches without the cwd.
    #[test]
    fn terraform_accepts_the_path_the_run_handed_it() {
        let findings = parse_terraform(
            b"/tmp/dx-scratch/sub/main.tf\n",
            b"",
            Some(3),
            &files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
    }

    /// `terraform fmt -check -no-color` stderr for a syntax error, verbatim from 1.16.1.
    const BROKEN: &str = "\nError: Invalid expression\n\n  on sub/main.tf line 2, in resource \"a\" \"b\":\n   2:   x = \n   3: }\n\nExpected the start of an expression, but found an invalid expression token.\n\n\nError: Unclosed configuration block\n\n  on sub/Other.tf line 1, in a:\n   1: a {\n\nThere is no closing brace for this block before the end of the file.\n";

    #[test]
    fn terraform_syntax_errors_become_located_errors() {
        let findings =
            parse_terraform(b"", BROKEN.as_bytes(), Some(2), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
        assert_eq!(findings[0].finding.message, "Invalid expression");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[1].file, "/tmp/dx-scratch/sub/Other.tf");
        assert_eq!(findings[1].finding.start.line, 1);
    }

    #[test]
    fn terraform_block_labels_naming_a_line_do_not_move_the_location() {
        let stderr = b"Error: Invalid expression\n\n  on sub/main.tf line 2, in resource \"a\" \"my line 5\":\n   2:   a = \n\n";
        let findings = parse_terraform(b"", stderr, Some(2), &files(), scratch()).expect("parsed");
        assert_eq!(findings[0].file, "/tmp/dx-scratch/sub/main.tf");
        assert_eq!(findings[0].finding.start.line, 2);
    }

    #[test]
    fn terraform_exit_two_without_a_located_error_is_a_shape_failure() {
        assert!(parse_terraform(b"", b"", Some(2), &files(), scratch()).is_err());
        let unlocated = b"\nError: No file or directory at x.tf\n\n";
        assert!(parse_terraform(b"", unlocated, Some(2), &files(), scratch()).is_err());
    }

    #[test]
    fn terraform_exit_codes_are_pinned() {
        for code in [Some(1), Some(4), None] {
            assert!(parse_terraform(DIRTY.as_bytes(), b"", code, &files(), scratch()).is_err());
        }
    }
}
