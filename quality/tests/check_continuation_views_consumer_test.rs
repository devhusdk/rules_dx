mod check_continuation_common;

use check_continuation_common::*;
use dx_testing::{read, serde_json};

#[test]
fn text_and_report_views_agree_with_the_phase_outcomes() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let text = dx(&root, "check", "//...", &["--output=text"]);
    assert_eq!(text.status.code(), Some(1), "{}", text.combined());
    assert!(
        text.stderr.contains(
            "dx: phases: generate findings, format findings, lint findings, typecheck findings"
        ),
        "{}",
        text.combined()
    );
    let reported = dx(
        &root,
        "check",
        "//...",
        &["--output=json", "--report", "sarif=check.sarif"],
    );
    assert_eq!(reported.status.code(), Some(1), "{}", reported.combined());
    let all = events(&reported);
    assert_eq!(phase_exits(&all), exits(1, 1, 1, 1), "{}", reported.stdout);
    let sarif: serde_json::Value = serde_json::from_str(&read(&root.join("check.sarif")))
        .unwrap_or_else(|error| panic!("sarif: {error}"));
    assert_eq!(sarif["version"], "2.1.0");
    let results: Vec<String> = sarif["runs"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .flat_map(|run| run["results"].as_array().cloned().unwrap_or_default())
        .map(|result| result["ruleId"].as_str().unwrap_or("?").to_owned())
        .collect();
    assert!(
        results.iter().any(|rule| rule.ends_with("F401")),
        "{results:?}"
    );
    assert!(
        results
            .iter()
            .any(|rule| rule.ends_with("invalid-return-type")),
        "{results:?}"
    );
    let report = of_kind(&all, "report");
    let written = report
        .iter()
        .find(|event| event["path"] == "check.sarif")
        .unwrap_or_else(|| panic!("{}", reported.stdout));
    assert_eq!(written["results_complete"], true, "{}", reported.stdout);
    let mut expected = before;
    expected.insert(
        "check.sarif".to_owned(),
        read(&root.join("check.sarif")).into_bytes(),
    );
    assert_same(&root, &expected);
}
