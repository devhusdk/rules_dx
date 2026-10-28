use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

use super::{
    as_text, code_name, columned, finding, listed_paths, prefixed, FileFinding, ParseError,
    Spelling,
};

const TOOL: &str = "julia_format";

/// Findings from `format.jl --check`: listed files are unformatted, `path:line:col: error:` lines are syntax errors.
pub fn parse_julia_format(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let errors = syntax_errors(stderr, files)?;
    if errors.is_empty() {
        return listed_paths(TOOL, stdout, code, files, &[], Spelling::Exact);
    }
    if code != Some(1) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with syntax errors (want 1)", code_name(code)),
        ));
    }
    let mut findings = listed_paths(TOOL, stdout, Some(0), files, &[], Spelling::Exact)?;
    findings.extend(errors);
    Ok(findings)
}

fn syntax_errors(stderr: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((file, rest)) = prefixed(line, files) else {
            continue;
        };
        let Some((row, col, tail)) = columned(TOOL, line, rest).ok() else {
            continue;
        };
        if let Some(message) = tail.strip_prefix("error:") {
            out.push(finding(
                TOOL,
                file,
                "syntax".to_owned(),
                message.trim().to_owned(),
                ToolSeverity::Error,
                TextPosition {
                    line: row,
                    column: col,
                },
                ColumnEncoding::Scalars,
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "src/dirty.jl\n";
    const BROKEN: &str = "src/bad.jl:1:11: error: premature end of input\n";

    #[test]
    fn julia_format_reports_listed_files() {
        let files = ["src/dirty.jl", "src/clean.jl"];
        let findings = parse_julia_format(DIRTY.as_bytes(), b"", Some(1), &files).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/dirty.jl");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert!(parse_julia_format(b"", b"", Some(0), &files)
            .expect("parsed")
            .is_empty());
        assert!(parse_julia_format(b"", b"", Some(1), &files).is_err());
        assert!(parse_julia_format(b"other.jl\n", b"", Some(1), &files).is_err());
        assert!(parse_julia_format(&[0xff], b"", Some(0), &files).is_err());
    }

    #[test]
    fn julia_format_reports_syntax_errors_at_their_location() {
        let files = ["src/bad.jl", "src/dirty.jl"];
        let findings = parse_julia_format(DIRTY.as_bytes(), BROKEN.as_bytes(), Some(1), &files)
            .expect("parsed");
        assert_eq!(findings.len(), 2);
        let error = &findings[1];
        assert_eq!(error.file, "src/bad.jl");
        assert_eq!(error.finding.severity, ToolSeverity::Error);
        assert_eq!(error.finding.rule_id, "syntax");
        assert_eq!(error.finding.message, "premature end of input");
        assert!(parse_julia_format(b"", BROKEN.as_bytes(), Some(0), &files).is_err());
        assert!(parse_julia_format(b"", BROKEN.as_bytes(), Some(1), &["src/other.jl"]).is_err());
        assert!(parse_julia_format(b"", &[0xff], Some(1), &files).is_err());
    }

    #[test]
    fn julia_format_picks_the_longest_checked_file() {
        let findings = parse_julia_format(
            b"",
            b"/s/a.b:1:2: error: bad\n",
            Some(1),
            &["/s/a", "/s/a.b"],
        )
        .expect("parsed");
        assert_eq!(findings[0].file, "/s/a.b");
    }
}
