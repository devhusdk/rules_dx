use serde::Deserialize;

use super::{check_output_size, code_name, known, unformatted_at, FileFinding, ParseError};

#[derive(Debug, Deserialize)]
struct FantomasReport {
    #[serde(default)]
    files: Vec<FantomasFile>,
}

#[derive(Debug, Deserialize)]
struct FantomasFile {
    path: String,
    status: String,
}

pub fn parse_fantomas(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "fantomas";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if text.trim().is_empty() {
        if code == Some(0) {
            return Ok(Vec::new());
        }
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with empty output", code_name(code)),
        });
    }
    let report: FantomasReport = serde_json::from_str(text).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for entry in &report.files {
        let normalized = entry.path.strip_prefix("./").unwrap_or(&entry.path);
        match entry.status.as_str() {
            "unchanged" | "formatted" => {
                known(TOOL, files, normalized)?;
            }
            "needs-formatting" => {
                let checked = known(TOOL, files, normalized)?;
                findings.push(unformatted_at(TOOL, checked, 1, 1));
            }
            other => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: format!("unknown file status: {other}"),
                });
            }
        }
    }
    if findings.is_empty() {
        if code != Some(0) {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("exit {} with no needs-formatting files", code_name(code)),
            });
        }
    } else if code != Some(99) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!(
                "needs-formatting files with exit {} (want 99)",
                code_name(code)
            ),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = r#"{"files": [{"path": "fsharp/tests/fixtures/fantomas/Sample.fs", "status": "unchanged"}]}"#;
    const DIRTY: &str = r#"{"files": [{"path": "fsharp/tests/fixtures/fantomas/Sample.fs", "status": "needs-formatting"}]}"#;

    #[test]
    fn fantomas_reports_needs_formatting() {
        let findings = parse_fantomas(
            DIRTY.as_bytes(),
            Some(99),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "fsharp/tests/fixtures/fantomas/Sample.fs");
        let clean = parse_fantomas(
            CLEAN.as_bytes(),
            Some(0),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_fantomas(b"", Some(99), &["x"]).is_err());
        assert!(parse_fantomas(
            DIRTY.as_bytes(),
            Some(0),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"]
        )
        .is_err());
        assert!(parse_fantomas(
            CLEAN.as_bytes(),
            Some(99),
            &["fsharp/tests/fixtures/fantomas/Sample.fs"]
        )
        .is_err());
        assert!(parse_fantomas(DIRTY.as_bytes(), Some(99), &["other.fs"]).is_err());
        assert!(parse_fantomas(&[0xff], Some(99), &["x"]).is_err());
        let bad_status = r#"{"files": [{"path": "x", "status": "bogus"}]}"#;
        assert!(parse_fantomas(bad_status.as_bytes(), Some(99), &["x"]).is_err());
    }
}
