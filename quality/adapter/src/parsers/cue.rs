use crate::{ColumnEncoding, TextPosition};
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
                findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
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
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    Ok(findings)
}

/// `cue vet -c=false -E`. Exit 0 with no output is clean. Exit 1 prints one block per error:
/// a message line, then indented `path:row:col` positions. Each block becomes one finding at its
/// first position inside `owned`, else at its first position inside `known` (a dependency, which
/// the caller drops or reports). Any other outcome is an error, never clean.
pub fn parse_cue_vet(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    owned: &[&str],
    known: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let errors = as_text(TOOL, stderr)?;
    match code {
        Some(0) if out.trim().is_empty() && errors.trim().is_empty() => Ok(Vec::new()),
        Some(1) if out.trim().is_empty() => {
            let findings = vet_errors(errors, owned, known, cwd)?;
            if findings.is_empty() {
                return Err(ParseError::shape(TOOL, "exit 1 with no located errors"));
            }
            Ok(findings)
        }
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected vet exit {} with output", code_name(code)),
        )),
    }
}

fn vet_errors(
    stderr: &str,
    owned: &[&str],
    known: &[&str],
    cwd: &Path,
) -> Result<Vec<FileFinding>, ParseError> {
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
        let mut located: Vec<(&str, u64, u64)> = Vec::new();
        while let Some(next) = lines
            .get(index)
            .filter(|n| n.starts_with(char::is_whitespace))
        {
            index += 1;
            if let Some((path, row, column)) = path_row_col(next.trim()) {
                let path = path.strip_prefix("./").unwrap_or(path);
                if let Ok(file) = known_relative(TOOL, known, cwd, path) {
                    located.push((file, row, column));
                }
            }
        }
        let pick = located
            .iter()
            .find(|(file, _, _)| owned.contains(file))
            .or_else(|| located.first());
        let Some(&(file, row, column)) = pick else {
            return Err(ParseError::shape(
                TOOL,
                format!("error without a known location: {message}"),
            ));
        };
        let (row, column) = within_file(file, row, column);
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            message,
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
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
    const VET: &str = "greeting.name: conflicting values 1 and string (mismatched types int and string):\n    ./lib/g.cue:4:11\n    ./main.cue:5:11\n    ./main.cue:6:8\n";

    fn vet_files() -> [&'static str; 2] {
        ["/tmp/dx-scratch/lib/g.cue", "/tmp/dx-scratch/main.cue"]
    }

    #[test]
    fn vet_is_clean_only_on_silent_exit_zero() {
        let owned = ["/tmp/dx-scratch/main.cue"];
        let ok = parse_cue_vet(b"", b"", Some(0), &owned, &vet_files(), scratch());
        assert!(ok.expect("clean").is_empty());
        assert!(parse_cue_vet(b"", b"x\n", Some(0), &owned, &vet_files(), scratch()).is_err());
        assert!(parse_cue_vet(b"", b"", Some(1), &owned, &vet_files(), scratch()).is_err());
        assert!(parse_cue_vet(
            b"",
            VET.as_bytes(),
            Some(2),
            &owned,
            &vet_files(),
            scratch()
        )
        .is_err());
        assert!(parse_cue_vet(b"", VET.as_bytes(), None, &owned, &vet_files(), scratch()).is_err());
        assert!(parse_cue_vet(b"", b"boom\n", Some(1), &owned, &vet_files(), scratch()).is_err());
    }

    #[test]
    fn vet_reports_a_conflict_at_the_first_owned_position() {
        let owned = ["/tmp/dx-scratch/main.cue"];
        let found = parse_cue_vet(
            b"",
            VET.as_bytes(),
            Some(1),
            &owned,
            &vet_files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/tmp/dx-scratch/main.cue");
        assert_eq!(found[0].finding.start.line, 5);
        assert!(found[0]
            .finding
            .message
            .starts_with("greeting.name: conflicting values"));
    }

    #[test]
    fn vet_keeps_a_dependency_only_error_on_the_dependency() {
        let owned = ["/tmp/dx-scratch/main.cue"];
        let text = "import failed: expected '}', found 'EOF':\n    ./lib/g.cue:2:54\n";
        let found = parse_cue_vet(
            b"",
            text.as_bytes(),
            Some(1),
            &owned,
            &vet_files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(found[0].file, "/tmp/dx-scratch/lib/g.cue");
    }

    #[test]
    fn vet_reports_unresolved_imports_and_syntax_errors() {
        let owned = ["/tmp/dx-scratch/main.cue"];
        let text = "import failed: cannot find package \"example.com/o/x\": cannot find module:\n    ./main.cue:2:8\nexpected '}', found 'EOF':\n    ./main.cue:3:1\n";
        let found = parse_cue_vet(
            b"",
            text.as_bytes(),
            Some(1),
            &owned,
            &vet_files(),
            scratch(),
        )
        .expect("parsed");
        assert_eq!(found.len(), 2);
        let stray = "oops:\n    ./other.cue:1:1\n";
        assert!(parse_cue_vet(
            b"",
            stray.as_bytes(),
            Some(1),
            &owned,
            &vet_files(),
            scratch()
        )
        .is_err());
    }
}
