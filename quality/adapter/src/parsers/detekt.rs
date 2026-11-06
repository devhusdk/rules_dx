use quick_xml::XmlVersion;
use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::{FileFinding, ParseError, check_output_size, code_name, missing};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "detekt";

fn attribute(
    element: &BytesStart<'_>,
    name: &str,
    decoder: Decoder,
) -> Result<Option<String>, ParseError> {
    for candidate in element.attributes() {
        let candidate = candidate.map_err(|err| ParseError::Shape {
            tool: TOOL,
            detail: format!("unreadable {name} attribute: {err}"),
        })?;
        if candidate.key.as_ref() == name.as_bytes() {
            return candidate
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map(|value| Some(value.into_owned()))
                .map_err(|err| ParseError::Shape {
                    tool: TOOL,
                    detail: format!("unreadable {name} value: {err}"),
                });
        }
    }
    Ok(None)
}

fn required(element: &BytesStart<'_>, name: &str, decoder: Decoder) -> Result<String, ParseError> {
    attribute(element, name, decoder)?
        .ok_or_else(|| missing(TOOL, name, &String::from_utf8_lossy(element)))
}

fn severity_of(word: &str, tag: &str) -> Result<ToolSeverity, ParseError> {
    match word {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        "info" => Ok(ToolSeverity::Info),
        _ => Err(missing(TOOL, "severity", tag)),
    }
}

/// detekt names files by absolute path; the checked file is the one the path ends with.
fn checked<'a>(files: &[&'a str], reported: &str) -> Result<&'a str, ParseError> {
    let reported = reported.replace('\\', "/");
    let found = files
        .iter()
        .copied()
        .filter(|file| {
            let file = file.replace('\\', "/");
            reported == file || reported.ends_with(&format!("/{}", file.trim_start_matches("./")))
        })
        .max_by_key(|file| file.len());
    match found {
        Some(file) => Ok(file),
        None => Err(ParseError::UnknownFile {
            tool: TOOL,
            path: reported,
        }),
    }
}

fn rule_of(source: &str) -> &str {
    source.strip_prefix("detekt.").unwrap_or(source)
}

fn finding(
    element: &BytesStart<'_>,
    file: &str,
    decoder: Decoder,
) -> Result<FileFinding, ParseError> {
    let tag = String::from_utf8_lossy(element).into_owned();
    let line = required(element, "line", decoder)?
        .parse()
        .map_err(|_| missing(TOOL, "line", &tag))?;
    let column = required(element, "column", decoder)?
        .parse()
        .map_err(|_| missing(TOOL, "column", &tag))?;
    let severity = severity_of(&required(element, "severity", decoder)?, &tag)?;
    let message = required(element, "message", decoder)?;
    let source = required(element, "source", decoder)?;
    let start = TextPosition { line, column };
    Ok(FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule_of(&source).to_owned(),
            message,
            severity,
            start,
            end: Some(start),
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Utf16,
            related: Vec::new(),
        },
    })
}

/// Parses the checkstyle-format report written by `--report xml:<path>`.
///
/// Exit 2 is detekt's "issues found"; any other non-zero exit is a failed run.
pub fn parse_detekt(
    report: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 2)) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {}", code_name(code)),
        });
    }
    check_output_size(TOOL, report)?;
    let text = std::str::from_utf8(report).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut reader = Reader::from_str(text);
    let decoder = reader.decoder();
    let mut findings = Vec::new();
    let mut rooted = false;
    let mut current: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                match element.local_name().as_ref() {
                    b"checkstyle" => rooted = true,
                    b"file" => {
                        let name = required(&element, "name", decoder)?;
                        current = Some(checked(files, &name)?.to_owned());
                    }
                    b"error" => {
                        let Some(file) = current.as_deref() else {
                            return Err(missing(TOOL, "file", &String::from_utf8_lossy(&element)));
                        };
                        findings.push(finding(&element, file, decoder)?);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"file" => current = None,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: err.to_string(),
                });
            }
        }
    }
    if !rooted {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "no <checkstyle> root".to_owned(),
        });
    }
    if findings.is_empty() && code == Some(2) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "exit 2 with no error elements".to_owned(),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<checkstyle version=\"4.3\">\n<file name=\"/scratch/src/A.kt\">\n\t<error line=\"3\" column=\"13\" severity=\"info\" message=\"Private function `unused` is unused.\" source=\"detekt.UnusedPrivateMember\" />\n</file>\n</checkstyle>\n";
    const CLEAN: &str =
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<checkstyle version=\"4.3\">\n</checkstyle>\n";

    #[test]
    fn detekt_reports_info_findings_for_the_checked_file() {
        let findings = parse_detekt(DIRTY.as_bytes(), Some(2), &["src/A.kt"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/A.kt");
        assert_eq!(findings[0].finding.rule_id, "UnusedPrivateMember");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert_eq!(
            findings[0].finding.start,
            TextPosition {
                line: 3,
                column: 13
            }
        );
    }

    #[test]
    fn detekt_clean_run_has_no_findings() {
        let findings = parse_detekt(CLEAN.as_bytes(), Some(0), &["src/A.kt"]).expect("parsed");
        assert!(findings.is_empty());
    }

    #[test]
    fn detekt_rejects_failed_runs_and_unknown_files() {
        assert!(parse_detekt(CLEAN.as_bytes(), Some(2), &["src/A.kt"]).is_err());
        assert!(parse_detekt(CLEAN.as_bytes(), Some(3), &["src/A.kt"]).is_err());
        assert!(parse_detekt(CLEAN.as_bytes(), None, &["src/A.kt"]).is_err());
        assert!(parse_detekt(DIRTY.as_bytes(), Some(2), &["other/B.kt"]).is_err());
        assert!(parse_detekt(b"not xml", Some(0), &["src/A.kt"]).is_err());
        assert!(parse_detekt(b"<checkstyle><file ", Some(0), &["x"]).is_err());
    }
}
