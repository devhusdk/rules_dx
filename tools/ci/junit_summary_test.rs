use junit_summary::render;

fn suite(target: &str, cases: &str) -> String {
    format!("<testsuites><testsuite name=\"{target}\">{cases}</testsuite></testsuites>")
}

#[test]
fn passing_report_is_empty() {
    let xml = suite(
        "//a:t",
        "<testcase name=\"ok\"/><testcase name=\"fine\"></testcase>",
    );
    assert_eq!(render(&xml), "");
}

#[test]
fn failures_show_target_case_and_first_line() {
    let xml = suite(
        "//a:t",
        "<testcase name=\"ok\"/>\
         <testcase name=\"bad\"><failure message=\"boom&#10;second\"/></testcase>\
         <testcase name=\"worse\"><error>\n  crashed here\nmore</error></testcase>",
    );
    let out = render(&xml);
    assert!(out.contains("| //a:t | bad | boom |"), "{out}");
    assert!(out.contains("| //a:t | worse | crashed here |"), "{out}");
    assert!(!out.contains("second") && !out.contains("| ok |"), "{out}");
}

#[test]
fn cells_escape_pipes() {
    let xml = suite(
        "//a:t",
        "<testcase name=\"a|b\"><failure message=\"x\"/></testcase>",
    );
    assert!(render(&xml).contains("| a\\|b |"));
}

#[test]
fn rows_are_capped_at_fifty() {
    let cases: String = (0..53)
        .map(|n| format!("<testcase name=\"c{n}\"><failure message=\"m\"/></testcase>"))
        .collect();
    let out = render(&suite("//a:t", &cases));
    assert_eq!(out.matches("| //a:t |").count(), 50);
    assert!(out.contains("3 more failed."), "{out}");
    assert!(!out.contains("c50"));
}
