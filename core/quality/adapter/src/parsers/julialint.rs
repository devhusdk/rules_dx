use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{FileFinding, ParseError, as_text, code_name, finding};

const TOOL: &str = "julialint";

/// Findings from the JSON-lines report the pinned `lint.jl` driver writes.
///
/// Each line is `{"v":1,"file":..,"line":..,"col":..,"severity":..,"rule":..,"message":..}`.
/// The rule is `julialint/<rule>`. The driver only reports the files it was given, so a
/// line for any other file, a bad line, an unknown version or a nonzero exit fails the run.
pub fn parse_julialint(
    report: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} (want 0)", code_name(code)),
        ));
    }
    let text = as_text(TOOL, report)?;
    let mut findings: Vec<FileFinding> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = index + 1;
        let entry: serde_json::Value = serde_json::from_str(line).map_err(|err| {
            ParseError::shape(TOOL, format!("report line {at} is not JSON: {err}"))
        })?;
        if entry.get("v").and_then(serde_json::Value::as_u64) != Some(1) {
            return Err(ParseError::shape(
                TOOL,
                format!("report line {at} has an unknown version"),
            ));
        }
        let text_of = |key: &str| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| ParseError::shape(TOOL, format!("report line {at} has no {key}")))
        };
        let number_of = |key: &str| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .filter(|value| *value >= 1)
                .ok_or_else(|| ParseError::shape(TOOL, format!("report line {at} has no {key}")))
        };
        let file = text_of("file")?;
        if !files.contains(&file) {
            return Err(ParseError::shape(
                TOOL,
                format!("report line {at} names {file}, which was not checked"),
            ));
        }
        let severity = match text_of("severity")? {
            "error" => ToolSeverity::Error,
            "warning" => ToolSeverity::Warning,
            "information" | "hint" => ToolSeverity::Info,
            other => {
                return Err(ParseError::shape(
                    TOOL,
                    format!("report line {at} has unknown severity {other}"),
                ));
            }
        };
        findings.push(finding(
            TOOL,
            file,
            format!("julialint/{}", text_of("rule")?),
            text_of("message")?.to_owned(),
            severity,
            TextPosition {
                line: number_of("line")?,
                column: number_of("col")?,
            },
            ColumnEncoding::Scalars,
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = concat!(
        r#"{"v":1,"file":"a.jl","line":4,"col":12,"severity":"error","rule":"missing-reference","message":"Missing reference: x"}"#,
        "\n",
        r#"{"v":1,"file":"a.jl","line":3,"col":5,"severity":"hint","rule":"lint","message":"unused"}"#,
        "\n"
    );

    #[test]
    fn julialint_reports_rule_position_and_severity() {
        let found = parse_julialint(REPORT.as_bytes(), Some(0), &["a.jl"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.finding.rule_id.as_str(),
                    f.finding.start.line,
                    f.finding.start.column,
                    f.finding.severity,
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                ("julialint/missing-reference", 4, 12, ToolSeverity::Error),
                ("julialint/lint", 3, 5, ToolSeverity::Info),
            ]
        );
    }

    #[test]
    fn julialint_empty_reports_have_no_findings() {
        assert!(
            parse_julialint(b"", Some(0), &["a.jl"])
                .expect("empty")
                .is_empty()
        );
    }

    #[test]
    fn julialint_bad_reports_fail_the_run() {
        let files = ["a.jl"];
        assert!(parse_julialint(REPORT.as_bytes(), Some(2), &files).is_err());
        assert!(parse_julialint(REPORT.as_bytes(), None, &files).is_err());
        assert!(parse_julialint(b"not json", Some(0), &files).is_err());
        assert!(parse_julialint(REPORT.as_bytes(), Some(0), &["other.jl"]).is_err());
        assert!(parse_julialint(&[0xff], Some(0), &files).is_err());
        for line in [
            r#"{"v":2,"file":"a.jl","line":1,"col":1,"severity":"error","rule":"r","message":"m"}"#,
            r#"{"v":1,"line":1,"col":1,"severity":"error","rule":"r","message":"m"}"#,
            r#"{"v":1,"file":"a.jl","line":0,"col":1,"severity":"error","rule":"r","message":"m"}"#,
            r#"{"v":1,"file":"a.jl","line":1,"severity":"error","rule":"r","message":"m"}"#,
            r#"{"v":1,"file":"a.jl","line":1,"col":1,"severity":"fatal","rule":"r","message":"m"}"#,
            r#"{"v":1,"file":"a.jl","line":1,"col":1,"severity":"error","message":"m"}"#,
            r#"{"v":1,"file":"a.jl","line":1,"col":1,"severity":"error","rule":"r"}"#,
        ] {
            assert!(
                parse_julialint(line.as_bytes(), Some(0), &files).is_err(),
                "{line}"
            );
        }
    }
}
