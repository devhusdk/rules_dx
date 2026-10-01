use super::junit_types::{JunitCase, JunitMessage};
use super::ReportError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesStart;
use std::borrow::Cow;

fn junit_error(detail: impl Into<String>) -> ReportError {
    ReportError::InvalidJunit {
        detail: detail.into(),
    }
}

pub fn parse_test_xml(
    bytes: &[u8],
    shard: u32,
    attempt: u32,
) -> Result<Vec<JunitCase>, ReportError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| junit_error(format!("test XML is not UTF-8: {e}")))?;
    if !text.contains('<') {
        return Err(junit_error("test XML has no elements"));
    }
    let report = deserialize_report(text)?;

    let mut cases: Vec<JunitCase> = Vec::new();
    for suite in &report.test_suites {
        for case in &suite.test_cases {
            if case.name.as_str().is_empty() {
                return Err(junit_error("testcase without name"));
            }
            let (failure, error, skipped) = match &case.status {
                quick_junit::TestCaseStatus::Success { .. } => (None, None, None),
                quick_junit::TestCaseStatus::NonSuccess {
                    kind,
                    message,
                    description,
                    ..
                } => {
                    let note = JunitMessage {
                        message: message.as_ref().map(|m| m.as_str().to_owned()),
                        text: description
                            .as_ref()
                            .map(|d| d.as_str().to_owned())
                            .unwrap_or_default(),
                    };
                    match kind {
                        quick_junit::NonSuccessKind::Failure => (Some(note), None, None),
                        quick_junit::NonSuccessKind::Error => (None, Some(note), None),
                    }
                }
                quick_junit::TestCaseStatus::Skipped {
                    message,
                    description,
                    ..
                } => (
                    None,
                    None,
                    Some(JunitMessage {
                        message: message.as_ref().map(|m| m.as_str().to_owned()),
                        text: description
                            .as_ref()
                            .map(|d| d.as_str().to_owned())
                            .unwrap_or_default(),
                    }),
                ),
            };
            cases.push(JunitCase {
                name: case.name.as_str().to_owned(),
                classname: case.classname.as_ref().map(|c| c.as_str().to_owned()),
                time: case.time.map(|d| d.as_secs_f64()).unwrap_or(0.0),
                failure,
                error,
                skipped,
                system_out: case.system_out.as_ref().map(|s| s.as_str().to_owned()),
                system_err: case.system_err.as_ref().map(|s| s.as_str().to_owned()),
                shard,
                attempt,
            });
        }
    }
    Ok(cases)
}

fn deserialize_report(text: &str) -> Result<quick_junit::Report, ReportError> {
    let normalized = normalize_document(text)
        .replace("<testsuite>", "<testsuite name=\"dx\">")
        .replace("<testsuite/>", "<testsuite name=\"dx\"/>");
    match quick_junit::Report::deserialize_from_str(&normalized) {
        Ok(report) => {
            if report.test_suites.is_empty() && normalized.contains("<testcase") {
                let wrapped = format!(
                    "<testsuites><testsuite name=\"dx\">{normalized}</testsuite></testsuites>"
                );
                quick_junit::Report::deserialize_from_str(&wrapped)
                    .map_err(|e| junit_error(format!("malformed test XML: {e}")))
            } else {
                Ok(report)
            }
        }
        Err(first) => {
            let msg = first.to_string();
            if msg.contains("testsuites") {
                let inner = strip_leading_decl(&normalized);
                let wrapped = format!("<testsuites>{inner}</testsuites>");
                quick_junit::Report::deserialize_from_str(&wrapped)
                    .map_err(|e| junit_error(format!("malformed test XML: {e}")))
            } else {
                Err(junit_error(format!("malformed test XML: {first}")))
            }
        }
    }
}

fn strip_leading_decl(text: &str) -> &str {
    let trimmed = text.trim_start();
    if trimmed.starts_with("<?xml") {
        if let Some(end) = trimmed.find("?>") {
            return trimmed[end + 2..].trim_start();
        }
    }
    text
}

/// Drops `timestamp` attributes and clamps negative `time` values, failing open on bad XML.
fn normalize_document(text: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;
    use quick_xml::writer::Writer;
    use std::io::Cursor;

    if !text.contains("timestamp") && !text.contains("=\"-") && !text.contains("='-") {
        return text.to_owned();
    }

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text_start = false;
    reader.config_mut().trim_text_end = false;
    reader.config_mut().expand_empty_elements = false;

    let mut writer = Writer::new(Cursor::new(Vec::with_capacity(text.len())));
    let mut changed = false;

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match rewrite_attributes(&e) {
                Ok(Some(elem)) => {
                    changed = true;
                    if writer.write_event(Event::Start(elem)).is_err() {
                        return text.to_owned();
                    }
                }
                Ok(None) => {
                    if writer.write_event(Event::Start(e)).is_err() {
                        return text.to_owned();
                    }
                }
                Err(()) => return text.to_owned(),
            },
            Ok(Event::Empty(e)) => match rewrite_attributes(&e) {
                Ok(Some(elem)) => {
                    changed = true;
                    if writer.write_event(Event::Empty(elem)).is_err() {
                        return text.to_owned();
                    }
                }
                Ok(None) => {
                    if writer.write_event(Event::Empty(e)).is_err() {
                        return text.to_owned();
                    }
                }
                Err(()) => return text.to_owned(),
            },
            Ok(event) => {
                if writer.write_event(event).is_err() {
                    return text.to_owned();
                }
            }
            Err(_) => return text.to_owned(),
        }
    }

    if !changed {
        return text.to_owned();
    }
    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

/// Rebuilds a tag without its `timestamp` attributes and with negative `time` values clamped.
fn rewrite_attributes<'a>(e: &'a BytesStart<'_>) -> Result<Option<BytesStart<'static>>, ()> {
    let mut kept: Vec<Attribute<'a>> = Vec::new();
    let mut changed = false;
    for attr in e.attributes() {
        let Attribute { key, value } = attr.map_err(|_| ())?;
        if key.as_ref() == b"timestamp" {
            changed = true;
        } else if key.as_ref() == b"time" && value.starts_with(b"-") {
            changed = true;
            kept.push(Attribute {
                key,
                value: Cow::Borrowed(&b"0"[..]),
            });
        } else {
            kept.push(Attribute {
                key,
                value: escape_inner_quotes(value),
            });
        }
    }
    if !changed {
        return Ok(None);
    }
    let name = e.name();
    let name = std::str::from_utf8(name.as_ref()).map_err(|_| ())?;
    let mut elem = BytesStart::new(name).into_owned();
    for attr in kept {
        elem.push_attribute(attr);
    }
    Ok(Some(elem))
}

/// Attribute values keep their raw escaping, so only the quote that delimits them needs care.
fn escape_inner_quotes(value: Cow<'_, [u8]>) -> Cow<'_, [u8]> {
    if !value.contains(&b'"') {
        return value;
    }
    let mut out = Vec::with_capacity(value.len());
    let mut rest = value.as_ref();
    while let Some(at) = rest.iter().position(|b| *b == b'"') {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(b"&quot;");
        rest = &rest[at + 1..];
    }
    out.extend_from_slice(rest);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_document_keeps_unrelated_and_malformed_input() {
        for input in [
            "testcase timestamp",
            "testcase timestamp=",
            "testcase timestamp=unquoted",
            "testcase timestamp='unterminated",
            "testcase note='timestamp'",
            "testcase timestamp >",
            "testcase timestamp /",
            "testcase timestamp ?",
            "testcase = timestamp",
            "testcase timestamp   ",
            "<short",
            "<testcase name=\"a\" timestamp",
            "<testcase name=\"a\" timestamp='unterminated",
            "<testcase name=\"a\" time='-1'",
        ] {
            assert_eq!(normalize_document(input), input, "{input}");
        }
        assert_eq!(
            normalize_document("<testcase timestamp='ignored' name='kept'/>"),
            r#"<testcase name="kept"/>"#
        );
    }

    #[test]
    fn negative_duration_normalization_preserves_results() {
        let cases = parse_test_xml(
            b"<testsuite><testcase name=\"standalone\" time=\"-2\"/></testsuite>",
            2,
            3,
        )
        .expect("testcase");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].name, "standalone");
        for (input, expected) in [
            (
                "<testcase runtime=\"-1\" time = \"-2\"/>",
                "<testcase runtime=\"-1\" time=\"0\"/>",
            ),
            ("<testcase time='-1'/>", "<testcase time=\"0\"/>"),
            ("<testcase time=\"-1>", "<testcase time=\"-1>"),
            (
                "<testcase time other=\"x\"/>",
                "<testcase time other=\"x\"/>",
            ),
        ] {
            assert_eq!(normalize_document(input), expected, "{input}");
        }
        assert_eq!(
            strip_leading_decl("<?xml unterminated"),
            "<?xml unterminated"
        );
        for bad in [
            "<testsuite timestamp=\"unterminated",
            "<testsuite timestamp=noquote>",
            "<testsuite timestamp=\"x\" broken>",
        ] {
            assert!(parse_test_xml(bad.as_bytes(), 0, 0).is_err(), "{bad}");
        }
    }

    #[test]
    fn junit_parse_covers_happy_and_error_paths() {
        let good = r#"<?xml version="1.0"?><!-- c --><testsuite><testcase name="a" classname="c" time="1.5"><failure message="m">text</failure></testcase><testcase name="b"/><testcase name="c" time="0"><error/><system-out/><system-err/></testcase></testsuite>"#;
        let cases = parse_test_xml(good.as_bytes(), 0, 0).expect("good");
        assert_eq!(cases.len(), 3);
        let nested = r#"<testsuite><testcase name="a"><failure>text <b>bold</b> moretail</failure></testcase><testcase name="b"><error><![CDATA[blob]]></error></testcase></testsuite>"#;
        let cases = parse_test_xml(nested.as_bytes(), 0, 0).expect("nested");
        assert_eq!(cases.len(), 2);
        assert!(cases[0]
            .failure
            .as_ref()
            .expect("failure")
            .text
            .contains("text"));
        assert!(parse_test_xml(&[0xff], 0, 0).is_err());
        assert!(parse_test_xml(b"hello", 0, 0).is_err());
        assert!(parse_test_xml(b"<testcase", 0, 0).is_err());
        assert!(parse_test_xml(b"<testsuite><testcase/></testsuite>", 0, 0).is_err());
        assert!(parse_test_xml(b"<testsuite><testcase name=\"\"/></testsuite>", 0, 0).is_err());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\" time=\"bogus\"/></testsuite>",
            0,
            0
        )
        .is_err());
        let negative = parse_test_xml(
            b"<testsuite><testcase name=\"a\" time=\"-1\"/></testsuite>",
            0,
            0,
        )
        .expect("negative time clamps");
        assert_eq!(negative.len(), 1);
        assert_eq!(negative[0].time, 0.0);
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\" time=\"inf\"/></testsuite>",
            0,
            0
        )
        .is_err());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"><failure/><failure/></testcase></testsuite>",
            0,
            0
        )
        .is_err());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"><error/><error/></testcase></testsuite>",
            0,
            0
        )
        .is_err());
        assert!(parse_test_xml(b"<testsuite><testcase name=\"a\">", 0, 0).is_err());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"><failure message=\"\xff\"/></testcase></testsuite>",
            0,
            0
        )
        .is_err());
        assert!(
            parse_test_xml(
                b"<testsuite><testcase name=\"a\"><failure message=\"m\">ok</failure></testcase></testsuite>",
                0, 0
            )
            .is_ok()
        );
    }

    #[test]
    fn junit_parse_rejects_multiple_main_statuses() {
        let xml = b"<testsuite><testcase name=\"a\"><error/><skipped/></testcase></testsuite>";
        assert!(parse_test_xml(xml, 0, 0).is_err());
    }

    #[test]
    fn junit_parse_covers_start_and_end_branches() {
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"\"></testcase></testsuite>",
            0,
            0
        )
        .is_err());
        for bad in ["bogus", "inf", "NaN"] {
            let xml =
                format!("<testsuite><testcase name=\"a\" time=\"{bad}\"></testcase></testsuite>");
            assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_err(), "{bad}");
        }
        let xml = "<testsuite><testcase name=\"a\" time=\"-1\"></testcase></testsuite>";
        let clamped = parse_test_xml(xml.as_bytes(), 0, 0).expect("negative time clamps");
        assert_eq!(clamped.len(), 1);
        assert_eq!(clamped[0].time, 0.0);
        for child in [
            "<failure></failure>",
            "<error></error>",
            "<skipped></skipped>",
            "<system-out>out</system-out>",
            "<system-err>err</system-err>",
            "<foo></foo>",
            "<foo/>",
        ] {
            let xml = format!("<testsuite><testcase name=\"a\">{child}</testcase></testsuite>");
            assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_ok(), "{child}");
        }
        let xml = "<testsuite><testcase name=\"a\"><skipped/><skipped/></testcase></testsuite>";
        assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_err());
        for dup in ["<system-out/><system-out/>", "<system-err/><system-err/>"] {
            let xml = format!("<testsuite><testcase name=\"a\">{dup}</testcase></testsuite>");
            assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_ok(), "{dup}");
        }
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\">hello</testcase></testsuite>",
            0,
            0
        )
        .is_ok());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"><![CDATA[hello]]></testcase></testsuite>",
            0,
            0
        )
        .is_ok());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"><failure>&notanentity</failure></testcase></testsuite>",
            0,
            0
        )
        .is_err());
        for dup in [
            "<failure></failure><failure></failure>",
            "<error></error><error></error>",
            "<skipped></skipped><skipped></skipped>",
        ] {
            let xml = format!("<testsuite><testcase name=\"a\">{dup}</testcase></testsuite>");
            assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_err(), "{dup}");
        }
        for dup in [
            "<system-out>a</system-out><system-out>b</system-out>",
            "<system-err>a</system-err><system-err>b</system-err>",
        ] {
            let xml = format!("<testsuite><testcase name=\"a\">{dup}</testcase></testsuite>");
            assert!(parse_test_xml(xml.as_bytes(), 0, 0).is_ok(), "{dup}");
        }
        assert!(parse_test_xml(
            b"<testsuite><failure></failure><testcase name=\"a\"/></testsuite>",
            0,
            0
        )
        .is_ok());
        assert!(parse_test_xml(
            b"<testsuite><testcase name=\"a\"></testcase></testsuite>",
            0,
            0
        )
        .is_ok());
        assert!(parse_test_xml(b"<testsuite><testcase name=\"a\"></testsuite>", 0, 0).is_err());
    }

    #[test]
    fn junit_parse_accepts_jest_timestamp_without_timezone() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="jest tests" tests="2" failures="0" errors="0" time="0.803">
  <testsuite name="Hello.astro" errors="0" failures="0" skipped="0" timestamp="2026-09-19T21:10:02" time="0.602" tests="2">
    <testcase classname="Hello.astro parses" name="parses" time="0.005">
    </testcase>
    <testcase classname="Hello.astro reports" name="reports" time="0.002">
    </testcase>
  </testsuite>
</testsuites>"#;
        let cases = parse_test_xml(xml.as_bytes(), 0, 0).expect("jest timestamp");
        assert_eq!(cases.len(), 2);
        let tricky = r#"<testsuite name="a" timestamp="2026-09-19T21:10:02"><testcase name="a"><failure>timestamp="kept"</failure></testcase></testsuite>"#;
        let cases = parse_test_xml(tricky.as_bytes(), 0, 0).expect("tricky");
        assert!(cases[0]
            .failure
            .as_ref()
            .expect("failure")
            .text
            .contains("kept"));
    }

    #[test]
    fn junit_strip_timestamp_uses_typed_events() {
        let single = r#"<testsuite name="a" timestamp = '2026-09-19T21:10:02'><testcase name="a"/></testsuite>"#;
        let stripped = normalize_document(single);
        assert!(!stripped.contains("2026-09-19T21:10:02"));
        assert!(stripped.contains(r#"name="a""#));
        assert!(parse_test_xml(single.as_bytes(), 0, 0).is_ok());

        let gt = r#"<testsuite name="a>b" timestamp="2026-09-19T21:10:02"><testcase name="a"/></testsuite>"#;
        let stripped = normalize_document(gt);
        assert!(!stripped.contains("2026-09-19T21:10:02"));
        assert!(stripped.contains(r#"name="a>b""#));
        assert!(parse_test_xml(gt.as_bytes(), 0, 0).is_ok());

        let value_lookalike = r#"<testsuite name='a timestamp="kept" b' timestamp="2026-09-19T21:10:02"><testcase name="a"/></testsuite>"#;
        let stripped = normalize_document(value_lookalike);
        assert!(!stripped.contains("2026-09-19T21:10:02"));
        assert!(stripped.contains("&quot;kept&quot;"));
        assert!(parse_test_xml(value_lookalike.as_bytes(), 0, 0).is_ok());

        let lookalike_name = r#"<testsuite><testcase name='a timestamp="kept" b' timestamp="2026-09-19T21:10:02"/></testsuite>"#;
        let cases = parse_test_xml(lookalike_name.as_bytes(), 0, 0).expect("lookalike name");
        assert_eq!(cases[0].name, r#"a timestamp="kept" b"#);

        let cdata_inner = r#"<testsuite timestamp="2026-09-19T21:10:02">"#;
        let cdata = r#"<testsuite name="a" timestamp="2026-09-19T21:10:02"><testcase name="a"><failure><![CDATA[<testsuite timestamp="2026-09-19T21:10:02">]]></failure></testcase></testsuite>"#;
        let stripped = normalize_document(cdata);
        assert!(stripped.contains(cdata_inner));
        let cases = parse_test_xml(cdata.as_bytes(), 0, 0).expect("cdata");
        assert!(cases[0]
            .failure
            .as_ref()
            .expect("failure")
            .text
            .contains("testsuite"));

        let comment = r#"<!-- <testsuite timestamp="2026-09-19T21:10:02"> -->"#;
        let with_comment = format!(
            r#"<testsuite name="a" timestamp="2026-09-19T21:10:02">{comment}<testcase name="a"/></testsuite>"#
        );
        let stripped = normalize_document(&with_comment);
        assert!(stripped.contains(comment));
        assert!(parse_test_xml(with_comment.as_bytes(), 0, 0).is_ok());

        let roots = r#"<testsuites timestamp="2026-09-19T21:10:02"><testsuite><testcase name="a"/></testsuite></testsuites>"#;
        assert!(parse_test_xml(roots.as_bytes(), 0, 0).is_ok());
        let case_ts =
            r#"<testsuite><testcase name="a" timestamp="2026-09-19T21:10:02"/></testsuite>"#;
        assert!(parse_test_xml(case_ts.as_bytes(), 0, 0).is_ok());
    }
}
