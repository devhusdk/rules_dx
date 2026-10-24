use serde_json::Value;

use super::sarif::parse_sarif;
use super::{check_output_size, FileFinding, ParseError};

const TOOL: &str = "roslyn";

fn owned_file<'a>(uri: &str, files: &[&'a str]) -> Option<&'a str> {
    let path = dx_path::uri_to_path(uri).ok()?;
    let spelled = path.to_string_lossy().replace('\\', "/");
    files
        .iter()
        .filter(|file| {
            let file = file.replace('\\', "/");
            spelled == file || spelled.ends_with(&format!("/{}", file.trim_start_matches('/')))
        })
        .max_by_key(|file| file.len())
        .copied()
}

fn keep_locations(result: &mut Value, files: &[&str]) -> bool {
    let Some(locations) = result.get_mut("locations").and_then(Value::as_array_mut) else {
        return false;
    };
    locations.retain_mut(|location| {
        let Some(artifact) = location.pointer_mut("/physicalLocation/artifactLocation") else {
            return true;
        };
        let uri = artifact
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !uri.starts_with("file:") || artifact.get("uriBaseId").is_some() {
            return true;
        }
        match owned_file(uri, files) {
            Some(file) => {
                artifact["uri"] = Value::String(file.to_owned());
                true
            }
            None => false,
        }
    });
    !locations.is_empty()
}

fn analyzer_failure(result: &Value) -> Option<String> {
    let rule = result
        .get("ruleId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let level = result
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !rule.starts_with("AD") && level != "error" && level != "warning" {
        return None;
    }
    let message = result
        .pointer("/message/text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Some(format!("{rule}: {message}"))
}

pub fn parse_roslyn(sarif: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, sarif)?;
    let mut log: Value = serde_json::from_slice(sarif).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    for run in log
        .get_mut("runs")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let Some(results) = run.get_mut("results").and_then(Value::as_array_mut) else {
            continue;
        };
        let mut kept = Vec::with_capacity(results.len());
        for mut result in results.drain(..) {
            if result
                .get("ruleId")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: "result without a rule id".to_owned(),
                });
            }
            let has_locations = result
                .get("locations")
                .and_then(Value::as_array)
                .is_some_and(|locations| !locations.is_empty());
            if !has_locations {
                if let Some(detail) = analyzer_failure(&result) {
                    return Err(ParseError::Incomplete { tool: TOOL, detail });
                }
                continue;
            }
            if keep_locations(&mut result, files) {
                kept.push(result);
            } else if result.get("level").and_then(Value::as_str) == Some("error") {
                let rule = result["ruleId"].as_str().unwrap_or_default();
                let message = result
                    .pointer("/message/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                return Err(ParseError::Incomplete {
                    tool: TOOL,
                    detail: format!("{rule} outside the checked files: {message}"),
                });
            }
        }
        *results = kept;
    }
    let bytes = serde_json::to_vec(&log).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    parse_sarif(TOOL, &bytes, Some(0), files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextPosition, ToolSeverity};

    const SINGLE: &str = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "csc"}}, "results": [{"ruleId": "CA1822", "level": "warning", "message": {"text": "Member 'Greet' does not access instance data"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "csharp/tests/fixtures/roslyn/Sample.cs"}, "region": {"startLine": 7, "startColumn": 19, "endLine": 7, "endColumn": 24}}}]}]}]}"#;

    #[test]
    fn roslyn_reports_sarif_results() {
        let findings = parse_roslyn(
            SINGLE.as_bytes(),
            &["csharp/tests/fixtures/roslyn/Sample.cs"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "CA1822");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            findings[0].finding.start,
            TextPosition {
                line: 7,
                column: 19
            }
        );
        let end = findings[0].finding.end.expect("extent");
        assert_eq!((end.line, end.column), (7, 24));
        let clean = parse_roslyn(
            r#"{"version": "2.1.0", "runs": []}"#.as_bytes(),
            &["csharp/tests/fixtures/roslyn/Sample.cs"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_roslyn(SINGLE.as_bytes(), &["other.cs"]).is_err());
        assert!(parse_roslyn(b"not json", &["x"]).is_err());
        assert!(parse_roslyn(&[0xff], &["x"]).is_err());
        let no_rule = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"","locations":[]}]}]}"#;
        assert!(parse_roslyn(no_rule.as_bytes(), &["x"]).is_err());
    }

    #[test]
    fn roslyn_resolves_base_ids_and_every_location() {
        let log = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"originalUriBaseIds":{"%SRCROOT%":{"uri":"file:///src/"}},"results":[{"ruleId":"CA1822","level":"warning","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"Sample.cs","uriBaseId":"%SRCROOT%"},"region":{"startLine":7,"startColumn":19}}},{"physicalLocation":{"artifactLocation":{"uri":"Other.cs"},"region":{"startLine":9,"startColumn":1}}}]}]}]}"#;
        let findings =
            parse_roslyn(log.as_bytes(), &["/src/Sample.cs", "/src/Other.cs"]).expect("parsed");
        let files: Vec<&str> = findings.iter().map(|found| found.file.as_str()).collect();
        assert_eq!(files, vec!["/src/Sample.cs", "/src/Other.cs"]);
        assert_eq!(
            findings[1].finding.start,
            TextPosition { line: 9, column: 1 }
        );
    }

    const EXECROOT: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CA1016","level":"note","message":{"text":"Mark assemblies"}},{"ruleId":"CA1822","level":"note","message":{"text":"static"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///work/sandbox/1/execroot/_main/core/Greeter.cs"},"region":{"startLine":6,"startColumn":19}}}]},{"ruleId":"CS8019","level":"warning","message":{"text":"generated"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///work/sandbox/1/execroot/_main/bazel-out/gen/Other.cs"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#;

    #[test]
    fn roslyn_maps_execroot_paths_to_owned_files_and_skips_the_rest() {
        let findings = parse_roslyn(EXECROOT.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "core/Greeter.cs");
        assert_eq!(findings[0].finding.rule_id, "CA1822");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
    }

    #[test]
    fn roslyn_fails_when_an_analyzer_did_not_run() {
        let crashed = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"AD0001","level":"warning","message":{"text":"Analyzer 'X' threw an exception"}}]}]}"#;
        let error = parse_roslyn(crashed.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("AD0001"), "{error}");
        let broken = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS5001","level":"error","message":{"text":"no entry point"}}]}]}"#;
        assert!(parse_roslyn(broken.as_bytes(), &["a.cs"]).is_err());
        let load = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS8034","level":"warning","message":{"text":"Unable to load analyzer"}}]}]}"#;
        let error = parse_roslyn(load.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("CS8034"), "{error}");
        let raised = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CA1016","level":"warning","message":{"text":"Mark assemblies"}}]}]}"#;
        assert!(parse_roslyn(raised.as_bytes(), &["a.cs"]).is_err());
    }

    #[test]
    fn roslyn_fails_on_compile_errors_in_unowned_files() {
        let generated = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS0103","level":"error","message":{"text":"name does not exist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///work/execroot/_main/bazel-out/gen/Other.cs"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#;
        let error = parse_roslyn(generated.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("CS0103"), "{error}");
    }
}
