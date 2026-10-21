use std::path::{Path, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

use url::Url;

use serde_sarif::sarif::{ArtifactLocation, Invocation, Notification, Region, ResultLevel, Sarif};

use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity, LINE_END};

fn sarif_severity(level: Option<ResultLevel>) -> ToolSeverity {
    match level {
        None | Some(ResultLevel::Warning) => ToolSeverity::Warning,
        Some(ResultLevel::Error) => ToolSeverity::Error,
        Some(ResultLevel::Note) | Some(ResultLevel::None) => ToolSeverity::Info,
    }
}

fn drive_rooted(path: &str) -> bool {
    dx_path::drive_rooted(path.trim_start_matches('/'))
}

fn rooted(path: &str) -> bool {
    path.starts_with(['/', MAIN_SEPARATOR]) || drive_rooted(path)
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join(MAIN_SEPARATOR_STR);
    if drive_rooted(path) {
        joined
    } else if path.starts_with('/') {
        format!("{MAIN_SEPARATOR_STR}{joined}")
    } else {
        joined
    }
}

fn suffix_for(path: &str) -> String {
    let spelled = dx_path::native(Path::new(path));
    if rooted(path) {
        spelled
    } else {
        format!("{}{spelled}", dx_path::native(Path::new("/")))
    }
}

fn match_spelling<'a>(files: &[&'a str], path: &str) -> Option<&'a str> {
    if let Some(hit) = files.iter().find(|file| **file == path) {
        return Some(*hit);
    }
    unique_suffix(files, &suffix_for(path))
}

fn unique_suffix<'a>(files: &[&'a str], suffix: &str) -> Option<&'a str> {
    let mut hits = files.iter().filter(|file| file.ends_with(suffix));
    let hit = hits.next()?;
    hits.next().is_none().then_some(*hit)
}

fn parse_reference(tool: &'static str, base: &Url, text: &str) -> Result<Url, ParseError> {
    let mut chars = text.chars();
    let drive_like =
        matches!((chars.next(), chars.next()), (Some(c), Some(':')) if c.is_ascii_alphabetic());
    let text = if drive_like {
        format!("./{text}")
    } else {
        text.to_owned()
    };
    let url = Url::options()
        .base_url(Some(base))
        .parse(&text)
        .map_err(|err| ParseError::Shape {
            tool,
            detail: format!("malformed URI {text:?}: {err}"),
        })?;
    if url.scheme() != "file" {
        return Err(ParseError::Shape {
            tool,
            detail: format!("unsupported URI scheme in {text:?}"),
        });
    }
    Ok(url)
}

fn resolve_file<'a>(
    tool: &'static str,
    files: &[&'a str],
    uri: &str,
    base_id: Option<&str>,
    bases: &std::collections::BTreeMap<String, ArtifactLocation>,
) -> Result<&'a str, ParseError> {
    let root = parse_reference(
        tool,
        &Url::from_directory_path("/").map_err(|()| ParseError::Shape {
            tool,
            detail: "no root URL".to_owned(),
        })?,
        "/",
    )?;
    let base = match base_id {
        Some(id) => {
            let location = bases.get(id).ok_or_else(|| ParseError::Shape {
                tool,
                detail: format!("unknown uriBaseId: {id}"),
            })?;
            let text = location.uri.as_deref().unwrap_or_default();
            let text = if text.ends_with('/') {
                text.to_owned()
            } else {
                format!("{text}/")
            };
            parse_reference(tool, &root, &text)?
        }
        None => root,
    };
    let target = parse_reference(tool, &base, uri)?;
    let path = dx_path::uri_to_path(target.as_str()).map_err(|err| ParseError::Shape {
        tool,
        detail: format!("unsupported file URI {uri:?}: {err}"),
    })?;
    let spelled = path.to_string_lossy().replace(MAIN_SEPARATOR, "/");
    let normalized = normalize_path(&spelled);
    if base_id.is_none() && !uri.starts_with('/') && !uri.starts_with("file:") {
        let relative = normalized.trim_start_matches(MAIN_SEPARATOR);
        if let Some(hit) = files.iter().find(|file| **file == relative) {
            return Ok(hit);
        }
    }
    if let Some(hit) = match_spelling(files, &normalized) {
        return Ok(hit);
    }
    Err(ParseError::UnknownFile {
        tool,
        path: uri.to_owned(),
    })
}

fn start(tool: &'static str, region: &Region) -> Result<TextPosition, ParseError> {
    let line = region.start_line.ok_or_else(|| ParseError::Shape {
        tool,
        detail: "location without start line".to_owned(),
    })?;
    let column = region.start_column.unwrap_or(1);
    if line < 1 || column < 1 {
        return Err(ParseError::Shape {
            tool,
            detail: format!("bad start position {line}:{column}"),
        });
    }
    Ok(TextPosition {
        line: line as u64,
        column: column as u64,
    })
}

fn end(
    tool: &'static str,
    region: &Region,
    start: TextPosition,
) -> Result<Option<TextPosition>, ParseError> {
    if region.end_line.is_none() && region.end_column.is_none() {
        return Ok(None);
    }
    let line = region.end_line.unwrap_or(start.line as i64);
    if let Some(column) = region.end_column {
        if line < 1 || column < 1 {
            return Err(ParseError::Shape {
                tool,
                detail: format!("bad end position {line}:{column}"),
            });
        }
    }
    if line < 1 || (line as u64) < start.line {
        return Err(ParseError::Shape {
            tool,
            detail: format!("end line {line} precedes start line {}", start.line),
        });
    }
    Ok(Some(TextPosition {
        line: line as u64,
        column: region.end_column.map_or(LINE_END, |column| column as u64),
    }))
}

fn error_notifications(invocation: &Invocation) -> Vec<String> {
    let groups = [
        &invocation.tool_configuration_notifications,
        &invocation.tool_execution_notifications,
    ];
    groups
        .into_iter()
        .flatten()
        .flatten()
        .filter(|note: &&Notification| {
            note.level.as_ref().and_then(|v| v.as_str()) == Some("error")
        })
        .map(|note| note.message.text.clone().unwrap_or_default())
        .collect()
}

fn check_invocations(tool: &'static str, invocations: &[Invocation]) -> Result<(), ParseError> {
    for invocation in invocations {
        let errors = error_notifications(invocation);
        if invocation.execution_successful && errors.is_empty() {
            continue;
        }
        let mut detail = if invocation.execution_successful {
            "error notifications".to_owned()
        } else {
            "invocation was not successful".to_owned()
        };
        if let Some(message) = &invocation.process_start_failure_message {
            detail.push_str(&format!(": {message}"));
        }
        if !errors.is_empty() {
            detail.push_str(&format!(": {}", errors.join("; ")));
        }
        return Err(ParseError::Incomplete { tool, detail });
    }
    Ok(())
}

pub fn parse_sarif(
    tool: &'static str,
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(tool, stdout)?;
    let log: Sarif = serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
        tool,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for run in &log.runs {
        check_invocations(tool, run.invocations.as_deref().unwrap_or_default())?;
        let bases = run.original_uri_base_ids.as_ref();
        let empty = std::collections::BTreeMap::new();
        let bases = bases.unwrap_or(&empty);
        for result in run.results.iter().flatten() {
            let text = result.message.text.as_deref().unwrap_or_default();
            if text.is_empty() {
                return Err(ParseError::Shape {
                    tool,
                    detail: "result with empty message".to_owned(),
                });
            }
            let severity = sarif_severity(result.level);
            let rule_id = result.rule_id.clone().unwrap_or_default();
            let locations = result.locations.as_deref().unwrap_or_default();
            if locations.is_empty() {
                return Err(ParseError::Shape {
                    tool,
                    detail: "result with no locations".to_owned(),
                });
            }
            for location in locations {
                let physical = location
                    .physical_location
                    .as_ref()
                    .ok_or(ParseError::Shape {
                        tool,
                        detail: "location without physicalLocation".to_owned(),
                    })?;
                let artifact = physical.artifact_location.as_ref();
                let uri = artifact
                    .and_then(|artifact| artifact.uri.as_deref())
                    .unwrap_or_default();
                if uri.is_empty() {
                    return Err(ParseError::Shape {
                        tool,
                        detail: "location without artifact uri".to_owned(),
                    });
                }
                let region = physical.region.as_ref().ok_or(ParseError::Shape {
                    tool,
                    detail: "location without region".to_owned(),
                })?;
                let start = start(tool, region)?;
                let end = end(tool, region, start)?;
                let checked = resolve_file(
                    tool,
                    files,
                    uri,
                    artifact.and_then(|artifact| artifact.uri_base_id.as_deref()),
                    bases,
                )?;
                findings.push(FileFinding {
                    file: checked.to_owned(),
                    finding: Finding {
                        tool_id: tool.to_owned(),
                        rule_id: rule_id.clone(),
                        message: text.to_owned(),
                        severity,
                        start,
                        end,
                        suggestions: Vec::new(),
                    },
                });
            }
        }
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool,
            detail: format!("exit {} with no results", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL: &str = "sarif-test";

    fn log(results: &str) -> Vec<u8> {
        format!("{{\"version\":\"2.1.0\",\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"t\"}}}},\"results\":[{results}]}}]}}").into_bytes()
    }

    #[test]
    fn sarif_reports_point_and_range() {
        let stdout = log(
            r#"{"ruleId":"R1","level":"error","message":{"text":"boom"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:/s/a.java"},"region":{"startLine":2,"startColumn":8}}}]},{"ruleId":"R2","level":"warning","message":{"text":"warn"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":3,"startColumn":1,"endLine":3,"endColumn":5}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &stdout, Some(1), &["/s/a.java"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding.rule_id, "R1");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(findings[0].finding.end.is_none());
        let end = findings[1].finding.end.expect("extent");
        assert_eq!((end.line, end.column), (3, 5));
        let clean_json = log("");
        let clean = parse_sarif(TOOL, &clean_json, Some(0), &["/s/a.java"]).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_sarif(TOOL, &clean_json, Some(1), &["/s/a.java"]).is_err());
        assert!(parse_sarif(TOOL, b"not json", Some(1), &["/s/a.java"]).is_err());
    }

    fn with_invocation(invocation: &str, results: &str) -> Vec<u8> {
        format!("{{\"version\":\"2.1.0\",\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"t\"}}}},\"results\":[{results}],\"invocations\":[{invocation}]}}]}}").into_bytes()
    }

    const FINDING: &str = r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1}}}]}"#;

    #[test]
    fn a_failed_invocation_is_incomplete_analysis() {
        let files = ["/s/a.java"];
        let missing = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[],"invocations":[{"executionSuccessful":false,"toolConfigurationNotifications":[{"level":"error","message":{"text":"Classes needed for analysis were missing: [example.Dependency]"}}]}]}]}"#;
        let err = parse_sarif(TOOL, missing, Some(0), &files).expect_err("incomplete");
        assert!(matches!(err, ParseError::Incomplete { .. }));
        assert!(err.to_string().contains("example.Dependency"), "{err}");

        let bare = with_invocation(r#"{"executionSuccessful":false}"#, "");
        assert!(parse_sarif(TOOL, &bare, Some(0), &files).is_err());

        let mixed = with_invocation(r#"{"executionSuccessful":false}"#, FINDING);
        assert!(matches!(
            parse_sarif(TOOL, &mixed, Some(1), &files),
            Err(ParseError::Incomplete { .. })
        ));

        let execution = with_invocation(
            r#"{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"error","message":{"text":"boom"}}]}"#,
            "",
        );
        let err = parse_sarif(TOOL, &execution, Some(0), &files).expect_err("error note");
        assert!(err.to_string().contains("boom"), "{err}");

        let second = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},"results":[]},{"tool":{"driver":{"name":"t"}},"results":[],"invocations":[{"executionSuccessful":true},{"executionSuccessful":false}]}]}"#;
        assert!(parse_sarif(TOOL, second, Some(0), &files).is_err());
    }

    #[test]
    fn a_successful_invocation_keeps_its_findings() {
        let files = ["/s/a.java"];
        let ok = with_invocation(
            r#"{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"warning","message":{"text":"w"}},{"level":"note","message":{"text":"n"}}]}"#,
            "",
        );
        assert!(parse_sarif(TOOL, &ok, Some(0), &files)
            .expect("clean")
            .is_empty());
        let found = with_invocation(r#"{"executionSuccessful":true}"#, FINDING);
        assert_eq!(
            parse_sarif(TOOL, &found, Some(1), &files)
                .expect("found")
                .len(),
            1
        );
    }

    #[test]
    fn a_windows_uri_resolves_to_the_native_file_name() {
        let sep = std::path::MAIN_SEPARATOR;
        let drive = format!("C:{sep}src{sep}a.cs");
        let rooted = format!("{sep}src{sep}a.cs");
        let relative = format!("src{sep}a.cs");
        assert!(drive_rooted("C:/src/a.cs"));
        assert!(drive_rooted("/C:/src/a.cs"));
        assert!(!drive_rooted("/src/a.cs"));
        assert_eq!(normalize_path("C:/src/a.cs"), drive);
        assert_eq!(normalize_path("/C:/src/a.cs"), drive);
        assert_eq!(normalize_path("/src/a.cs"), rooted);
        assert_eq!(
            suffix_for("C:/src/a.cs"),
            dx_path::native(Path::new("C:/src/a.cs"))
        );
        assert_eq!(
            suffix_for("src/a.cs"),
            format!("{MAIN_SEPARATOR_STR}{relative}")
        );
        let stdout = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///C:/src/a.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &stdout, Some(1), &[&drive]).expect("drive uri");
        assert_eq!(findings[0].file, drive);
        let relative_uri = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/a.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &relative_uri, Some(1), &[&drive]).expect("relative uri");
        assert_eq!(findings[0].file, drive);
        let based = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"k"}},"originalUriBaseIds":{"%SRCROOT%":{"uri":"file:///C:/src/"}},"results":[{"ruleId":"R","level":"error","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"../other/a.cs","uriBaseId":"%SRCROOT%"},"region":{"startLine":1}}}]}]}]}"#;
        let other = format!("C:{sep}other{sep}a.cs");
        let findings = parse_sarif(TOOL, based, Some(1), &[&other]).expect("drive base join");
        assert_eq!(findings[0].file, other);
    }

    #[test]
    fn a_percent_encoded_uri_resolves_to_the_native_file_name() {
        let sep = std::path::MAIN_SEPARATOR;
        let drive = format!("C:{sep}src{sep}my file.cs");
        let stdout = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///C:/src/my%20file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &stdout, Some(1), &[&drive]).expect("encoded drive uri");
        assert_eq!(findings[0].file, drive);
        let relative = format!("/scratch{sep}src{sep}my file.cs");
        let relative_uri = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/my%20file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings =
            parse_sarif(TOOL, &relative_uri, Some(1), &[&relative]).expect("encoded relative uri");
        assert_eq!(findings[0].file, relative);
        let literal = format!("/scratch{sep}src{sep}my%20file.cs");
        let both = [literal.as_str(), relative.as_str()];
        let findings = parse_sarif(TOOL, &relative_uri, Some(1), &both).expect("both spellings");
        assert_eq!(
            findings[0].file, relative,
            "the encoded URI names the decoded file, not the literal-percent one"
        );
        let double = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///scratch/src/my%2520file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &double, Some(1), &both).expect("double encoded");
        assert_eq!(findings[0].file, literal);
        let single = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:///scratch/src/my%20file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &single, Some(1), &both).expect("single encoded");
        assert_eq!(findings[0].file, relative);
        let only_literal = [literal.as_str()];
        assert!(
            parse_sarif(TOOL, &single, Some(1), &only_literal).is_err(),
            "a literal-percent file is not the decoded name"
        );
        let local = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file://localhost/scratch/src/my%20file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &local, Some(1), &both).expect("localhost");
        assert_eq!(findings[0].file, relative);
        let foreign = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"https://x/a.cs"},"region":{"startLine":3}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &foreign, Some(1), &both).is_err());
        let ambiguous = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"my%20file.cs"},"region":{"startLine":3}}}]}"#,
        );
        let other = format!("/other{sep}src{sep}my file.cs");
        let checked = [relative.as_str(), other.as_str()];
        assert!(
            parse_sarif(TOOL, &ambiguous, Some(1), &checked).is_err(),
            "an ambiguous suffix names no one file"
        );
    }

    #[test]
    fn a_drive_relative_uri_names_no_other_file() {
        let sep = std::path::MAIN_SEPARATOR;
        assert!(!drive_rooted("a:b/c.cs"));
        assert!(!drive_rooted("C:notes/notes.md"));
        assert_eq!(
            suffix_for("a:b/c.cs"),
            format!("{MAIN_SEPARATOR_STR}a:b{sep}c.cs")
        );
        assert_eq!(
            suffix_for("C:notes"),
            format!("{MAIN_SEPARATOR_STR}C:notes")
        );
        let stdout = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a:b/c.cs"},"region":{"startLine":3}}}]}"#,
        );
        let sibling = format!("/proj{sep}za:b{sep}c.cs");
        assert!(
            parse_sarif(TOOL, &stdout, Some(1), &[&sibling]).is_err(),
            "the colon belongs to the name, so za:b/c.cs is a different file"
        );
        let checked = format!("/proj{sep}a:b{sep}c.cs");
        let findings =
            parse_sarif(TOOL, &stdout, Some(1), &[&checked]).expect("drive relative uri");
        assert_eq!(findings[0].file, checked);
        let notes = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"C:notes/notes.md"},"region":{"startLine":1}}}]}"#,
        );
        let notes_file = format!("/proj{sep}C:notes{sep}notes.md");
        let findings = parse_sarif(TOOL, &notes, Some(1), &[&notes_file]).expect("drive relative");
        assert_eq!(findings[0].file, notes_file);
    }

    #[test]
    fn sarif_resolves_base_id_joins() {
        let stdout = br#"{"version":"2.1.0","runs":[{"originalUriBaseIds":{"%SRCROOT%":{"uri":"file:///home/u/"}},"tool":{"driver":{"name":"k"}},"results":[{"ruleId":"R","level":"error","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"../../tmp/x/Dirty.kt","uriBaseId":"%SRCROOT%"},"region":{"startLine":1,"startColumn":1}}}]}]}]}"#;
        let findings = parse_sarif(TOOL, stdout, Some(1), &["/tmp/x/Dirty.kt"]).expect("base join");
        assert_eq!(findings[0].file, "/tmp/x/Dirty.kt");
        let unknown_base = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"k"}},"results":[{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"a.java","uriBaseId":"%MISSING%"},"region":{"startLine":1}}}]}]}]}"#;
        assert!(parse_sarif(TOOL, unknown_base, Some(1), &["/tmp/x/Dirty.kt"]).is_err());
    }

    #[test]
    fn sarif_resolves_relative_and_level_defaults() {
        let stdout = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/a.java"},"region":{"startLine":1}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &stdout, Some(1), &["/scratch/src/a.java"])
            .expect("relative resolves");
        assert_eq!(findings[0].file, "/scratch/src/a.java");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (
                findings[0].finding.start.line,
                findings[0].finding.start.column
            ),
            (1, 1)
        );
        let note = log(
            r#"{"ruleId":"R","level":"note","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1,"startColumn":1}}}]}"#,
        );
        let findings = parse_sarif(TOOL, &note, Some(1), &["/s/a.java"]).expect("note");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        let bad_level = log(
            r#"{"ruleId":"R","level":"fatal","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1,"startColumn":1}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &bad_level, Some(1), &["/s/a.java"]).is_err());
        let unknown = log(
            r#"{"ruleId":"R","level":"warning","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/other.java"},"region":{"startLine":1,"startColumn":1}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &unknown, Some(1), &["/s/a.java"]).is_err());
        let empty_msg = log(
            r#"{"ruleId":"R","level":"warning","message":{"text":""},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1,"startColumn":1}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &empty_msg, Some(1), &["/s/a.java"]).is_err());
        let no_loc = log(r#"{"ruleId":"R","message":{"text":"m"},"locations":[]}"#);
        assert!(parse_sarif(TOOL, &no_loc, Some(1), &["/s/a.java"]).is_err());
        let bad_line = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":0,"startColumn":1}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &bad_line, Some(1), &["/s/a.java"]).is_err());
        for region in [
            r#"{"startLine":3,"endLine":2}"#,
            r#"{"startLine":3,"endLine":0}"#,
            r#"{"startLine":3,"endLine":4,"endColumn":0}"#,
            r#"{"startLine":3,"endColumn":-1}"#,
        ] {
            let bad = log(&format!(
                r#"{{"ruleId":"R","message":{{"text":"m"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"/s/a.java"}},"region":{region}}}}}]}}"#
            ));
            assert!(
                parse_sarif(TOOL, &bad, Some(1), &["/s/a.java"]).is_err(),
                "{region}"
            );
        }
    }

    fn one_region(region: &str) -> Finding {
        let stdout = log(&format!(
            r#"{{"ruleId":"R","message":{{"text":"m"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"/s/a.java"}},"region":{region}}}}}]}}"#
        ));
        let mut found = parse_sarif(TOOL, &stdout, Some(1), &["/s/a.java"]).expect("parsed");
        found.remove(0).finding
    }

    #[test]
    fn line_only_and_defaulted_ends_are_valid_regions() {
        const TEXT: &str = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let multiline = one_region(r#"{"startLine":3,"endLine":8}"#);
        assert_eq!(multiline.end.map(|end| end.line), Some(8));
        let placed = crate::place_finding(&multiline, "a.java", TEXT).expect("placed");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(4), Some(15)));
        let same_line = one_region(r#"{"startLine":3,"startColumn":1,"endColumn":2}"#);
        assert_eq!(same_line.end, Some(TextPosition { line: 3, column: 2 }));
        let placed = crate::place_finding(&same_line, "a.java", "ab\ncd\nef\n").expect("placed");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(6), Some(7)));
        let explicit = one_region(r#"{"startLine":3,"startColumn":1,"endLine":4,"endColumn":2}"#);
        assert_eq!(explicit.end, Some(TextPosition { line: 4, column: 2 }));
        let crlf =
            crate::place_finding(&multiline, "a.java", &TEXT.replace('\n', "\r\n")).expect("crlf");
        assert_eq!((crlf.start_byte, crlf.end_byte), (Some(6), Some(22)));
        let last = one_region(r#"{"startLine":2,"endLine":3}"#);
        let placed = crate::place_finding(&last, "a.java", "a\nb\nc").expect("no final newline");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(2), Some(5)));
        assert!(crate::place_finding(&multiline, "a.java", "a\nb\nc\n").is_err());
        let inverted = one_region(r#"{"startLine":3,"startColumn":5,"endColumn":2}"#);
        assert!(crate::place_finding(&inverted, "a.java", "ab\ncd\nefghij\n").is_err());
    }
}
