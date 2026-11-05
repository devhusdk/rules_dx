use crate::{cobertura_to_lcov, parse_lcov};

const REPORT: &str = r#"<?xml version="1.0" ?>
<coverage line-rate="0.5" version="1.9">
  <packages>
    <package name="app" line-rate="0.5">
      <classes>
        <class name="b" filename="/w/src/B &amp; C.zig" line-rate="0.5">
          <methods><method name="run"><lines><line number="3" hits="9"/></lines></method></methods>
          <lines>
            <line number="3" hits="2" branch="false"/>
            <line number="4" hits="0" branch="false"/>
          </lines>
        </class>
        <class name="a" filename="/w/src/a.zig">
          <lines><line number="1" hits="1"/></lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>"#;

#[test]
fn converts_class_lines_in_path_order_and_skips_method_lines() {
    let lcov = cobertura_to_lcov(REPORT).expect("converts");
    assert_eq!(
        lcov,
        "SF:/w/src/B & C.zig\nDA:3,2\nDA:4,0\nLF:2\nLH:1\nend_of_record\n\
         SF:/w/src/a.zig\nDA:1,1\nLF:1\nLH:1\nend_of_record\n"
    );
}

#[test]
fn output_parses_as_lcov() {
    let lcov = cobertura_to_lcov(REPORT).expect("converts");
    assert!(parse_lcov(&lcov).is_ok());
}

#[test]
fn rejects_reports_without_classes() {
    assert!(cobertura_to_lcov("<coverage/>").is_err());
}

#[test]
fn rejects_lines_outside_a_class() {
    assert!(cobertura_to_lcov(r#"<line number="1" hits="1"/>"#).is_err());
}
