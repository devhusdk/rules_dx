use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

use serde_sarif::sarif::{ArtifactLocation, Region, ResultLevel, Sarif};

use super::{check_output_size, code_name, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

fn sarif_severity(level: Option<ResultLevel>) -> ToolSeverity {
    match level {
        None | Some(ResultLevel::Warning) => ToolSeverity::Warning,
        Some(ResultLevel::Error) => ToolSeverity::Error,
        Some(ResultLevel::Note) | Some(ResultLevel::None) => ToolSeverity::Info,
    }
}

fn strip_file_uri(uri: &str) -> &str {
    uri.strip_prefix("file://")
        .or_else(|| uri.strip_prefix("file:"))
        .unwrap_or(uri)
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
    if rooted(path) {
        path.to_owned()
    } else {
        format!("{MAIN_SEPARATOR_STR}{path}")
    }
}

fn native(path: &str) -> String {
    path.replace('/', MAIN_SEPARATOR_STR)
}

fn percent_decode(path: &str) -> Option<String> {
    if !path.contains('%') {
        return None;
    }
    let bytes = path.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            out.push(u8::from_str_radix(pair, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
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

fn resolve_file<'a>(
    tool: &'static str,
    files: &[&'a str],
    uri: &str,
    base_id: Option<&str>,
    bases: &std::collections::BTreeMap<String, ArtifactLocation>,
) -> Result<&'a str, ParseError> {
    let mut joined = String::new();
    if let Some(id) = base_id {
        let base = bases.get(id).ok_or_else(|| ParseError::Shape {
            tool,
            detail: format!("unknown uriBaseId: {id}"),
        })?;
        let base_path = strip_file_uri(base.uri.as_deref().unwrap_or_default());
        joined.push_str(base_path.strip_suffix('/').unwrap_or(base_path));
        joined.push('/');
        joined.push_str(strip_file_uri(uri));
    } else {
        joined.push_str(strip_file_uri(uri));
    }
    let normalized = normalize_path(&joined);
    if let Some(hit) = match_spelling(files, &normalized) {
        return Ok(hit);
    }
    if rooted(&normalized) {
        let raw = strip_file_uri(uri);
        let raw_suffix = suffix_for(&native(raw));
        if let Some(hit) = unique_suffix(files, &raw_suffix) {
            return Ok(hit);
        }
    }
    if let Some(decoded) = percent_decode(&joined) {
        let decoded = normalize_path(&decoded);
        if decoded != normalized {
            if let Some(hit) = match_spelling(files, &decoded) {
                return Ok(hit);
            }
        }
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

fn end(tool: &'static str, region: &Region) -> Result<Option<TextPosition>, ParseError> {
    match (region.end_line, region.end_column) {
        (Some(line), Some(column)) => {
            if line < 1 || column < 1 {
                return Err(ParseError::Shape {
                    tool,
                    detail: format!("bad end position {line}:{column}"),
                });
            }
            Ok(Some(TextPosition {
                line: line as u64,
                column: column as u64,
            }))
        }
        (None, None) => Ok(None),
        _ => Err(ParseError::Shape {
            tool,
            detail: "partial end position".to_owned(),
        }),
    }
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
                let end = end(tool, region)?;
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
        assert_eq!(suffix_for("C:/src/a.cs"), native("C:/src/a.cs"));
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
        assert_eq!(
            percent_decode("src/my%20file.cs").as_deref(),
            Some("src/my file.cs")
        );
        assert_eq!(
            percent_decode("src/caf%C3%A9.cs").as_deref(),
            Some("src/café.cs")
        );
        assert_eq!(
            percent_decode("src/a%2Fb.cs").as_deref(),
            Some("src/a/b.cs")
        );
        for raw in [
            "src/a.cs",
            "src/100%/a.cs",
            "src/a%.cs",
            "src/a%2.cs",
            "src/a%zz.cs",
        ] {
            assert_eq!(percent_decode(raw), None, "raw: {raw:?}");
        }
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
            findings[0].file, literal,
            "the raw spelling names the file that holds it"
        );
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
        let partial_end = log(
            r#"{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/a.java"},"region":{"startLine":1,"endLine":2}}}]}"#,
        );
        assert!(parse_sarif(TOOL, &partial_end, Some(1), &["/s/a.java"]).is_err());
    }
}
