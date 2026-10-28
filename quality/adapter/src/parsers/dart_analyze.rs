use serde::Deserialize;

use super::{check_output_size, code_name, finding, FileFinding, ParseError};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

const TOOL: &str = "dart";

/// Which slice of one `dart analyze` run a capability keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DartAnalyzeView {
    /// Lints, hints, static warnings and TODOs.
    Lint,
    /// Compile-time and syntax errors.
    Typecheck,
}

#[derive(Debug, Deserialize)]
struct Report {
    version: u64,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    code: String,
    severity: String,
    #[serde(rename = "type")]
    kind: String,
    location: Location,
    #[serde(rename = "problemMessage")]
    problem_message: String,
    #[serde(rename = "correctionMessage")]
    correction_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Location {
    file: String,
    range: Range,
}

#[derive(Debug, Deserialize)]
struct Range {
    start: Position,
}

#[derive(Debug, Deserialize)]
struct Position {
    line: u64,
    column: u64,
}

fn shape(detail: String) -> ParseError {
    ParseError::shape(TOOL, detail)
}

/// Whether a diagnostic type belongs to the lint view (`Some(true)`) or the typecheck view.
fn is_lint(kind: &str) -> Result<bool, ParseError> {
    match kind {
        "LINT" | "HINT" | "STATIC_WARNING" | "STATIC_TYPE_WARNING" | "TODO" => Ok(true),
        "COMPILE_TIME_ERROR" | "CHECKED_MODE_COMPILE_TIME_ERROR" | "SYNTACTIC_ERROR" => Ok(false),
        _ => Err(shape(format!("unknown diagnostic type: {kind}"))),
    }
}

/// Findings from `dart analyze --format=json <options> <files...>`.
///
/// `files` are the analyzed sources as the tool spells them (canonical paths) and `options_path`
/// is the staged `analysis_options.yaml`, which is listed so that a broken `include:` or an
/// unknown lint surfaces; a diagnostic on it is a configuration failure, not a finding. The exit
/// code must agree with the worst severity: 0 for info or nothing, 2 for warnings, 3 for errors.
/// The lint and typecheck views share one run and partition it by diagnostic type.
pub fn parse_dart_analyze(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    options_path: &str,
    view: DartAnalyzeView,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let report: Report = serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if report.version != 1 {
        return Err(shape(format!(
            "unsupported report version {}",
            report.version
        )));
    }
    let mut worst = 0;
    let mut findings = Vec::new();
    for diagnostic in &report.diagnostics {
        let (severity, rank) = match diagnostic.severity.as_str() {
            "ERROR" => (ToolSeverity::Error, 3),
            "WARNING" => (ToolSeverity::Warning, 2),
            "INFO" => (ToolSeverity::Info, 0),
            other => return Err(shape(format!("unknown severity: {other}"))),
        };
        worst = worst.max(rank);
        let lint = is_lint(&diagnostic.kind)?;
        let message = match diagnostic.correction_message.as_deref() {
            Some(correction) if !correction.is_empty() => {
                format!("{} {correction}", diagnostic.problem_message)
            }
            _ => diagnostic.problem_message.clone(),
        };
        let file = diagnostic.location.file.as_str();
        if file == options_path {
            return Err(shape(format!(
                "analysis_options.yaml is unusable: {}: {message}",
                diagnostic.code
            )));
        }
        let Some(known) = files.iter().copied().find(|known| *known == file) else {
            return Err(ParseError::UnknownFile {
                tool: TOOL,
                path: file.to_owned(),
            });
        };
        let start = &diagnostic.location.range.start;
        if start.line < 1 || start.column < 1 {
            return Err(shape(format!(
                "bad position {}:{} in {file}",
                start.line, start.column
            )));
        }
        if lint != (view == DartAnalyzeView::Lint) {
            continue;
        }
        findings.push(finding(
            TOOL,
            known,
            diagnostic.code.clone(),
            message,
            severity,
            TextPosition {
                line: start.line,
                column: start.column,
            },
            ColumnEncoding::Utf16,
        ));
    }
    if code != Some(worst) {
        return Err(shape(format!(
            "exit {} with worst severity exit {worst}",
            code_name(code)
        )));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: &str = "/s/analysis_options.yaml";
    const FILES: [&str; 1] = ["/s/lib/a.dart"];

    fn diag(file: &str, kind: &str, severity: &str) -> String {
        format!(
            r#"{{"code":"c","severity":"{severity}","type":"{kind}","location":{{"file":"{file}","range":{{"start":{{"offset":3,"line":2,"column":5}},"end":{{"offset":4,"line":2,"column":6}}}}}},"problemMessage":"bad.","correctionMessage":"Fix it."}}"#
        )
    }

    fn report(diagnostics: &[String]) -> Vec<u8> {
        format!(
            r#"{{"version":1,"diagnostics":[{}]}}"#,
            diagnostics.join(",")
        )
        .into_bytes()
    }

    fn run(
        diagnostics: &[String],
        code: i32,
        view: DartAnalyzeView,
    ) -> Result<Vec<FileFinding>, ParseError> {
        parse_dart_analyze(&report(diagnostics), b"", Some(code), &FILES, OPTIONS, view)
    }

    #[test]
    fn clean_run_has_no_findings() {
        assert!(run(&[], 0, DartAnalyzeView::Lint).unwrap().is_empty());
    }

    #[test]
    fn views_partition_the_same_output() {
        let all = [
            diag(FILES[0], "LINT", "INFO"),
            diag(FILES[0], "HINT", "WARNING"),
            diag(FILES[0], "COMPILE_TIME_ERROR", "ERROR"),
        ];
        let lint = run(&all, 3, DartAnalyzeView::Lint).unwrap();
        let types = run(&all, 3, DartAnalyzeView::Typecheck).unwrap();
        assert_eq!(lint.len(), 2);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].finding.severity, ToolSeverity::Error);
        assert_eq!(types[0].finding.message, "bad. Fix it.");
        assert_eq!(types[0].finding.start, TextPosition { line: 2, column: 5 });
        assert_eq!(types[0].finding.encoding, ColumnEncoding::Utf16);
    }

    #[test]
    fn options_file_diagnostics_are_a_config_error() {
        let found = [diag(OPTIONS, "COMPILE_TIME_ERROR", "ERROR")];
        let err = run(&found, 3, DartAnalyzeView::Lint).unwrap_err();
        assert!(err
            .to_string()
            .contains("analysis_options.yaml is unusable"));
    }

    #[test]
    fn exit_must_match_worst_severity() {
        let warn = [diag(FILES[0], "HINT", "WARNING")];
        assert!(run(&warn, 0, DartAnalyzeView::Lint).is_err());
        assert!(run(&warn, 2, DartAnalyzeView::Lint).is_ok());
        assert!(run(&[], 64, DartAnalyzeView::Lint).is_err());
    }

    #[test]
    fn unknown_input_fails_closed() {
        let unknown_file = [diag("/s/lib/b.dart", "LINT", "INFO")];
        assert!(run(&unknown_file, 0, DartAnalyzeView::Lint).is_err());
        let unknown_type = [diag(FILES[0], "NEW", "INFO")];
        assert!(run(&unknown_type, 0, DartAnalyzeView::Lint).is_err());
        let unknown_severity = [diag(FILES[0], "LINT", "FATAL")];
        assert!(run(&unknown_severity, 0, DartAnalyzeView::Lint).is_err());
        assert!(parse_dart_analyze(
            b"nope",
            b"",
            Some(0),
            &FILES,
            OPTIONS,
            DartAnalyzeView::Lint
        )
        .is_err());
        let bad_version = br#"{"version":2,"diagnostics":[]}"#;
        assert!(parse_dart_analyze(
            bad_version,
            b"",
            Some(0),
            &FILES,
            OPTIONS,
            DartAnalyzeView::Lint
        )
        .is_err());
    }
}
