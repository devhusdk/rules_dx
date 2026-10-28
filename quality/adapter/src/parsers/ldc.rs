use super::gnu_diagnostics::{read, Dialect, Disposition};
use super::{FileFinding, ParseError};
use crate::ColumnEncoding;
use crate::ToolSeverity;

const DIALECT: Dialect = Dialect {
    tool: "ldc",
    // `-verror-style=gnu` columns count characters of the line.
    encoding: ColumnEncoding::Scalars,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        "warning" | "deprecation" => Some(Disposition::Report(ToolSeverity::Warning)),
        _ => None,
    },
    bracket_rule: false,
};

/// `ldc2 -o- -verror-style=gnu` output. A nonzero exit that printed no diagnostic is an error.
pub fn parse_ldc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    read(&DIALECT, output, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "d/a.d";

    #[test]
    fn ldc_reports_errors_warnings_and_skips_supplemental_lines() {
        let text = "d/a.d:4:9: Error: undefined identifier `y`\n       auto x = y;\n                ^\nd/a.d:2:1: Deprecation: `foo` is deprecated\nd/a.d:2:1:        `foo` is declared here\nd/a.d:7:3: Warning: statement is not reachable\n";
        let found = parse_ldc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        let severities: Vec<_> = found.iter().map(|f| f.finding.severity).collect();
        assert_eq!(
            severities,
            vec![
                ToolSeverity::Error,
                ToolSeverity::Warning,
                ToolSeverity::Warning
            ]
        );
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(found[0].finding.message, "undefined identifier `y`");
    }

    #[test]
    fn ldc_accepts_a_missing_column_and_non_ascii_text() {
        let text = "d/a.d:5: Error: unterminated «string»\n";
        let found = parse_ldc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found[0].finding.start.line, 5);
        assert_eq!(found[0].finding.start.column, 1);
    }

    #[test]
    fn ldc_unlocated_error_with_warning_fails_closed() {
        let text = "Error: no location\nd/a.d:1:1: Warning: w\n";
        assert!(parse_ldc(text.as_bytes(), Some(1), &["d/a.d"]).is_err());
    }

    #[test]
    fn ldc_fails_closed() {
        assert!(parse_ldc(b"", Some(0), &[FILE]).expect("clean").is_empty());
        assert!(parse_ldc(b"", Some(1), &[FILE]).is_err());
        assert!(parse_ldc(b"", None, &[FILE]).is_err());
        assert!(parse_ldc(b"Error: no location\n", Some(1), &[FILE]).is_err());
        let dep = "d/b.d:1:1: Error: broken dependency\n";
        assert!(matches!(
            parse_ldc(dep.as_bytes(), Some(1), &[FILE]),
            Err(ParseError::UnknownFile { .. })
        ));
        let foreign_warning = "d/b.d:1:1: Warning: noise\n";
        assert!(parse_ldc(foreign_warning.as_bytes(), Some(0), &[FILE])
            .expect("clean")
            .is_empty());
    }
}
