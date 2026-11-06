use crate::{clover_to_lcov, parse_lcov};

const REPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<coverage generated="1">
  <project timestamp="1">
    <package name="App">
      <file name="/w/src/B &amp; C.php">
        <class name="C" namespace="App"><metrics methods="1"/></class>
        <line num="9" type="method" name="run" count="3"/>
        <line num="10" type="stmt" count="3"/>
        <line num="12" type="stmt" count="0"/>
        <metrics loc="12"/>
      </file>
    </package>
    <file name="/w/src/A.php">
      <line num="2" type="stmt" count="1"/>
    </file>
  </project>
</coverage>"#;

#[test]
fn converts_statement_lines_in_path_order() {
    let lcov = clover_to_lcov(REPORT).expect("converts");
    assert_eq!(
        lcov,
        "SF:/w/src/A.php\nDA:2,1\nLF:1\nLH:1\nend_of_record\n\
         SF:/w/src/B & C.php\nDA:10,3\nDA:12,0\nLF:2\nLH:1\nend_of_record\n"
    );
}

#[test]
fn output_is_valid_lcov() {
    let files = parse_lcov(&clover_to_lcov(REPORT).expect("converts")).expect("parses");
    assert_eq!(files["/w/src/B & C.php"].lines[&12], 0);
}

#[test]
fn rejects_reports_without_files() {
    assert!(clover_to_lcov("<coverage/>").is_err());
}

#[test]
fn rejects_a_line_outside_a_file() {
    assert!(clover_to_lcov(r#"<line num="1" type="stmt" count="1"/>"#).is_err());
}

#[test]
fn rejects_a_non_numeric_count() {
    let xml = r#"<file name="a.php"><line num="1" type="stmt" count="x"/></file>"#;
    assert!(clover_to_lcov(xml).is_err());
}

#[test]
fn attribute_skips_earlier_attributes_and_stops_at_bad_quoting() {
    assert_eq!(super::attribute("a=\"1\" b='2'", "b"), Some("2".to_owned()));
    assert_eq!(super::attribute("a=\"1\" b=2", "b"), None);
    assert_eq!(super::attribute("a=\"1\"", "c"), None);
}
