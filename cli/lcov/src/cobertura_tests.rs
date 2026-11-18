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

#[test]
fn joins_relative_filenames_to_the_source() {
    let report = r#"<coverage>
  <sources><source>/w/src/</source></sources>
  <classes><class name="a" filename="a.zig"><lines><line number="2" hits="3"/></lines></class></classes>
</coverage>"#;
    let lcov = cobertura_to_lcov(report).expect("converts");
    assert_eq!(lcov, "SF:/w/src/a.zig\nDA:2,3\nLF:1\nLH:1\nend_of_record\n");
}

#[test]
fn keeps_windows_absolute_filenames_out_of_the_source_join() {
    let xml = r#"<coverage><sources><source>/w</source></sources><classes>
<class filename="C:\x\a.py"><lines><line number="1" hits="1"/></lines></class>
<class filename="D:/y/b.py"><lines><line number="1" hits="1"/></lines></class>
<class filename="\\srv\s\c.py"><lines><line number="1" hits="1"/></lines></class>
</classes></coverage>"#;
    let lcov = cobertura_to_lcov(xml).expect("converts");
    assert!(lcov.contains("SF:C:\\x\\a.py\n"), "{lcov}");
    assert!(lcov.contains("SF:D:/y/b.py\n"), "{lcov}");
    assert!(lcov.contains("SF:\\\\srv\\s\\c.py\n"), "{lcov}");
}

#[test]
fn decodes_numeric_character_references_in_filenames() {
    let xml = r#"<coverage><classes>
<class filename="/w/caf&#233;&#x20;&#x1F600;.py"><lines><line number="1" hits="1"/></lines></class>
<class filename="/w/&amp;lt;.py"><lines><line number="1" hits="1"/></lines></class>
</classes></coverage>"#;
    let lcov = cobertura_to_lcov(xml).expect("converts");
    assert!(lcov.contains("SF:/w/café 😀.py\n"), "{lcov}");
    assert!(lcov.contains("SF:/w/&lt;.py\n"), "{lcov}");
}

#[test]
fn keeps_a_greater_than_sign_inside_a_quoted_attribute() {
    let xml = r#"<coverage><classes><class name="a" filename='/w/a>b.py'><lines><line number="1" hits="1"/></lines></class></classes></coverage>"#;
    let lcov = cobertura_to_lcov(xml).expect("converts");
    assert_eq!(lcov, "SF:/w/a>b.py\nDA:1,1\nLF:1\nLH:1\nend_of_record\n");
}

#[test]
fn skips_tags_inside_comments() {
    let xml = r#"<coverage><!-- a > b <line number="9" hits="1"/> --><classes><class filename="/w/a.py"><lines><line number="1" hits="1"/></lines></class></classes></coverage>"#;
    let lcov = cobertura_to_lcov(xml).expect("converts");
    assert_eq!(lcov, "SF:/w/a.py\nDA:1,1\nLF:1\nLH:1\nend_of_record\n");
}

#[test]
fn skips_tags_inside_cdata() {
    let xml = r#"<coverage><classes><class filename="/w/a.py"><![CDATA[ > <class filename="/w/ghost.py"> ]]><lines><line number="1" hits="1"/></lines></class></classes></coverage>"#;
    let lcov = cobertura_to_lcov(xml).expect("converts");
    assert_eq!(lcov, "SF:/w/a.py\nDA:1,1\nLF:1\nLH:1\nend_of_record\n");
}

#[test]
fn rejects_an_unterminated_cdata_section() {
    let xml = r#"<coverage><classes><class filename="/w/a.py"><lines><line number="1" hits="1"/></lines></class></classes><![CDATA[ a > b"#;
    assert!(cobertura_to_lcov(xml).is_err());
}
