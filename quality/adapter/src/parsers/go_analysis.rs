use std::collections::BTreeMap;

use serde::Deserialize;

use super::{check_output_size, known, require_findings, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Analyzer {
    Diagnostics(Vec<Diagnostic>),
    Failure { error: String },
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    posn: String,
    message: String,
}

type Report = BTreeMap<String, BTreeMap<String, Analyzer>>;

/// Splits a unitchecker `file:line:col` position, keeping drive colons in the file.
fn split_posn(posn: &str) -> Option<(&str, u64, u64)> {
    let mut parts = posn.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    (line > 0 && column > 0 && !file.is_empty()).then_some((file, line, column))
}

/// Parses the `-json` output of a stock unitchecker driver (govet, staticcheck, errcheck).
///
/// The rule id is the analyzer name. An analyzer `{error}` entry is a parse error, never clean.
pub fn parse_go_analysis(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    parse_report(tool, stdout, code, files, ToolSeverity::Warning)
}

/// Parses the `dx_gotype` driver report, whose `syntax` and `typecheck` entries are errors.
pub fn parse_gotype(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    parse_report(tool, stdout, code, files, ToolSeverity::Error)
}

fn parse_report(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    severity: ToolSeverity,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stdout)?;
    let mut findings = Vec::new();
    let stream = serde_json::Deserializer::from_slice(stdout).into_iter::<Report>();
    for report in stream {
        let report = report.map_err(|err| ParseError::Json {
            tool,
            detail: err.to_string(),
        })?;
        for (package, analyzers) in report {
            for (analyzer, result) in analyzers {
                let diagnostics = match result {
                    Analyzer::Failure { error } => {
                        return Err(ParseError::shape(
                            tool,
                            format!("{package}: analyzer {analyzer} failed: {error}"),
                        ));
                    }
                    Analyzer::Diagnostics(diagnostics) => diagnostics,
                };
                for diagnostic in diagnostics {
                    let Some((path, line, column)) = split_posn(&diagnostic.posn) else {
                        return Err(ParseError::shape(
                            tool,
                            format!("malformed position: {}", diagnostic.posn),
                        ));
                    };
                    let file = known(tool, files, path)?;
                    findings.push(FileFinding {
                        file: file.to_owned(),
                        finding: Finding {
                            tool_id: tool.to_owned(),
                            rule_id: analyzer.clone(),
                            message: diagnostic.message,
                            severity,
                            start: TextPosition { line, column },
                            end: None,
                            suggestions: Vec::new(),
                            encoding: ColumnEncoding::Utf8,
                            related: Vec::new(),
                        },
                    });
                }
            }
        }
    }
    require_findings(tool, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str =
        r#"{"p":{"printf":[{"posn":"go/a.go:7:2","message":"bad verb"}],"unusedresult":[]}}"#;

    #[test]
    fn reports_analyzer_diagnostics() {
        let findings =
            parse_go_analysis("govet", DIRTY.as_bytes(), Some(0), &["go/a.go"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go/a.go");
        assert_eq!(findings[0].finding.rule_id, "printf");
        assert_eq!(findings[0].finding.tool_id, "govet");
        assert_eq!(findings[0].finding.start.line, 7);
    }

    #[test]
    fn clean_and_empty_are_clean_only_on_zero() {
        assert!(parse_go_analysis("govet", b"", Some(0), &[])
            .unwrap()
            .is_empty());
        assert!(parse_go_analysis("govet", br#"{"p":{}}"#, Some(0), &[])
            .unwrap()
            .is_empty());
        assert!(parse_go_analysis("govet", b"", Some(1), &[]).is_err());
        assert!(parse_go_analysis("govet", b"", None, &[]).is_err());
    }

    #[test]
    fn analyzer_error_is_never_clean() {
        let out = br#"{"p":{"printf":{"error":"typecheck failed"}}}"#;
        assert!(parse_go_analysis("govet", out, Some(0), &[]).is_err());
    }

    #[test]
    fn rejects_unknown_files_bad_positions_and_bad_json() {
        assert!(parse_go_analysis("govet", DIRTY.as_bytes(), Some(0), &["other.go"]).is_err());
        let bad = br#"{"p":{"x":[{"posn":"nope","message":"m"}]}}"#;
        assert!(parse_go_analysis("govet", bad, Some(0), &["nope"]).is_err());
        assert!(parse_go_analysis("govet", b"{", Some(0), &[]).is_err());
        assert!(parse_go_analysis("govet", &[0xff], Some(0), &[]).is_err());
    }

    #[test]
    fn reads_drive_rooted_paths() {
        let out = br#"{"p":{"errcheck":[{"posn":"C:\\tmp\\a.go:3:1","message":"m"}]}}"#;
        let findings =
            parse_go_analysis("errcheck", out, Some(0), &["C:\\tmp\\a.go"]).expect("parsed");
        assert_eq!(findings[0].finding.start.column, 1);
    }

    const TYPED: &str = r#"{"p":{"typecheck":[{"posn":"go/a.go:4:9","message":"undefined: x"}],"syntax":[{"posn":"go/a.go:9:1","message":"unexpected }"}]}}"#;

    #[test]
    fn gotype_reports_errors_with_stable_rule_ids() {
        let findings =
            parse_gotype("gotype", TYPED.as_bytes(), Some(0), &["go/a.go"]).expect("parsed");
        let ids: Vec<_> = findings
            .iter()
            .map(|f| f.finding.rule_id.as_str())
            .collect();
        assert_eq!(ids, ["syntax", "typecheck"]);
        assert!(findings
            .iter()
            .all(|f| matches!(f.finding.severity, ToolSeverity::Error)));
    }

    #[test]
    fn gotype_clean_and_context_failures() {
        assert!(parse_gotype("gotype", br#"{"p":{}}"#, Some(0), &[])
            .unwrap()
            .is_empty());
        assert!(parse_gotype("gotype", b"", Some(1), &[]).is_err());
        let err = br#"{"p":{"typecheck":{"error":"missing export data"}}}"#;
        assert!(parse_gotype("gotype", err, Some(0), &[]).is_err());
        let drive = br#"{"p":{"typecheck":[{"posn":"C:\\t\\a.go:3:1","message":"m"}]}}"#;
        assert_eq!(
            parse_gotype("gotype", drive, Some(0), &["C:\\t\\a.go"])
                .unwrap()
                .len(),
            1
        );
    }
}
