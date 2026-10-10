use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

fn json_fail(tool: &'static str, detail: String) -> ParseError {
    ParseError::Json { tool, detail }
}

fn shape_fail(tool: &'static str, detail: String) -> ParseError {
    ParseError::Shape { tool, detail }
}

fn position(value: &serde_json::Value, what: &str) -> Result<TextPosition, ParseError> {
    const TOOL: &str = "staticcheck";
    let line = value
        .get("line")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| shape_fail(TOOL, format!("finding {what} lacks a line")))?;
    let column = value
        .get("column")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| shape_fail(TOOL, format!("finding {what} lacks a column")))?;
    if line == 0 || column == 0 {
        return Err(shape_fail(
            TOOL,
            format!("finding {what} carries a zero position"),
        ));
    }
    Ok(TextPosition { line, column })
}

pub fn parse_staticcheck(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "staticcheck";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| shape_fail(TOOL, err.to_string()))?;
    let mut findings = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let item: serde_json::Value =
            serde_json::from_str(line).map_err(|err| json_fail(TOOL, err.to_string()))?;
        if !item.is_object() {
            return Err(shape_fail(
                TOOL,
                "staticcheck line is not an object".to_owned(),
            ));
        }
        let rule = item
            .get("code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a code".to_owned()))?;
        if rule.is_empty() {
            return Err(shape_fail(TOOL, "finding carries an empty code".to_owned()));
        }
        if rule == "compile" {
            let message = item
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("package load failure");
            return Err(shape_fail(
                TOOL,
                format!("build failure, not a finding: {message}"),
            ));
        }
        let severity = match item.get("severity").and_then(serde_json::Value::as_str) {
            Some("error") => ToolSeverity::Error,
            Some("warning") => ToolSeverity::Warning,
            Some(other) => {
                return Err(shape_fail(TOOL, format!("unknown severity: {other}")));
            }
            None => return Err(shape_fail(TOOL, "finding lacks a severity".to_owned())),
        };
        let location = item
            .get("location")
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a location".to_owned()))?;
        let path = location
            .get("file")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a file".to_owned()))?;
        let checked = known(TOOL, files, path)?;
        let start = position(location, "location")?;
        let end = match item.get("end") {
            Some(end) => position(end, "end").ok(),
            None => None,
        };
        let message = item
            .get("message")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| shape_fail(TOOL, "finding lacks a message".to_owned()))?;
        if message.is_empty() {
            return Err(shape_fail(
                TOOL,
                "finding carries an empty message".to_owned(),
            ));
        }
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: rule.to_owned(),
                message: message.to_owned(),
                severity,
                start,
                end,
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(shape_fail(
            TOOL,
            format!("exit {} with no findings", code_name(code)),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "{\"code\": \"SA4017\", \"severity\": \"error\", \"location\": {\"file\": \"go/tests/fixtures/staticcheck/Sample.go\", \"line\": 6, \"column\": 2}, \"end\": {\"file\": \"go/tests/fixtures/staticcheck/Sample.go\", \"line\": 6, \"column\": 31}, \"message\": \"Sprintf doesn't have side effects and its return value is ignored\"}";

    const SECOND: &str = "{\"code\": \"SA4017\", \"severity\": \"error\", \"location\": {\"file\": \"go/tests/fixtures/staticcheck/dep.go\", \"line\": 6, \"column\": 2}, \"end\": {\"file\": \"go/tests/fixtures/staticcheck/dep.go\", \"line\": 6, \"column\": 28}, \"message\": \"Sprintf doesn't have side effects and its return value is ignored\"}";

    const COMPILE: &str = "{\"code\": \"compile\", \"severity\": \"error\", \"location\": {\"file\": \"\", \"line\": 0, \"column\": 0}, \"end\": {\"file\": \"\", \"line\": 0, \"column\": 0}, \"message\": \"found packages app (app.go) and staticcheck (sample_copy.go) in /tmp/sc2/app\"}";

    const UNUSED: &str = "{\"code\": \"U1000\", \"severity\": \"error\", \"location\": {\"file\": \"go/tests/fixtures/staticcheck/Sample.go\", \"line\": 5, \"column\": 6}, \"end\": {\"file\": \"\", \"line\": 0, \"column\": 0}, \"message\": \"func Greet is unused\"}";

    #[test]
    fn staticcheck_reports_json_findings() {
        let findings = parse_staticcheck(
            DIRTY.as_bytes(),
            Some(1),
            &["go/tests/fixtures/staticcheck/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "go/tests/fixtures/staticcheck/Sample.go");
        assert_eq!(findings[0].finding.rule_id, "SA4017");
        assert_eq!(
            findings[0].finding.message,
            "Sprintf doesn't have side effects and its return value is ignored"
        );
        assert_eq!(findings[0].finding.start.line, 6);
        assert_eq!(findings[0].finding.start.column, 2);
        let end = findings[0].finding.end.expect("end");
        assert_eq!(end.line, 6);
        assert_eq!(end.column, 31);
        let trailed = format!("{DIRTY}\n");
        let retrailed = parse_staticcheck(
            trailed.as_bytes(),
            Some(1),
            &["go/tests/fixtures/staticcheck/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(retrailed.len(), 1);
        let clean = parse_staticcheck(b"", Some(0), &["go/tests/fixtures/staticcheck/Sample.go"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_staticcheck(b"", Some(1), &["go/tests/fixtures/staticcheck/Sample.go"]).is_err()
        );
        assert!(parse_staticcheck(DIRTY.as_bytes(), Some(1), &["other.go"]).is_err());
        assert!(parse_staticcheck(b"{}", Some(1), &["x"]).is_err());
        assert!(parse_staticcheck(b"[]", Some(0), &["x"]).is_err());
        assert!(parse_staticcheck(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn staticcheck_reports_each_line_finding() {
        let both = format!("{DIRTY}\n{SECOND}\n");
        let findings = parse_staticcheck(
            both.as_bytes(),
            Some(1),
            &[
                "go/tests/fixtures/staticcheck/Sample.go",
                "go/tests/fixtures/staticcheck/dep.go",
            ],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "go/tests/fixtures/staticcheck/Sample.go");
        assert_eq!(findings[1].file, "go/tests/fixtures/staticcheck/dep.go");
    }

    #[test]
    fn staticcheck_rejects_build_failures() {
        let err = parse_staticcheck(COMPILE.as_bytes(), Some(1), &["app/app.go"])
            .expect_err("compile is not a finding");
        assert!(err.to_string().contains("build failure"));
        assert!(parse_staticcheck(COMPILE.as_bytes(), Some(0), &["app/app.go"]).is_err());
    }

    #[test]
    fn staticcheck_keeps_findings_without_an_end() {
        let findings = parse_staticcheck(
            UNUSED.as_bytes(),
            Some(1),
            &["go/tests/fixtures/staticcheck/Sample.go"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "U1000");
        assert!(findings[0].finding.end.is_none());
    }
}
