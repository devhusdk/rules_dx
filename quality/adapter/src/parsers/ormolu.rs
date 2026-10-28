use super::{as_text, code_name, finding, known_native, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "ormolu";

/// Exit 0 is clean and 100 prints the file name and a diff on stderr. Any other exit is a
/// failure that names `file:line:col` on stderr, and one that does not is a shape error.
pub fn parse_ormolu(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;
    if !out.trim().is_empty() {
        return Err(ParseError::shape(TOOL, "unexpected stdout"));
    }
    match code {
        Some(0) if err.trim().is_empty() => Ok(Vec::new()),
        Some(100) => unformatted_files(err, files),
        Some(0) => Err(ParseError::shape(TOOL, "exit 0 with output")),
        _ => failure(err, files, code),
    }
}

/// A diff header is a line holding only a checked path; hunk lines start with a marker.
fn unformatted_files(stderr: &str, files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings: Vec<FileFinding> = Vec::new();
    for line in stderr.lines() {
        if line.is_empty() || line.starts_with(['@', ' ', '+', '-']) {
            continue;
        }
        let file = known_native(TOOL, files, line.trim_end())?;
        if !findings.iter().any(|f| f.file == file) {
            findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
        }
    }
    if findings.is_empty() {
        return Err(ParseError::shape(TOOL, "exit 100 without a file header"));
    }
    Ok(findings)
}

/// A located failure: `path:line:col` or `path:(l1,c1)-(l2,c2)`, then an indented message.
fn failure(
    stderr: &str,
    files: &[&str],
    code: Option<i32>,
) -> Result<Vec<FileFinding>, ParseError> {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut findings = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with(' ') || line.trim().is_empty() {
            continue;
        }
        let Some((file, row, column)) = locate(line, files) else {
            continue;
        };
        let message = lines[index + 1..]
            .iter()
            .take_while(|next| next.starts_with(' '))
            .map(|next| next.trim())
            .collect::<Vec<_>>()
            .join(" ");
        findings.push(finding(
            TOOL,
            file,
            String::new(),
            if message.is_empty() {
                "ormolu failed".to_owned()
            } else {
                message
            },
            ToolSeverity::Error,
            TextPosition { line: row, column },
            ColumnEncoding::Scalars,
        ));
    }
    if findings.is_empty() {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} without a located error", code_name(code)),
        ));
    }
    Ok(findings)
}

fn locate<'a>(line: &str, files: &[&'a str]) -> Option<(&'a str, u64, u64)> {
    let file = files.iter().find(|file| {
        line.strip_prefix(**file)
            .is_some_and(|rest| rest.starts_with(':'))
    })?;
    let rest = line.strip_prefix(*file)?.strip_prefix(':')?;
    let (row, column) = match rest.strip_prefix('(') {
        Some(span) => {
            let (row, after) = span.split_once(',')?;
            (row, after.split(')').next()?)
        }
        None => {
            let mut parts = rest.split(':');
            let row = parts.next()?;
            (row, parts.next()?.split(['-', ' ']).next()?)
        }
    };
    let row: u64 = row.parse().ok()?;
    let column: u64 = column.parse().ok()?;
    Some((file, row.max(1), column.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/a.hs", "/s/b.hs"];
    const DIRTY: &str = "/s/a.hs\n@@ -1,4 +1,5 @@\n  module B where\n- main::IO()\n- main   =   putStrLn    \"héllo\"\n\n+ main :: IO ()\n+ main = putStrLn \"héllo\"\n+\n";
    const BROKEN: &str = "/s/b.hs:2:8\n  The GHC parser (in Haddock mode) failed:\n  [GHC-58481] parse error on input `='\n";

    #[test]
    fn ormolu_clean_is_empty() {
        assert!(parse_ormolu(b"", b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn ormolu_reports_an_unformatted_file_with_a_non_ascii_diff() {
        let findings = parse_ormolu(b"", DIRTY.as_bytes(), Some(100), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/a.hs");
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn ormolu_locates_a_parse_error() {
        let findings = parse_ormolu(b"", BROKEN.as_bytes(), Some(3), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/b.hs");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 8);
        assert!(findings[0].finding.message.contains("parse error"));
    }

    #[test]
    fn ormolu_locates_a_multi_line_span_at_its_start() {
        let spanned =
            "/s/b.hs:(2,8)-(3,4): error:\n  The GHC parser failed:\n  parse error on input `='\n";
        let findings = parse_ormolu(b"", spanned.as_bytes(), Some(3), &FILES).expect("ok");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/b.hs");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 8);
        assert!(findings[0].finding.message.contains("parse error"));
    }

    #[test]
    fn ormolu_rejects_a_malformed_span() {
        for broken in [
            "/s/b.hs:(2)-(3,4): error:\n  x\n",
            "/s/b.hs:(2,x): error:\n  x\n",
        ] {
            assert!(
                parse_ormolu(b"", broken.as_bytes(), Some(3), &FILES).is_err(),
                "{broken}"
            );
        }
    }

    #[test]
    fn ormolu_never_reports_a_failure_as_clean() {
        assert!(parse_ormolu(b"", b"", Some(100), &FILES).is_err());
        assert!(parse_ormolu(b"", b"", Some(3), &FILES).is_err());
        assert!(parse_ormolu(b"", b"", Some(102), &FILES).is_err());
        assert!(parse_ormolu(b"", b"", None, &FILES).is_err());
        assert!(parse_ormolu(b"", b"boom\n", Some(1), &FILES).is_err());
        assert!(parse_ormolu(b"", b"warning\n", Some(0), &FILES).is_err());
        assert!(parse_ormolu(b"x\n", b"", Some(0), &FILES).is_err());
        assert!(parse_ormolu(b"", b"/s/other.hs\n@@\n", Some(100), &FILES).is_err());
        assert!(parse_ormolu(b"", &[0xff], Some(100), &FILES).is_err());
    }
}
