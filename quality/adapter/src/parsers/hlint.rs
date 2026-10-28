use serde::Deserialize;

use super::{check_output_size, code_name, known_native, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "hlint";

const PARSE_ERROR: &str = "Parse error";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hint {
    severity: String,
    hint: String,
    file: String,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
    from: Option<String>,
    to: Option<String>,
}

fn severity(value: &str) -> Result<ToolSeverity, ParseError> {
    match value {
        "Error" => Ok(ToolSeverity::Error),
        "Warning" => Ok(ToolSeverity::Warning),
        "Suggestion" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unknown severity: {value}"),
        )),
    }
}

fn message(hint: &Hint) -> String {
    if hint.hint.starts_with(PARSE_ERROR) {
        return hint.hint.clone();
    }
    match (hint.from.as_deref(), hint.to.as_deref()) {
        (Some(from), Some(to)) if !from.is_empty() => {
            format!("{}: found `{}`, perhaps `{}`", hint.hint, from, to)
        }
        (Some(from), None) if !from.is_empty() => {
            format!("{}: found `{}`, perhaps remove it", hint.hint, from)
        }
        _ => hint.hint.clone(),
    }
}

/// `hlint --json` prints one JSON array on stdout: `[]` and exit 0 when every file is clean, a
/// non-empty array and exit 1 otherwise. A parse error is a hint with severity `Error`. A missing
/// file or an undecodable file also exits 1 but prints no array, and a missing system library
/// exits 127 with a loader message on stderr; all of those are errors here and never clean.
pub fn parse_hlint(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    if String::from_utf8_lossy(stderr).contains("error while loading shared libraries") {
        return Err(ParseError::shape(
            TOOL,
            "the host lacks libgmp.so.10; install it or set --@rules_dx//config:tool_hlint"
                .to_owned(),
        ));
    }
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let hints: Vec<Hint> = serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if hints.is_empty() != (code == Some(0)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} hints", code_name(code), hints.len()),
        ));
    }
    let mut findings = Vec::with_capacity(hints.len());
    for hint in &hints {
        if hint.start_line == 0 || hint.start_column == 0 {
            return Err(ParseError::shape(TOOL, "zero start position".to_owned()));
        }
        let checked = known_native(TOOL, files, &hint.file)?;
        let end = (hint.end_line != 0 && hint.end_column != 0).then_some(TextPosition {
            line: hint.end_line,
            column: hint.end_column,
        });
        let rule_id = if hint.hint.starts_with(PARSE_ERROR) {
            PARSE_ERROR.to_owned()
        } else {
            hint.hint.clone()
        };
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id,
                message: message(hint),
                severity: severity(&hint.severity)?,
                start: TextPosition {
                    line: hint.start_line,
                    column: hint.start_column,
                },
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::ScalarsTabs { width: 8 },
                related: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 2] = ["/s/haskell/A.hs", "/s/haskell/B.hs"];

    const DIRTY: &str = r#"[{"module":["A"],"decl":["f"],"severity":"Warning","hint":"Evaluate","file":"/s/haskell/A.hs","startLine":2,"startColumn":5,"endLine":2,"endColumn":21,"from":"map (\\ x -> x) []","to":"[]","note":[],"refactorings":"[]"}
,{"module":["A"],"decl":["f"],"severity":"Warning","hint":"Use id","file":"/s/haskell/A.hs","startLine":2,"startColumn":10,"endLine":2,"endColumn":17,"from":"\\ x -> x","to":"id","note":[],"refactorings":"[]"}]"#;

    const SYNTAX: &str = r#"[{"module":[],"decl":[],"severity":"Error","hint":"Parse error: on input `='","file":"/s/haskell/B.hs","startLine":3,"startColumn":11,"endLine":3,"endColumn":12,"from":"  x\n","to":null,"note":[],"refactorings":"[]"}]"#;

    fn parse(stdout: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_hlint(stdout.as_bytes(), b"", Some(code), &FILES)
    }

    #[test]
    fn hlint_is_clean_on_an_empty_array_and_exit_zero() {
        assert!(parse("[]", 0).expect("parsed").is_empty());
    }

    #[test]
    fn hlint_locates_each_hint() {
        let findings = parse(DIRTY, 1).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[1].file, "/s/haskell/A.hs");
        assert_eq!(findings[1].finding.tool_id, "hlint");
        assert_eq!(findings[1].finding.rule_id, "Use id");
        assert_eq!(
            findings[1].finding.message,
            "Use id: found `\\ x -> x`, perhaps `id`"
        );
        assert_eq!(findings[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[1].finding.start, findings[1].finding.end),
            (
                TextPosition {
                    line: 2,
                    column: 10
                },
                Some(TextPosition {
                    line: 2,
                    column: 17
                })
            )
        );
        assert_eq!(
            findings[1].finding.encoding,
            ColumnEncoding::ScalarsTabs { width: 8 }
        );
    }

    #[test]
    fn hlint_reports_a_parse_error_as_a_finding() {
        let findings = parse(SYNTAX, 1).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/haskell/B.hs");
        assert_eq!(findings[0].finding.rule_id, "Parse error");
        assert_eq!(findings[0].finding.message, "Parse error: on input `='");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn hlint_maps_suggestions_to_info_and_rejects_unknown_severities() {
        let info = SYNTAX.replace("\"Error\"", "\"Suggestion\"");
        assert_eq!(
            parse(&info, 1).expect("parsed")[0].finding.severity,
            ToolSeverity::Info
        );
        assert!(parse(&SYNTAX.replace("\"Error\"", "\"Ignore\""), 1).is_err());
        assert!(parse(&SYNTAX.replace("\"Error\"", "\"Fatal\""), 1).is_err());
    }

    #[test]
    fn hlint_fails_closed() {
        // Exit and array disagree.
        assert!(parse("[]", 1).is_err());
        assert!(parse(SYNTAX, 0).is_err());
        // Unknown exits, including a usage error, a missing loader library and a signal.
        assert!(parse("[]", 2).is_err());
        assert!(parse(SYNTAX, 2).is_err());
        assert!(parse("", 127).is_err());
        assert!(parse_hlint(b"[]", b"", None, &FILES).is_err());
        // A missing file or an undecodable file exits 1 without a report.
        assert!(parse("", 1).is_err());
        assert!(parse("Couldn't find file: x.hs", 1).is_err());
        assert!(parse_hlint(&[0xff, 0xfe], b"", Some(1), &FILES).is_err());
        // A truncated report.
        assert!(parse(&SYNTAX[..60], 1).is_err());
        // A finding in a file that was not passed.
        assert!(parse_hlint(SYNTAX.as_bytes(), b"", Some(1), &["/s/other.hs"]).is_err());
        // A zero position.
        assert!(parse(&SYNTAX.replace("\"startLine\":3", "\"startLine\":0"), 1).is_err());
        assert!(parse(
            &SYNTAX.replace("\"startColumn\":11", "\"startColumn\":0"),
            1
        )
        .is_err());
    }

    #[test]
    fn hlint_names_the_missing_library_and_the_override() {
        let err = parse_hlint(
            b"",
            b"hlint: error while loading shared libraries: libgmp.so.10: cannot open",
            Some(127),
            &FILES,
        )
        .expect_err("loader failure");
        let text = err.to_string();
        assert!(text.contains("libgmp.so.10"), "{text}");
        assert!(text.contains("--@rules_dx//config:tool_hlint"), "{text}");
    }

    #[cfg(windows)]
    #[test]
    fn hlint_accepts_either_separator_on_windows() {
        let files = ["C:\\s\\haskell\\B.hs"];
        let report = SYNTAX.replace("/s/haskell/B.hs", "C:/s/haskell/B.hs");
        assert!(parse_hlint(report.as_bytes(), b"", Some(1), &files).is_ok());
    }
}
