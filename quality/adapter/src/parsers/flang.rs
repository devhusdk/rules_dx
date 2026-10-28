use super::gnu_diagnostics::{read, Dialect, Disposition};
use super::{FileFinding, ParseError};
use crate::ColumnEncoding;
use crate::ToolSeverity;

const DIALECT: Dialect = Dialect {
    tool: "flang",
    // flang counts bytes within the line.
    encoding: ColumnEncoding::Utf8,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        "warning" | "portability" => Some(Disposition::Report(ToolSeverity::Warning)),
        "info" | "note" | "because" => Some(Disposition::Skip),
        _ => None,
    },
    bracket_rule: true,
};

/// `flang -fsyntax-only` output. A nonzero exit that printed no diagnostic is an error.
pub fn parse_flang(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    read(&DIALECT, output, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "fortran/a.f90";

    #[test]
    fn flang_reports_errors_and_skips_excerpts() {
        let text = "fortran/a.f90:3:7: error: No explicit type declared for 'y'\n    y = 1\n        ^\nerror: Semantic errors in fortran/a.f90\n";
        let found = parse_flang(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 7);
    }

    #[test]
    fn flang_reads_warning_flag_as_rule_and_drops_foreign_warnings() {
        let text = "fortran/a.f90:2:1: warning: Label should be in columns 1-5 [-Wlabel]\nfortran/b.f90:9:1: warning: dep noise\n    ^\nfortran/a.f90:2:1: because: context\n";
        let found = parse_flang(text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.rule_id, "-Wlabel");
        assert_eq!(found[0].finding.message, "Label should be in columns 1-5");
    }

    #[test]
    fn flang_fails_closed() {
        assert!(parse_flang(b"", Some(0), &[FILE])
            .expect("clean")
            .is_empty());
        assert!(parse_flang(b"", Some(1), &[FILE]).is_err());
        assert!(parse_flang(b"", None, &[FILE]).is_err());
        assert!(parse_flang(b"segfault\n", Some(139), &[FILE]).is_err());
        let dep = "fortran/b.f90:1:1: error: broken dependency\n";
        assert!(matches!(
            parse_flang(dep.as_bytes(), Some(1), &[FILE]),
            Err(ParseError::UnknownFile { .. })
        ));
        assert!(parse_flang(&[0xff], Some(1), &[FILE]).is_err());
    }
}
