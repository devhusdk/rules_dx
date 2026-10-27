mod common;

use common::{consumer, dx, error_codes, events, finished, of_kind, results_json, Tamper};
use dx_nested::Case;
use dx_testing::serde_json::Value;

const CONSUMER: [&str; 2] = ["--", "--config=consumer"];

fn test_args<'a>(targets: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["test", "--output=json"];
    args.extend_from_slice(targets);
    args.extend_from_slice(&CONSUMER);
    args.extend_from_slice(extra);
    args
}

/// The `test_outcome` event of the last attempt of `target`; passing results emit none.
fn outcome<'a>(events: &'a [Value], target: &str) -> &'a Value {
    let found: Vec<&Value> = events
        .iter()
        .filter(|event| {
            event["event"] == "test_outcome" && event["target"] == target && event["final"] != false
        })
        .collect();
    assert_eq!(found.len(), 1, "one outcome for {target}: {found:?}");
    found[0]
}

/// The record of the last attempt of `target` in results.json.
fn result<'a>(document: &'a Value, target: &str) -> &'a Value {
    let found: Vec<&Value> = document["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results: {document}"))
        .iter()
        .filter(|result| result["target"] == target && result["final"] != false)
        .collect();
    assert_eq!(found.len(), 1, "one result for {target}: {document}");
    found[0]
}

fn results(case: &Case) -> Value {
    results_json(&case.root().join(".dx/results.json"))
}

fn configured(outcome: &Value) {
    let id = outcome["configuration"].as_str().unwrap_or_default();
    assert!(!id.is_empty(), "no configuration id: {outcome}");
}

#[test]
fn executed_then_cached() {
    let case = consumer(
        "executed",
        &[
            ("pass", "pass"),
            ("fail", "fail"),
            ("none", "none"),
            ("retry", "retry"),
        ],
    );
    let tamper = Tamper::new();
    let targets = [
        "//cases:pass",
        "//cases:fail",
        "//cases:none",
        "//cases:retry",
    ];
    let args = test_args(&targets, &["--flaky_test_attempts=2"]);

    let first = dx(&case, &args, &[], &tamper);
    assert_eq!(first.status.code(), Some(3), "{}", first.combined());
    let all = events(&first);
    assert!(
        error_codes(&all).contains(&"tests_failed".to_owned()),
        "{}",
        first.stdout
    );
    assert!(!error_codes(&all).contains(&"incomplete_results".to_owned()));

    let fail = outcome(&all, "//cases:fail");
    assert_eq!(fail["outcome"], "failed", "{fail}");
    assert_ne!(fail["evidence_complete"], false, "{fail}");
    configured(fail);
    let failing = fail["failing_cases"]
        .as_array()
        .unwrap_or_else(|| panic!("failing_cases: {fail}"));
    assert_eq!(failing.len(), 1, "{fail}");
    assert_eq!(failing[0]["classname"], "cases.fail", "{fail}");
    assert_eq!(failing[0]["name"], "breaks", "{fail}");
    assert_eq!(failing[0]["message"], "expected 1, got 2", "{fail}");

    let retry = outcome(&all, "//cases:retry");
    assert_eq!(retry["outcome"], "passed_after_retry", "{retry}");
    assert_eq!(retry["attempt"], 2, "{retry}");
    let done = finished(&all);
    assert_eq!(done["exit_code"], 3, "{done}");
    assert_eq!(done["results_complete"], true, "{done}");
    assert_eq!(done["tests"]["cached"], 0, "{done}");

    let document = results(&case);
    assert_eq!(document["complete"], true, "{document}");
    assert_eq!(document["exit_code"], 3, "{document}");
    assert_eq!(document["run_id"], done["run_id"], "{document}");
    for target in targets {
        let record = result(&document, target);
        configured(record);
        assert_ne!(record["evidence_complete"], false, "{record}");
        assert_ne!(record["cached"], true, "{record}");
    }
    let pass = result(&document, "//cases:pass");
    assert_eq!(pass["outcome"], "passed", "{pass}");
    assert_eq!(pass["cases_total"], 2, "{pass}");
    assert_eq!(pass["cases_skipped"], 1, "{pass}");
    assert_eq!(result(&document, "//cases:none")["outcome"], "no_cases");
    assert_eq!(
        result(&document, "//cases:retry")["outcome"],
        "passed_after_retry"
    );

    let second = dx(&case, &args, &[], &tamper);
    assert_eq!(second.status.code(), Some(3), "{}", second.combined());
    let again = events(&second);
    assert!(
        error_codes(&again).contains(&"tests_failed".to_owned()),
        "{}",
        second.stdout
    );
    let done = finished(&again);
    assert_eq!(done["results_complete"], true, "{done}");
    let document = results(&case);
    assert_eq!(document["complete"], true, "{document}");
    for target in ["//cases:pass", "//cases:none"] {
        let record = result(&document, target);
        assert_eq!(record["cached"], true, "{record}");
        configured(record);
    }
    assert_ne!(result(&document, "//cases:fail")["cached"], true);
    let cached = done["tests"]["cached"].as_u64().unwrap_or_default();
    assert!(cached >= 2, "{done}");
    assert!(tamper.changed().is_empty(), "{:?}", tamper.changed());
}

fn assert_incomplete(run: &dx_testing::Run) -> Vec<Value> {
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(run);
    assert!(
        error_codes(&all).contains(&"incomplete_results".to_owned()),
        "{}",
        run.stdout
    );
    let done = finished(&all);
    assert_eq!(done["exit_code"], 1, "{done}");
    assert_eq!(done["results_complete"], false, "{done}");
    all
}

#[test]
fn malformed_xml_is_incomplete() {
    let case = consumer("malformed", &[("truncated", "truncated")]);
    let tamper = Tamper::new();
    let plain = case.bazel(&["test", "--config=consumer", "//cases:truncated"]);
    assert_eq!(plain.status.code(), Some(0), "{}", plain.combined());

    let junit = case.root().join("junit.xml");
    let junit_arg = format!("junit={}", junit.display());
    let run = dx(
        &case,
        &[
            "test",
            "--output=json",
            "--report",
            junit_arg.as_str(),
            "//cases:truncated",
            "--",
            "--config=consumer",
        ],
        &[],
        &tamper,
    );
    let all = assert_incomplete(&run);
    assert_eq!(
        outcome(&all, "//cases:truncated")["evidence_complete"],
        false
    );
    let report = common::read(&junit);
    assert!(report.contains("dx.infrastructure"), "{report}");
}

fn tampered(name: &str, var: &str, verb: &str) {
    let case = consumer(name, &[("kept", "pass"), ("hit", "pass")]);
    let tamper = Tamper::new();
    let args = test_args(&["//cases:kept", "//cases:hit"], &[]);
    let run = dx(&case, &args, &[(var, "//cases:hit")], &tamper);
    let changed = tamper.changed();
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert!(changed[0].starts_with(verb), "{changed:?}");
    assert!(changed[0].ends_with("/cases/hit/test.xml"), "{changed:?}");
    let all = assert_incomplete(&run);
    let hit = outcome(&all, "//cases:hit");
    assert_eq!(hit["evidence_complete"], false, "{hit}");
    assert_eq!(hit["outcome"], "passed", "{hit}");
    assert!(
        !all.iter()
            .any(|event| event["event"] == "test_outcome" && event["target"] == "//cases:kept"),
        "{}",
        run.stdout
    );
    let errors = of_kind(&all, "error");
    let message = errors
        .iter()
        .find(|event| event["code"] == "incomplete_results")
        .map(|event| event["message"].as_str().unwrap_or_default().to_owned())
        .unwrap_or_default();
    assert!(message.contains("//cases:hit"), "{message}");
    assert!(message.contains("cases/hit/test.xml"), "{message}");
    assert!(!message.contains("//cases:kept"), "{message}");
    let document = results(&case);
    assert_eq!(document["complete"], false, "{document}");
    assert_eq!(result(&document, "//cases:hit")["evidence_complete"], false);
    let kept = result(&document, "//cases:kept");
    assert_ne!(kept["evidence_complete"], false, "{kept}");
    assert_eq!(kept["cases_total"], 2, "{kept}");

    let text = dx(
        &case,
        &[
            "test",
            "--output=text",
            "//cases:kept",
            "//cases:hit",
            "--",
            "--config=consumer",
        ],
        &[(var, "//cases:hit")],
        &tamper,
    );
    assert_eq!(text.status.code(), Some(1), "{}", text.combined());
    assert!(
        text.stderr.contains("dx: incomplete_results:"),
        "{}",
        text.stderr
    );

    let clean = dx(&case, &args, &[], &tamper);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    let all = events(&clean);
    assert!(error_codes(&all).is_empty(), "{}", clean.stdout);
    assert_eq!(finished(&all)["results_complete"], true);
    let document = results(&case);
    assert_eq!(document["complete"], true, "{document}");
    let hit = result(&document, "//cases:hit");
    assert_ne!(hit["cached"], true, "{hit}");
    assert_ne!(hit["evidence_complete"], false, "{hit}");
    assert_eq!(hit["cases_total"], 2, "{hit}");
}

#[test]
fn removed_xml_is_incomplete() {
    tampered("removed", "DX_TAMPER_REMOVE", "removed ");
}

#[test]
fn corrupted_xml_is_incomplete() {
    tampered("corrupted", "DX_TAMPER_CORRUPT", "corrupted ");
}
