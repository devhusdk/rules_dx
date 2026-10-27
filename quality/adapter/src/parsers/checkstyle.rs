use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;

use super::{check_output_size, sarif::parse_sarif, FileFinding, ParseError};
use crate::ColumnEncoding;

const DEFAULT_TAB_WIDTH: u32 = 8;

fn property_value(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Option<String> {
    let mut name = None;
    let mut value = None;
    let mut default = None;
    for attr in element.attributes().flatten() {
        let text = attr
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .ok()?
            .into_owned();
        match attr.key.as_ref() {
            b"name" => name = Some(text),
            b"value" => value = Some(text),
            b"default" => default = Some(text),
            _ => {}
        }
    }
    if name.as_deref() != Some("tabWidth") {
        return None;
    }
    let value = value?;
    if value.contains("${") {
        default
    } else {
        Some(value)
    }
}

/// Reads `tabWidth` from the top-level `Checker` module of a Checkstyle
/// config. Returns 8 (Checkstyle's default) when absent, unparsable or 0.
pub fn checkstyle_tab_width(config: &[u8]) -> u32 {
    let mut reader = Reader::from_reader(config);
    let decoder = reader.decoder();
    // Open elements, innermost last; `true` marks a `<module>`.
    let mut stack: Vec<bool> = Vec::new();
    let mut buf = Vec::new();
    loop {
        let event = match reader.read_event_into(&mut buf) {
            Ok(event) => event,
            Err(_) => return DEFAULT_TAB_WIDTH,
        };
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let is_module = e.name().as_ref() == b"module";
                let directly_in_checker =
                    stack.iter().filter(|m| **m).count() == 1 && stack.last() == Some(&true);
                if e.name().as_ref() == b"property" && directly_in_checker {
                    if let Some(text) = property_value(e, decoder) {
                        return match text.trim().parse::<u32>() {
                            Ok(0) | Err(_) => DEFAULT_TAB_WIDTH,
                            Ok(width) => width,
                        };
                    }
                }
                if matches!(event, Event::Start(_)) {
                    stack.push(is_module);
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => return DEFAULT_TAB_WIDTH,
            _ => {}
        }
        buf.clear();
    }
}

pub fn parse_checkstyle(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    tab_width: u32,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("checkstyle", stdout)?;
    let mut findings = parse_sarif("checkstyle", stdout, code, files, ColumnEncoding::Scalars)?;
    for found in &mut findings {
        if found.finding.encoding == ColumnEncoding::Scalars {
            found.finding.encoding = ColumnEncoding::ScalarsTabs { width: tab_width };
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"Checkstyle"}},"results":[{"ruleId":"com.puppycrawl.tools.checkstyle.checks.imports.UnusedImportsCheck","level":"error","message":{"text":"Unused import - java.util.ArrayList."},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"file:/s/Dirty.java"},"region":{"startLine":2,"startColumn":8}}}]}]}]}"#;

    #[test]
    fn checkstyle_reports_sarif_points() {
        let findings =
            parse_checkstyle(SARIF.as_bytes(), Some(1), &["/s/Dirty.java"], 8).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].finding.rule_id,
            "com.puppycrawl.tools.checkstyle.checks.imports.UnusedImportsCheck"
        );
        assert!(findings[0].finding.end.is_none());
        let clean_json = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"Checkstyle"}},"results":[]}]}"#;
        let clean =
            parse_checkstyle(clean_json.as_bytes(), Some(0), &["/s/Dirty.java"], 8).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_checkstyle(clean_json.as_bytes(), Some(1), &["/s/Dirty.java"], 8).is_err());
        assert!(parse_checkstyle(b"not json", Some(1), &["/s/Dirty.java"], 8).is_err());
    }

    #[test]
    fn checkstyle_tab_columns_place_on_the_token() {
        let text = "\t\treturn 42;\n";
        let sarif = |col: u32| {
            format!(
                r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"Checkstyle"}}}},"results":[{{"ruleId":"MagicNumber","level":"error","message":{{"text":"magic"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"file:/s/T.java"}},"region":{{"startLine":1,"startColumn":{col}}}}}}}]}}]}}]}}"#
            )
        };
        for (col, width) in [(24, 8), (16, 4)] {
            let found = parse_checkstyle(sarif(col).as_bytes(), Some(1), &["/s/T.java"], width)
                .expect("parsed");
            let diagnostic =
                crate::place_finding(&found[0].finding, "T.java", text).expect("placed");
            let start = diagnostic.start_byte.expect("start") as usize;
            assert_eq!(&text[start..start + 2], "42");
        }
        let mid =
            parse_checkstyle(sarif(5).as_bytes(), Some(1), &["/s/T.java"], 8).expect("parsed");
        assert!(matches!(
            crate::place_finding(&mid[0].finding, "T.java", text),
            Err(crate::PlaceError::UnmappablePosition { .. })
        ));
    }

    #[test]
    fn checkstyle_tab_width_reads_checker_property() {
        let cfg = |inner: &str| {
            format!("<?xml version=\"1.0\"?><module name=\"Checker\">{inner}</module>")
        };
        let width = |inner: &str| checkstyle_tab_width(cfg(inner).as_bytes());
        assert_eq!(width(r#"<property name="tabWidth" value="4"/>"#), 4);
        assert_eq!(
            width(r#"<property name="tabWidth" value="${x}" default="2"/>"#),
            2
        );
        assert_eq!(
            width(r#"<module name="TreeWalker"><property name="tabWidth" value="3"/></module>"#),
            8
        );
        assert_eq!(
            width(
                r#"<module name="TreeWalker"><property name="tabWidth" value="3"/></module><property name="tabWidth" value="5"/>"#
            ),
            5
        );
        assert_eq!(width(""), 8);
        assert_eq!(width(r#"<property name="tabWidth" value="abc"/>"#), 8);
        assert_eq!(width(r#"<property name="tabWidth" value="0"/>"#), 8);
        assert_eq!(
            width(r#"<property name="tabWidth" value="99999999999"/>"#),
            8
        );
        assert_eq!(checkstyle_tab_width(b"<module name=\"Checker\"><"), 8);
        assert_eq!(checkstyle_tab_width(b""), 8);
    }
}
