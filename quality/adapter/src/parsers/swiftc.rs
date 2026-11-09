use super::gnu_diagnostics::{Dialect, Disposition, read};
use super::{FileFinding, ParseError};
use crate::ColumnEncoding;
use crate::ToolSeverity;

const DIALECT: Dialect = Dialect {
    tool: "swiftc",
    // swiftc columns count UTF-8 bytes of the line.
    encoding: ColumnEncoding::Utf8,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        "warning" => Some(Disposition::Report(ToolSeverity::Warning)),
        "note" | "remark" => Some(Disposition::Skip),
        _ => None,
    },
    bracket_rule: true,
};

/// `swiftc -typecheck` output. A nonzero exit that printed no diagnostic is an error.
pub fn parse_swiftc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    read(&DIALECT, output, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "swift/a.swift";

    #[test]
    fn swiftc_reports_errors_and_warnings_and_skips_notes() {
        let text = "swift/a.swift:4:13: error: cannot find 'y' in scope\n    let x = y\n            ^\nswift/a.swift:2:5: warning: variable 'z' was never used; consider replacing with '_' or removing it [#no-usage]\nswift/a.swift:2:5: note: remove 'z'\n";
        let found = parse_swiftc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.start.column, 13);
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[1].finding.rule_id, "#no-usage");
    }

    #[test]
    fn swiftc_error_in_a_foreign_file_fails_closed() {
        let text = "swift/b.swift:1:1: error: boom\n";
        assert!(parse_swiftc(text.as_bytes(), Some(1), &[FILE]).is_err());
    }

    #[test]
    fn swiftc_clean_and_silent_failure() {
        assert!(parse_swiftc(b"", Some(0), &[FILE]).expect("clean").is_empty());
        assert!(parse_swiftc(b"", Some(1), &[FILE]).is_err());
        assert!(parse_swiftc(b"", None, &[FILE]).is_err());
        assert!(parse_swiftc(b"error: no location\n", Some(1), &[FILE]).is_err());
    }
}
