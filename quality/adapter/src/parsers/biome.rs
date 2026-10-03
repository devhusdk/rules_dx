use serde::Deserialize;

use super::{check_output_size, code_name, known, unformatted_at, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct BiomeReport {
    diagnostics: Vec<BiomeDiagnostic>,
    command: String,
}

#[derive(Debug, Deserialize)]
struct BiomeDiagnostic {
    severity: String,
    message: String,
    category: String,
    location: BiomeLocation,
}

#[derive(Debug, Deserialize)]
struct BiomeLocation {
    path: String,
    start: BiomePosition,
    end: BiomePosition,
}

#[derive(Debug, Deserialize)]
struct BiomePosition {
    line: u64,
    column: u64,
}

fn biome_severity(level: &str) -> Result<ToolSeverity, ParseError> {
    match level {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        "info" => Ok(ToolSeverity::Info),
        _ => Err(ParseError::Shape {
            tool: "biome",
            detail: format!("unknown severity: {level}"),
        }),
    }
}

fn biome_position(
    tool: &'static str,
    what: &str,
    position: &BiomePosition,
) -> Result<TextPosition, ParseError> {
    if position.line < 1 || position.column < 1 {
        return Err(ParseError::Shape {
            tool,
            detail: format!("bad {what} position {}:{}", position.line, position.column),
        });
    }
    Ok(TextPosition {
        line: position.line,
        column: position.column,
    })
}

pub fn parse_biome_lint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "biome";
    check_output_size(TOOL, stdout)?;
    let report: BiomeReport = serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if report.command != "lint" {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unexpected command {:?}", report.command),
        });
    }
    let mut findings = Vec::with_capacity(report.diagnostics.len());
    for diagnostic in &report.diagnostics {
        let checked = known(TOOL, files, &diagnostic.location.path)?;
        if diagnostic.category.is_empty() || diagnostic.message.is_empty() {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: "diagnostic with an empty category or message".to_owned(),
            });
        }
        let start = biome_position(TOOL, "start", &diagnostic.location.start)?;
        let end = biome_position(TOOL, "end", &diagnostic.location.end)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.category.clone(),
                message: diagnostic.message.clone(),
                severity: biome_severity(&diagnostic.severity)?,
                start,
                end: Some(end),
                suggestions: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty diagnostics array", code_name(code)),
        });
    }
    Ok(findings)
}

pub fn parse_biome_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "biome_format";
    /// The id the registry knows biome by, which both of its capabilities report under.
    const REGISTRY: &str = "biome";
    check_output_size(TOOL, stdout)?;
    let report: BiomeReport = serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    if report.command != "format" {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("unexpected command {:?}", report.command),
        });
    }
    let mut findings = Vec::with_capacity(report.diagnostics.len());
    for diagnostic in &report.diagnostics {
        if diagnostic.category != "format" {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unexpected category {:?}", diagnostic.category),
            });
        }
        let zero = diagnostic.location.start.line == 0
            && diagnostic.location.start.column == 0
            && diagnostic.location.end.line == 0
            && diagnostic.location.end.column == 0;
        if !zero {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: "format diagnostic outside 0:0".to_owned(),
            });
        }
        let checked = known(TOOL, files, &diagnostic.location.path)?;
        findings.push(unformatted_at(REGISTRY, checked, 1, 1));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty diagnostics array", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIOME_LINT_DIRTY: &str = r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[{"severity":"warning","message":"This variable unusedVar is unused.","category":"lint/correctness/noUnusedVariables","location":{"path":"/s/dirty.js","start":{"line":1,"column":7},"end":{"line":1,"column":16}},"advices":[]}],"command":"lint"}"#;

    const BIOME_LINT_CLEAN: &str =
        r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[],"command":"lint"}"#;

    const BIOME_FMT_DIRTY: &str = r#"{"summary":{"changed":0,"unchanged":1},"diagnostics":[{"severity":"error","message":"Formatter would have printed the following content:","category":"format","location":{"path":"/s/fmt.js","start":{"line":0,"column":0},"end":{"line":0,"column":0}},"advices":[]}],"command":"format"}"#;

    #[test]
    fn biome_lint_reports_categories() {
        let findings = parse_biome_lint(BIOME_LINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.js"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.js");
        assert_eq!(findings[0].finding.tool_id, "biome");
        assert_eq!(
            findings[0].finding.rule_id,
            "lint/correctness/noUnusedVariables"
        );
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (
                findings[0].finding.start.line,
                findings[0].finding.start.column
            ),
            (1, 7)
        );
        let clean = parse_biome_lint(BIOME_LINT_CLEAN.as_bytes(), Some(0), &["/s/clean.js"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_biome_lint(BIOME_LINT_CLEAN.as_bytes(), Some(1), &["/s/clean.js"]).is_err());
        assert!(parse_biome_lint(b"not json", Some(1), &["/s/dirty.js"]).is_err());
        let wrong_command =
            BIOME_LINT_DIRTY.replace("\"command\":\"lint\"", "\"command\":\"format\"");
        assert!(parse_biome_lint(wrong_command.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let zero_pos = BIOME_LINT_DIRTY.replace(
            "\"start\":{\"line\":1,\"column\":7}",
            "\"start\":{\"line\":0,\"column\":0}",
        );
        assert!(parse_biome_lint(zero_pos.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let unknown_sev =
            BIOME_LINT_DIRTY.replace("\"severity\":\"warning\"", "\"severity\":\"hint\"");
        assert!(parse_biome_lint(unknown_sev.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let empty_category = BIOME_LINT_DIRTY.replace(
            "\"category\":\"lint/correctness/noUnusedVariables\"",
            "\"category\":\"\"",
        );
        assert!(parse_biome_lint(empty_category.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
        let empty_message = BIOME_LINT_DIRTY.replace(
            "\"message\":\"This variable unusedVar is unused.\"",
            "\"message\":\"\"",
        );
        assert!(parse_biome_lint(empty_message.as_bytes(), Some(1), &["/s/dirty.js"]).is_err());
    }

    #[test]
    fn biome_format_normalizes_to_file_findings() {
        let findings = parse_biome_format(BIOME_FMT_DIRTY.as_bytes(), Some(1), &["/s/fmt.js"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.tool_id, "biome");
        assert_eq!(findings[0].finding.rule_id, "");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let clean = r#"{"summary":{},"diagnostics":[],"command":"format"}"#;
        assert!(parse_biome_format(clean.as_bytes(), Some(0), &["/s/c.js"])
            .expect("parsed")
            .is_empty());
        assert!(parse_biome_format(clean.as_bytes(), Some(1), &["/s/c.js"]).is_err());
        assert!(parse_biome_format(b"not json", Some(1), &["/s/fmt.js"]).is_err());
        let wrong_cat = BIOME_FMT_DIRTY.replace(
            "\"category\":\"format\"",
            "\"category\":\"lint/style/noFoo\"",
        );
        assert!(parse_biome_format(wrong_cat.as_bytes(), Some(1), &["/s/fmt.js"]).is_err());
        let wrong_command =
            BIOME_FMT_DIRTY.replace("\"command\":\"format\"", "\"command\":\"lint\"");
        assert!(parse_biome_format(wrong_command.as_bytes(), Some(1), &["/s/fmt.js"]).is_err());
        let nonzero_pos = BIOME_FMT_DIRTY.replacen(
            "\"start\":{\"line\":0,\"column\":0}",
            "\"start\":{\"line\":1,\"column\":1}",
            1,
        );
        assert!(parse_biome_format(nonzero_pos.as_bytes(), Some(1), &["/s/fmt.js"]).is_err());
    }
}
