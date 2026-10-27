use std::path::Path;

use super::{
    as_text, code_name, diff_paths, finding, known_relative, path_row_col, unformatted_at,
    FileFinding, ParseError,
};
use crate::ToolSeverity;

const TOOL: &str = "cue";

/// Exit 0 is clean. Exit 1 prints a diff for unformatted files or locates syntax errors on stderr.
pub fn parse_cue(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    let diff = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    match code {
        Some(0) if diff.trim().is_empty() => Ok(Vec::new()),
        Some(1) => {
            let mut findings = Vec::new();
            for path in diff_paths(diff) {
                let file = known_relative(TOOL, files, cwd, &path)?;
                findings.push(unformatted_at(TOOL, file, 1, 1));
            }
            findings.extend(syntax_errors(errors, files, cwd)?);
            if findings.is_empty() {
                return Err(ParseError::shape(
                    TOOL,
                    "exit 1 with no diff and no located errors",
                ));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {} with output", code_name(code)),
        )),
    }
}

fn syntax_errors(stderr: &str, files: &[&str], cwd: &Path) -> Result<Vec<FileFinding>, ParseError> {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
        if line.starts_with(char::is_whitespace) || line.trim().is_empty() {
            continue;
        }
        let message = line.trim_end().trim_end_matches(':').to_owned();
        let Some((path, row, column)) = lines[index..]
            .iter()
            .take_while(|next| next.starts_with(char::is_whitespace))
            .find_map(|next| {
                let (path, row, column) = path_row_col(next.trim())?;
                let path = path.strip_prefix("./").unwrap_or(path);
                (!path.is_empty()).then_some((path, row, column))
            })
        else {
            return Err(ParseError::shape(
                TOOL,
                format!("error without a location: {message}"),
            ));
        };
        let file = known_relative(TOOL, files, cwd, path)?;
        let (row, column) = within_file(file, row, column);
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message,
            ToolSeverity::Error,
            row,
            column,
        ));
    }
    Ok(findings)
}

fn within_file(file: &str, row: u64, column: u64) -> (u64, u64) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return (row, column);
    };
    let rows: Vec<&str> = text.lines().collect();
    let last = rows.len().max(1) as u64;
    let row = row.clamp(1, last);
    let width = rows.get(row as usize - 1).map_or(0, |line| line.len()) as u64;
    (row, column.clamp(1, width + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "diff other/o.cue.orig other/o.cue\n--- other/o.cue.orig\n+++ other/o.cue\n@@ -1,3 +1,3 @@\n package other\n \n-x:   1\n+x: 1\n\n";
    const BROKEN: &str = "expected '}', found 'EOF':\n    ./pkg/bad.cue:2:8\nexpected operand, found 'EOF':\n    ./pkg/c.cue:2:8\n";

    fn scratch() -> &'static Path {
        Path::new("/tmp/dx-scratch")
    }

    fn files() -> [&'static str; 3] {
        [
            "/tmp/dx-scratch/other/o.cue",
            "/tmp/dx-scratch/pkg/bad.cue",
            "/tmp/dx-scratch/pkg/c.cue",
        ]
    }

    #[test]
    fn cue_names_unformatted_files_from_its_working_directory() {
        let findings =
            parse_cue(DIRTY.as_bytes(), b"", Some(1), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/other/o.cue");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_cue(b"", b"", Some(0), &files(), scratch()).expect("parsed");
        assert!(clean.is_empty());
    }

    #[test]
    fn cue_locates_syntax_errors_in_every_file() {
        let findings =
            parse_cue(b"", BROKEN.as_bytes(), Some(1), &files(), scratch()).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/tmp/dx-scratch/pkg/bad.cue");
        assert_eq!(findings[0].finding.message, "expected '}', found 'EOF'");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 8);
        assert_eq!(findings[1].file, "/tmp/dx-scratch/pkg/c.cue");
        assert_eq!(findings[1].finding.message, "expected operand, found 'EOF'");
    }

    #[test]
    fn cue_never_reports_a_process_failure_as_clean() {
        assert!(parse_cue(b"", b"", Some(1), &files(), scratch()).is_err());
        assert!(parse_cue(b"", b"", Some(2), &files(), scratch()).is_err());
        assert!(parse_cue(b"", b"", None, &files(), scratch()).is_err());
        assert!(parse_cue(DIRTY.as_bytes(), b"", Some(0), &files(), scratch()).is_err());
        let missing = "stat /tmp/dx-scratch/gone.cue: no such file or directory\n";
        assert!(parse_cue(b"", missing.as_bytes(), Some(1), &files(), scratch()).is_err());
        assert!(parse_cue(&[0xff], b"", Some(0), &files(), scratch()).is_err());
    }

    #[test]
    fn cue_cannot_report_a_file_outside_the_run() {
        let err = parse_cue(
            b"--- elsewhere.cue.orig\n+++ elsewhere.cue\n@@ -1 +1 @@\n-a\n+b\n",
            b"",
            Some(1),
            &files(),
            scratch(),
        )
        .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
        let err = parse_cue(
            b"",
            b"expected '}', found 'EOF':\n    ./elsewhere.cue:2:8\n",
            Some(1),
            &files(),
            scratch(),
        )
        .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }
}
