use serde_json::Value;

use super::sarif::{parse_sarif, suppressed};
use super::{check_output_size, FileFinding, ParseError};

const TOOL: &str = "roslyn";

const EXEC_ROOT: &str = "/execroot/_main/";

fn exec_relative(spelled: &str) -> Option<&str> {
    spelled
        .rfind(EXEC_ROOT)
        .map(|at| &spelled[at + EXEC_ROOT.len()..])
}

fn outside_the_main_repository(prefix: &str) -> bool {
    prefix
        .split('/')
        .any(|segment| matches!(segment, "bazel-out" | "external" | "execroot"))
}

fn owned_file<'a>(uri: &str, files: &[&'a str]) -> Option<&'a str> {
    let path = dx_path::uri_to_path(uri).ok()?;
    let spelled = path.to_string_lossy().replace('\\', "/");
    let owned = files
        .iter()
        .map(|file| (*file, file.replace('\\', "/")))
        .collect::<Vec<_>>();
    if let Some((file, _)) = owned.iter().find(|(_, normal)| *normal == spelled) {
        return Some(file);
    }
    if let Some(relative) = exec_relative(&spelled) {
        return owned
            .iter()
            .find(|(_, normal)| normal.trim_start_matches('/') == relative)
            .map(|(file, _)| *file);
    }
    owned
        .iter()
        .filter(|(_, normal)| {
            spelled
                .strip_suffix(normal.trim_start_matches('/'))
                .and_then(|prefix| prefix.strip_suffix('/'))
                .is_some_and(|prefix| !outside_the_main_repository(prefix))
        })
        .max_by_key(|(file, _)| file.len())
        .map(|(file, _)| *file)
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

fn map_related_locations(result: &mut Value, files: &[&str]) {
    let Some(related) = result
        .get_mut("relatedLocations")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for location in related {
        let Some(artifact) = location.pointer_mut("/physicalLocation/artifactLocation") else {
            continue;
        };
        let uri = artifact
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !uri.starts_with("file:") || artifact.get("uriBaseId").is_some() {
            continue;
        }
        if let Some(file) = owned_file(uri, files) {
            artifact["uri"] = Value::String(file.to_owned());
        }
    }
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

pub fn parse_roslyn(
    sarif: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, sarif)?;
    let mut log: Value = serde_json::from_slice(sarif).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut has_error = false;
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
            let rule = result["ruleId"].as_str().unwrap_or_default();
            if !rule.starts_with("AD")
                && suppressed(
                    result
                        .get("suppressions")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|suppression| suppression.get("status")),
                )
            {
                continue;
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
                map_related_locations(&mut result, files);
                has_error |= result.get("level").and_then(Value::as_str) == Some("error");
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
    if code != Some(0) && !has_error {
        let exit = code.map_or_else(|| "signal".to_owned(), |code| code.to_string());
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: format!("csc exit {exit} without error results"),
        });
    }
    let bytes = serde_json::to_vec(&log).map_err(|err| ParseError::Json {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    parse_sarif(TOOL, &bytes, code, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TextPosition, ToolSeverity};

    fn parse_exit_zero(sarif: &[u8], files: &[&str]) -> Result<Vec<FileFinding>, ParseError> {
        parse_roslyn(sarif, Some(0), files)
    }

    const SINGLE: &str = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "csc"}}, "results": [{"ruleId": "CA1822", "level": "warning", "message": {"text": "Member 'Greet' does not access instance data"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "csharp/tests/fixtures/roslyn/Sample.cs"}, "region": {"startLine": 7, "startColumn": 19, "endLine": 7, "endColumn": 24}}}]}]}]}"#;

    #[test]
    fn roslyn_reports_sarif_results() {
        let findings = parse_exit_zero(
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
        let clean = parse_exit_zero(
            r#"{"version": "2.1.0", "runs": []}"#.as_bytes(),
            &["csharp/tests/fixtures/roslyn/Sample.cs"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_exit_zero(SINGLE.as_bytes(), &["other.cs"]).is_err());
        assert!(parse_exit_zero(b"not json", &["x"]).is_err());
        assert!(parse_exit_zero(&[0xff], &["x"]).is_err());
        let no_rule = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"","locations":[]}]}]}"#;
        assert!(parse_exit_zero(no_rule.as_bytes(), &["x"]).is_err());
    }

    #[test]
    fn roslyn_resolves_base_ids_and_every_location() {
        let log = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"originalUriBaseIds":{"%SRCROOT%":{"uri":"file:///src/"}},"results":[{"ruleId":"CA1822","level":"warning","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"Sample.cs","uriBaseId":"%SRCROOT%"},"region":{"startLine":7,"startColumn":19}}},{"physicalLocation":{"artifactLocation":{"uri":"Other.cs"},"region":{"startLine":9,"startColumn":1}}}]}]}]}"#;
        let findings =
            parse_exit_zero(log.as_bytes(), &["/src/Sample.cs", "/src/Other.cs"]).expect("parsed");
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
        let findings = parse_exit_zero(EXECROOT.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "core/Greeter.cs");
        assert_eq!(findings[0].finding.rule_id, "CA1822");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
    }

    #[test]
    fn roslyn_maps_execroot_related_locations_to_owned_files() {
        let log = csc(
            r#"{"ruleId":"CA1822","level":"warning","message":{"text":"static"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///w/execroot/_main/core/Greeter.cs"},"region":{"startLine":6,"startColumn":19}}}],"relatedLocations":[{"message":{"text":"declared here"},"physicalLocation":{"artifactLocation":{"uri":"file:///w/execroot/_main/core/Greeter.cs"},"region":{"startLine":2,"startColumn":3}}},{"message":{"text":"generated"},"physicalLocation":{"artifactLocation":{"uri":"file:///w/execroot/_main/bazel-out/gen/Other.cs"},"region":{"startLine":1,"startColumn":1}}}]}"#,
        );
        let findings = parse_exit_zero(log.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        let related = &findings[0].finding.related;
        assert_eq!(related.len(), 2);
        let location = related[0].location.as_ref().expect("owned related file");
        assert_eq!(location.file, "core/Greeter.cs");
        assert!(related[1].location.is_none());
    }

    #[test]
    fn roslyn_fails_when_an_analyzer_did_not_run() {
        let crashed = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"AD0001","level":"warning","message":{"text":"Analyzer 'X' threw an exception"}}]}]}"#;
        let error = parse_exit_zero(crashed.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("AD0001"), "{error}");
        let broken = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS5001","level":"error","message":{"text":"no entry point"}}]}]}"#;
        assert!(parse_exit_zero(broken.as_bytes(), &["a.cs"]).is_err());
        let load = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS8034","level":"warning","message":{"text":"Unable to load analyzer"}}]}]}"#;
        let error = parse_exit_zero(load.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("CS8034"), "{error}");
        let raised = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CA1016","level":"warning","message":{"text":"Mark assemblies"}}]}]}"#;
        assert!(parse_exit_zero(raised.as_bytes(), &["a.cs"]).is_err());
    }

    #[test]
    fn roslyn_fails_on_compile_errors_in_unowned_files() {
        let generated = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS0103","level":"error","message":{"text":"name does not exist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///work/execroot/_main/bazel-out/gen/Other.cs"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#;
        let error = parse_exit_zero(generated.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("CS0103"), "{error}");
    }

    fn warnings_at(uris: &[&str]) -> String {
        let results = uris
            .iter()
            .enumerate()
            .map(|(line, uri)| {
                let line = line + 1;
                format!(
                    r#"{{"ruleId":"CA1822","level":"warning","message":{{"text":"static"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{uri}"}},"region":{{"startLine":{line},"startColumn":1}}}}}}]}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"csc"}}}},"results":[{results}]}}]}}"#
        )
    }

    fn files_of(findings: &[FileFinding]) -> Vec<&str> {
        findings.iter().map(|found| found.file.as_str()).collect()
    }

    #[test]
    fn roslyn_ignores_generated_and_external_suffix_collisions() {
        let log = warnings_at(&[
            "file:///w/sandbox/3/execroot/_main/bazel-out/k8-fastbuild/bin/gen/Program.cs",
            "file:///w/execroot/_main/external/foo/Program.cs",
            "file:///w/execroot/_main/external/foo/app/Program.cs",
            "file:///w/execroot/foo/Program.cs",
            "file:///w/execroot/_main/app/Program.cs",
        ]);
        let findings =
            parse_exit_zero(log.as_bytes(), &["Program.cs", "app/Program.cs"]).expect("parsed");
        assert_eq!(files_of(&findings), vec!["app/Program.cs"]);
    }

    #[test]
    fn roslyn_maps_remote_and_windows_paths_to_owned_files() {
        let remote = warnings_at(&[
            "file:///b/f/w/core/Greeter.cs",
            "file:///b/f/w/bazel-out/k8/bin/core/Greeter.cs",
            "file:///b/f/my_external/core/Greeter.cs",
        ]);
        let findings = parse_exit_zero(remote.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        assert_eq!(
            files_of(&findings),
            vec!["core/Greeter.cs", "core/Greeter.cs"]
        );
        let windows = warnings_at(&["file:///C:/b/execroot/_main/core/Greeter.cs"]);
        let findings = parse_exit_zero(windows.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        assert_eq!(files_of(&findings), vec!["core/Greeter.cs"]);
        let nested =
            warnings_at(&["file:///home/execroot/_main/cache/execroot/_main/core/Greeter.cs"]);
        let findings = parse_exit_zero(nested.as_bytes(), &["core/Greeter.cs"]).expect("parsed");
        assert_eq!(files_of(&findings), vec!["core/Greeter.cs"]);
        let absolute = warnings_at(&["file:///src/execroot/_main/core/Greeter.cs"]);
        let findings = parse_exit_zero(
            absolute.as_bytes(),
            &["/src/execroot/_main/core/Greeter.cs"],
        )
        .expect("parsed");
        assert_eq!(
            files_of(&findings),
            vec!["/src/execroot/_main/core/Greeter.cs"]
        );
    }

    #[test]
    fn roslyn_fails_on_compile_errors_in_generated_files_named_like_owned_ones() {
        let generated = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CS0103","level":"error","message":{"text":"name does not exist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///w/execroot/_main/bazel-out/k8-fastbuild/bin/gen/Program.cs"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#;
        let error = parse_exit_zero(generated.as_bytes(), &["Program.cs"]).expect_err("incomplete");
        assert!(error.to_string().contains("CS0103"), "{error}");
    }

    fn csc(results: &str) -> String {
        format!(
            r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"csc"}}}},"results":[{results}]}}]}}"#
        )
    }

    const SAMPLE: &str = "csharp/tests/fixtures/roslyn/Sample.cs";

    fn located(suppressions: &str) -> String {
        csc(&format!(
            r#"{{"ruleId":"CA1822","level":"warning","message":{{"text":"static"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{SAMPLE}"}},"region":{{"startLine":7,"startColumn":19}}}}}}],"suppressions":{suppressions}}}"#
        ))
    }

    fn assembly(suppressions: &str) -> String {
        csc(&format!(
            r#"{{"ruleId":"CA1016","level":"warning","message":{{"text":"Mark assemblies"}},"suppressions":{suppressions}}}"#
        ))
    }

    #[test]
    fn roslyn_drops_suppressed_results() {
        let in_source = r#"[{"kind":"inSource"}]"#;
        let findings = parse_exit_zero(located(in_source).as_bytes(), &[SAMPLE]).expect("parsed");
        assert!(findings.is_empty(), "{findings:?}");

        let findings = parse_exit_zero(assembly(in_source).as_bytes(), &["a.cs"]).expect("parsed");
        assert!(findings.is_empty(), "{findings:?}");

        let unowned = csc(
            r#"{"ruleId":"CS0103","level":"error","message":{"text":"name does not exist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///work/execroot/_main/bazel-out/gen/Other.cs"},"region":{"startLine":1,"startColumn":1}}}],"suppressions":[{"kind":"inSource","status":"accepted"}]}"#,
        );
        let findings = parse_exit_zero(unowned.as_bytes(), &["a.cs"]).expect("parsed");
        assert!(findings.is_empty(), "{findings:?}");

        let crashed = csc(
            r#"{"ruleId":"AD0001","level":"warning","message":{"text":"Analyzer 'X' threw an exception"},"suppressions":[{"kind":"inSource"}]}"#,
        );
        let error = parse_exit_zero(crashed.as_bytes(), &["a.cs"]).expect_err("incomplete");
        assert!(matches!(error, ParseError::Incomplete { .. }), "{error}");
        assert!(error.to_string().contains("AD0001"), "{error}");

        for open in [
            r#"[{"kind":"inSource","status":"rejected"}]"#,
            r#"[{"kind":"inSource","status":"underReview"}]"#,
            r#"[{"kind":"inSource"},{"kind":"external","status":"underReview"}]"#,
            "[]",
        ] {
            let findings = parse_exit_zero(located(open).as_bytes(), &[SAMPLE]).expect("parsed");
            assert_eq!(findings.len(), 1, "{open}");
            assert_eq!(findings[0].finding.rule_id, "CA1822");
            let error = parse_exit_zero(assembly(open).as_bytes(), &["a.cs"]).expect_err(open);
            assert!(matches!(error, ParseError::Incomplete { .. }), "{error}");
            assert!(error.to_string().contains("CA1016"), "{error}");
        }
    }

    #[test]
    fn roslyn_fails_when_csc_exits_nonzero_without_error_results() {
        let empty = csc("");
        let error = parse_roslyn(empty.as_bytes(), Some(1), &["a.cs"]).expect_err("incomplete");
        assert!(matches!(error, ParseError::Incomplete { .. }), "{error}");
        assert!(parse_roslyn(empty.as_bytes(), None, &["a.cs"]).is_err());
        let warning = located("[]");
        assert!(parse_roslyn(warning.as_bytes(), Some(1), &[SAMPLE]).is_err());
        let compile_error = csc(
            r#"{"ruleId":"CS0103","level":"error","message":{"text":"The name 'x' does not exist"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.cs"},"region":{"startLine":3,"startColumn":5}}}]}"#,
        );
        let findings =
            parse_roslyn(compile_error.as_bytes(), Some(1), &["a.cs"]).expect("compile error");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "CS0103");
    }
}
