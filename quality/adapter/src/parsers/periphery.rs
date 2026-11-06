use super::{FileFinding, ParseError, as_text, code_name, point};
use crate::{ColumnEncoding, Finding, ToolSeverity};

const TOOL: &str = "periphery";

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| ParseError::shape(TOOL, format!("missing {key}")))
}

/// `path:line:column`, split from the right so a path may hold colons.
fn location(location: &str) -> Result<(&str, u64, u64), ParseError> {
    let bad = || ParseError::shape(TOOL, format!("malformed location: {location}"));
    let (rest, column) = location.rsplit_once(':').ok_or_else(bad)?;
    let (path, line) = rest.rsplit_once(':').ok_or_else(bad)?;
    let line = line.parse::<u64>().map_err(|_| bad())?;
    let column = column.parse::<u64>().map_err(|_| bad())?;
    Ok((path, line.max(1), column.max(1)))
}

/// Parses `periphery scan --format json`: stdout is one JSON array of unused declarations.
///
/// Periphery scans the whole index store and names files by absolute path, so a result counts
/// for a checked file when its path ends with that file; results in other files are dropped.
/// Every result is an info finding named by its kind, such as `function.free`.
pub fn parse_periphery(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} (want 0): {}",
                code_name(code),
                String::from_utf8_lossy(stderr).trim()
            ),
        ));
    }
    let report = as_text(TOOL, stdout)?.trim();
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(report).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: format!("stdout must be one JSON array ({err})"),
        })?;
    let mut findings = Vec::new();
    for entry in &entries {
        let kind = text(entry, "kind")?;
        let name = text(entry, "name")?;
        let (path, line, column) = location(text(entry, "location")?)?;
        let Some(file) = files.iter().copied().find(|file| {
            path == *file
                || path
                    .strip_suffix(file)
                    .is_some_and(|head| head.ends_with('/'))
        }) else {
            continue;
        };
        let (start, end) = point(line, column);
        findings.push(FileFinding {
            file: file.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: kind.to_owned(),
                message: format!("{name} is unused"),
                severity: ToolSeverity::Info,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = r#"[{"kind":"function.free","name":"unused()","location":"/work/pkg/Sources/a.swift:3:6","hints":["unused"],"accessibility":"internal","modifiers":[],"attributes":[],"ids":["s:1a6unusedyyF"]}]"#;

    #[test]
    fn reports_an_unused_function_as_info() {
        let found =
            parse_periphery(ONE.as_bytes(), b"", Some(0), &["Sources/a.swift"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "Sources/a.swift");
        assert_eq!(found[0].finding.rule_id, "function.free");
        assert_eq!(found[0].finding.message, "unused() is unused");
        assert_eq!(found[0].finding.severity, ToolSeverity::Info);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 6);
    }

    #[test]
    fn drops_files_that_were_not_checked() {
        let found =
            parse_periphery(ONE.as_bytes(), b"", Some(0), &["Sources/b.swift"]).expect("parsed");
        assert!(found.is_empty());
        let near =
            parse_periphery(ONE.as_bytes(), b"", Some(0), &["ources/a.swift"]).expect("parsed");
        assert!(near.is_empty());
    }

    #[test]
    fn clean_run_is_clean() {
        let found = parse_periphery(b"[]\n", b"", Some(0), &["a.swift"]).expect("parsed");
        assert!(found.is_empty());
    }

    #[test]
    fn rejects_bad_exit_and_bad_shapes() {
        let failed = parse_periphery(b"", b"boom\n", Some(1), &[]).expect_err("exit");
        assert!(failed.to_string().contains("exit 1"));
        assert!(parse_periphery(b"not json", b"", Some(0), &[]).is_err());
        assert!(
            parse_periphery(
                br#"[{"kind":"x","name":"y","location":"a.swift"}]"#,
                b"",
                Some(0),
                &[]
            )
            .is_err()
        );
        assert!(
            parse_periphery(
                br#"[{"name":"y","location":"a.swift:1:1"}]"#,
                b"",
                Some(0),
                &[]
            )
            .is_err()
        );
    }
}
