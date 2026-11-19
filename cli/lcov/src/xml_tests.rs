use super::{Unterminated, attribute, tags};

fn names(xml: &str) -> Vec<Result<String, Unterminated>> {
    tags(xml)
        .map(|tag| tag.map(|tag| format!("{}|{}", tag.name, tag.attrs)))
        .collect()
}

#[test]
fn skips_comments_cdata_and_processing_instructions() {
    let xml = r#"<?xml a="?"?><!-- <x> --><![CDATA[<y>]]><a k="1"/></a>"#;
    assert_eq!(
        names(xml),
        vec![Ok("a|k=\"1\"".to_owned()), Ok("/a|".to_owned())]
    );
}

#[test]
fn ends_a_tag_after_quoted_greater_than_signs() {
    let xml = r#"<a x="1>2" y='>'>text"#;
    let tag = tags(xml).next().expect("a tag").expect("terminated");
    assert_eq!(tag.attrs, r#"x="1>2" y='>'"#);
    assert_eq!(tag.tail, "text");
}

#[test]
fn reports_unterminated_markup_and_stops() {
    assert_eq!(names("<a x=\"1>"), vec![Err(Unterminated("tag"))]);
    assert_eq!(
        names("<a/><!-- x"),
        vec![Ok("a|".to_owned()), Err(Unterminated("comment"))]
    );
    assert_eq!(
        names("<![CDATA[ x"),
        vec![Err(Unterminated("CDATA section"))]
    );
    assert_eq!(
        names("<?xml"),
        vec![Err(Unterminated("processing instruction"))]
    );
}

#[test]
fn attribute_skips_earlier_attributes_and_stops_at_bad_quoting() {
    assert_eq!(attribute("a=\"1\" b='2'", "b"), Some("2".to_owned()));
    assert_eq!(attribute("a=\"1\" b=2", "b"), None);
    assert_eq!(attribute("a=\"1\"", "c"), None);
}
