use quick_xml::encoding::Decoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;

use super::{check_output_size, code_name, known, missing, FileFinding, ParseError};
use crate::{Finding, TextPosition, ToolSeverity};

const TOOL: &str = "cppcheck";

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

struct OpenError {
    depth: usize,
    tag: String,
    id: String,
    message: String,
    severity: ToolSeverity,
    located: bool,
}

fn severity_of(word: &str, tag: &str) -> Result<ToolSeverity, ParseError> {
    match word {
        "error" => Ok(ToolSeverity::Error),
        "warning" | "style" | "performance" | "portability" => Ok(ToolSeverity::Warning),
        "information" => Ok(ToolSeverity::Info),
        _ => Err(missing(TOOL, "severity", tag)),
    }
}

fn open_error(
    element: &BytesStart<'_>,
    depth: usize,
    decoder: Decoder,
) -> Result<OpenError, ParseError> {
    let tag = String::from_utf8_lossy(element).into_owned();
    let id = required(element, "id", decoder)?;
    let severity_word = required(element, "severity", decoder)?;
    let severity = severity_of(&severity_word, &tag)?;
    let message = required(element, "msg", decoder)?;
    if message.is_empty() {
        return Err(missing(TOOL, "message", &tag));
    }
    Ok(OpenError {
        depth,
        tag,
        id,
        message,
        severity,
        located: false,
    })
}

fn location_finding(
    element: &BytesStart<'_>,
    open: &mut OpenError,
    files: &[&str],
    decoder: Decoder,
) -> Result<FileFinding, ParseError> {
    let path = required(element, "file", decoder)?;
    let checked = known(TOOL, files, &path)?;
    let line = required(element, "line", decoder)?
        .parse()
        .map_err(|_| missing(TOOL, "line", &open.tag))?;
    let column = match attribute(element, "column", decoder)? {
        Some(raw) => raw
            .parse()
            .map_err(|_| missing(TOOL, "column", &open.tag))?,
        None => 1,
    };
    open.located = true;
    let start = TextPosition { line, column };
    Ok(FileFinding {
        file: checked.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: std::mem::take(&mut open.id),
            message: std::mem::take(&mut open.message),
            severity: open.severity,
            start,
            end: Some(start),
            suggestions: Vec::new(),
        },
    })
}

fn take_location(
    element: &BytesStart<'_>,
    depth: usize,
    open: &mut Option<OpenError>,
    findings: &mut Vec<FileFinding>,
    files: &[&str],
    decoder: Decoder,
) -> Result<(), ParseError> {
    let Some(current) = open.as_mut() else {
        return Ok(());
    };
    if current.located || current.depth + 1 != depth {
        return Ok(());
    }
    findings.push(location_finding(element, current, files, decoder)?);
    Ok(())
}

/// cppcheck reports file-less diagnostics such as a missing include as `<symbol>` children, so only a severity that blocks a build must carry a location.
fn require_location(open: &OpenError) -> Result<(), ParseError> {
    if open.located || open.severity == ToolSeverity::Info {
        return Ok(());
    }
    Err(missing(TOOL, "location", &open.tag))
}

fn close_error(open: &mut Option<OpenError>) -> Result<(), ParseError> {
    match open.take() {
        Some(current) => require_location(&current),
        None => Ok(()),
    }
}

pub fn parse_cppcheck(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stderr)?;
    let text = std::str::from_utf8(stderr).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut reader = Reader::from_str(text);
    let decoder = reader.decoder();
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut open: Option<OpenError> = None;
    loop {
        let (element, empty) = match reader.read_event() {
            Ok(Event::Start(element)) => (element, false),
            Ok(Event::Empty(element)) => (element, true),
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if open.as_ref().is_some_and(|current| current.depth >= depth) {
                    close_error(&mut open)?;
                }
                continue;
            }
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(err) => {
                return Err(ParseError::Shape {
                    tool: TOOL,
                    detail: err.to_string(),
                });
            }
        };
        let name = element.local_name();
        elements += 1;
        if empty {
            if name.as_ref() == b"error" {
                require_location(&open_error(&element, depth, decoder)?)?;
            } else if name.as_ref() == b"location" {
                take_location(&element, depth, &mut open, &mut findings, files, decoder)?;
            }
            continue;
        }
        if name.as_ref() == b"error" && open.is_none() {
            open = Some(open_error(&element, depth, decoder)?);
        } else if name.as_ref() == b"location" {
            take_location(&element, depth, &mut open, &mut findings, files, decoder)?;
        }
        depth += 1;
    }
    close_error(&mut open)?;
    if elements == 0 && !text.trim().is_empty() {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: "no XML elements".to_owned(),
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with no error elements", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results version=\"2\">\n  <cppcheck version=\"2.21.0\"/>\n  <errors>\n    <error id=\"nullPointer\" severity=\"error\" msg=\"Possible null pointer dereference: slot\" verbose=\"Possible null pointer dereference: slot\">\n      <location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"8\" column=\"10\"/>\n    </error>\n  </errors>\n</results>\n";
    const CLEAN: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results version=\"2\">\n  <cppcheck version=\"2.21.0\"/>\n  <errors>\n  </errors>\n</results>\n";

    #[test]
    fn cppcheck_reports_xml_errors() {
        let findings = parse_cppcheck(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/cppcheck/Sample.c");
        assert_eq!(findings[0].finding.rule_id, "nullPointer");
        assert_eq!(
            findings[0].finding.message,
            "Possible null pointer dereference: slot"
        );
        let clean = parse_cppcheck(
            CLEAN.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_cppcheck(
            CLEAN.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/cppcheck/Sample.c"]
        )
        .is_err());
        assert!(parse_cppcheck(b"not xml", Some(1), &["x"]).is_err());
        assert!(parse_cppcheck(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_cppcheck(b"<error ", Some(1), &["x"]).is_err());
        assert!(parse_cppcheck(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn attribute_values_decode_every_xml_escape() {
        let xml = "<results version=\"2\"><errors><error id=\"a&amp;b\" severity=\"information\" msg=\"x &lt; y &gt; z &quot;q&quot; &apos;p&apos; &amp; r\">\
            <location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"1\" column=\"2\"/>\
            </error></errors></results>";
        let findings = parse_cppcheck(
            xml.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "a&b");
        assert_eq!(findings[0].finding.message, "x < y > z \"q\" 'p' & r");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
    }

    #[test]
    fn a_commented_error_element_is_not_a_finding() {
        let xml = "<results version=\"2\"><errors><!-- <error id=\"ghost\" severity=\"error\" msg=\"g\"><location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"1\"/></error> --></errors></results>";
        let findings = parse_cppcheck(
            xml.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_file_less_information_error_does_not_hide_the_next_error() {
        let xml = "<results version=\"2\"><errors>\
            <error id=\"missingInclude\" severity=\"information\" msg=\"No such file or directory\"><symbol>main</symbol></error>\
            <error id=\"nullPointer\" severity=\"error\" msg=\"slot\"><location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"7\" column=\"10\"/></error>\
            </errors></results>";
        let findings = parse_cppcheck(
            xml.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].finding.rule_id, "nullPointer");
        assert_eq!(findings[0].finding.start.line, 7);
    }

    #[test]
    fn a_blocking_error_without_a_location_is_rejected() {
        for xml in [
            "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><symbol>main</symbol></error></errors></results>",
            "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"/></errors></results>",
            "<results version=\"2\"><errors><error id=\"a\" severity=\"style\" msg=\"m\"/></errors></results>",
        ] {
            let error = parse_cppcheck(xml.as_bytes(), Some(0), &["x"]).expect_err(xml);
            assert!(error.to_string().contains("malformed location"), "{xml}: {error}");
        }
        let informational =
            "<results version=\"2\"><errors><error id=\"a\" severity=\"information\" msg=\"m\"/></errors></results>";
        assert!(parse_cppcheck(informational.as_bytes(), Some(0), &["x"])
            .expect("information needs no location")
            .is_empty());
    }

    #[test]
    fn only_the_first_location_of_an_error_is_a_finding() {
        let xml = "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\">\
            <location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"1\"/>\
            <location file=\"cc/tests/fixtures/cppcheck/Sample.c\" line=\"2\"/>\
            </error></errors></results>";
        let findings = parse_cppcheck(
            xml.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].finding.start.line, 1);
    }

    #[test]
    fn attributes_may_be_single_quoted_or_split_across_lines() {
        let xml = "<results version=\"2\"><errors><error id='a' severity='style'\n msg='m'>\n<location\n file='cc/tests/fixtures/cppcheck/Sample.c'\n line='3'/>\n</error></errors></results>";
        let findings = parse_cppcheck(
            xml.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/cppcheck/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[0].finding.start.column, 1);
    }

    #[test]
    fn empty_output_is_clean_and_text_without_elements_is_not() {
        assert!(parse_cppcheck(b"", Some(0), &["x"])
            .expect("cppcheck writes nothing when clean")
            .is_empty());
        assert!(parse_cppcheck(b"   \n", Some(0), &["x"])
            .expect("whitespace is clean")
            .is_empty());
        for text in ["garbage", "<results", "not xml at all"] {
            assert!(
                parse_cppcheck(text.as_bytes(), Some(0), &["x"]).is_err(),
                "{text:?}"
            );
        }
    }

    #[test]
    fn malformed_shapes_report_the_offending_attribute() {
        for (xml, needle) in [
            (
                "<results version=\"2\"><errors><error severity=\"error\" msg=\"m\"><location file=\"x\" line=\"1\"/></error></errors></results>",
                "malformed id",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" msg=\"m\"><location file=\"x\" line=\"1\"/></error></errors></results>",
                "malformed severity",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" severity=\"nope\" msg=\"m\"><location file=\"x\" line=\"1\"/></error></errors></results>",
                "malformed severity",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"\"><location file=\"x\" line=\"1\"/></error></errors></results>",
                "malformed message",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><location file=\"x\" line=\"z\"/></error></errors></results>",
                "malformed line",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><location file=\"x\" line=\"1\" column=\"z\"/></error></errors></results>",
                "malformed column",
            ),
            (
                "<results version=\"2\"><errors><error id=\"a\" severity=\"error\" msg=\"m\"><location line=\"1\"/></error></errors></results>",
                "malformed file",
            ),
        ] {
            let error = parse_cppcheck(xml.as_bytes(), Some(0), &["x"]).expect_err(xml);
            assert!(
                error.to_string().contains(needle),
                "{xml} gave {error}"
            );
        }
    }
}
