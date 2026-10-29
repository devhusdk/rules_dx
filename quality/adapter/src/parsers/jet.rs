use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::{as_text, finding, FileFinding, ParseError};

const TOOL: &str = "jet";

/// Findings from the JSON-lines report the pinned `jet.jl` driver writes.
///
/// Each line is `{"v":1,"kind":..,"message":..,"frames":[{"file":..,"line":..}]}` with the
/// frames ordered from the analysis entry to the error point. A finding lands on the
/// deepest frame inside an owned file; a report with no owned frame is dropped. The rule
/// is `jet/<kind>` and the severity is error. `files` holds the owned paths as the run
/// spells them.
pub fn parse_jet(report: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, report)?;
    let mut findings: Vec<FileFinding> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: serde_json::Value = serde_json::from_str(line).map_err(|err| {
            ParseError::shape(
                TOOL,
                format!("report line {} is not JSON: {err}", index + 1),
            )
        })?;
        if entry.get("v").and_then(serde_json::Value::as_u64) != Some(1) {
            return Err(ParseError::shape(
                TOOL,
                format!("report line {} has an unknown version", index + 1),
            ));
        }
        let text_of = |key: &str| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    ParseError::shape(TOOL, format!("report line {} has no {key}", index + 1))
                })
        };
        let kind = text_of("kind")?;
        let message = text_of("message")?;
        let frames = entry
            .get("frames")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ParseError::shape(TOOL, format!("report line {} has no frames", index + 1))
            })?;
        let mut owned: Option<(&str, u64)> = None;
        for frame in frames {
            let file = frame
                .get("file")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| ParseError::shape(TOOL, "frame without a file".to_owned()))?;
            let at = frame
                .get("line")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| ParseError::shape(TOOL, "frame without a line".to_owned()))?;
            if files.contains(&file) {
                owned = Some((file, at));
            }
        }
        if let Some((file, at)) = owned {
            findings.push(finding(
                TOOL,
                file,
                format!("jet/{kind}"),
                message.to_owned(),
                ToolSeverity::Error,
                TextPosition {
                    line: at.max(1),
                    column: 1,
                },
                ColumnEncoding::Utf8,
            ));
        }
    }
    findings.sort_by(|a, b| {
        (&a.file, a.finding.start.line, &a.finding.rule_id).cmp(&(
            &b.file,
            b.finding.start.line,
            &b.finding.rule_id,
        ))
    });
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = concat!(
        r#"{"v":1,"kind":"UndefVarError","message":"`x` is not defined","frames":[{"file":"/s/a.jl","line":2},{"file":"/s/dep.jl","line":9},{"file":"/s/a.jl","line":7}]}"#,
        "\n",
        r#"{"v":1,"kind":"MethodError","message":"no method","frames":[{"file":"/s/dep.jl","line":3}]}"#,
        "\n",
        r#"{"v":1,"kind":"ToplevelError","message":"bad","frames":[{"file":"/s/b.jl","line":0}]}"#,
        "\n"
    );

    #[test]
    fn jet_uses_the_deepest_owned_frame_and_drops_dependency_reports() {
        let found = parse_jet(REPORT.as_bytes(), &["/s/a.jl", "/s/b.jl"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| {
                (
                    f.file.as_str(),
                    f.finding.start.line,
                    f.finding.rule_id.as_str(),
                    f.finding.severity,
                )
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                ("/s/a.jl", 7, "jet/UndefVarError", ToolSeverity::Error),
                ("/s/b.jl", 1, "jet/ToplevelError", ToolSeverity::Error),
            ]
        );
    }

    #[test]
    fn jet_empty_reports_have_no_findings() {
        assert!(parse_jet(b"", &["/s/a.jl"]).expect("empty").is_empty());
        assert!(parse_jet(REPORT.as_bytes(), &["/s/other.jl"])
            .expect("other")
            .is_empty());
    }

    #[test]
    fn jet_malformed_reports_fail_the_run() {
        assert!(parse_jet(b"not json", &[]).is_err());
        assert!(parse_jet(br#"{"v":2,"kind":"k","message":"m","frames":[]}"#, &[]).is_err());
        assert!(parse_jet(br#"{"v":1,"message":"m","frames":[]}"#, &[]).is_err());
        assert!(parse_jet(br#"{"v":1,"kind":"k","message":"m"}"#, &[]).is_err());
        assert!(parse_jet(
            br#"{"v":1,"kind":"k","message":"m","frames":[{"file":"/s/a.jl"}]}"#,
            &["/s/a.jl"]
        )
        .is_err());
    }
}
