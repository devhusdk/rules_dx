use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const MODULE: &str = "module(name = \"acme_renamed\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" test "*) ;; *) exit 0 ;; esac
for arg in "$@"; do
  case "$arg" in --build_event_json_file=*) bep="${arg#--build_event_json_file=}" ;; esac
done
printf '%s\n' "$FAKE_BEP" > "$bep"
exit "$FAKE_EXIT"
"#;

const FRESH_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="fresh"><testcase name="adds" classname="math"/><testcase name="divides" classname="math"><failure message="expected 2, got 3">trace</failure></testcase></testsuite></testsuites>"#;

const WARM_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="warm"><testcase name="parses" classname="io"/><testcase name="later" classname="io"><skipped message="not here"/></testcase></testsuite></testsuites>"#;

fn result(label: &str, status: &str, cached: bool, xml: &Path) -> Value {
    let mut body = json!({
        "status": status,
        "testActionOutput": [{"name": "test.xml", "uri": format!("file://{}", xml.display())}],
    });
    if cached {
        body["cachedLocally"] = json!(true);
    }
    json!({
        "id": {"testResult": {"label": label, "run": 1, "shard": 1, "attempt": 1, "configuration": {"id": "cfg1"}}},
        "testResult": body,
    })
}

fn run(
    fresh_status: &str,
    remove_warm: bool,
    exit: i32,
) -> (
    Option<i32>,
    Vec<Value>,
    String,
    Option<String>,
    Option<Value>,
) {
    let scratch = dx_test_scratch::scratch("dx-case-launcher-");
    let base = scratch.path().to_path_buf();
    let workspace = base.join("ws");
    let bin = base.join("bin");
    let tmp = base.join("tmp");
    let logs = base.join("logs");
    for dir in [&workspace, &bin, &tmp, &logs, &workspace.join(".dx")] {
        fs::create_dir_all(dir).expect("dir");
    }
    fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
    let fresh = logs.join("fresh.xml");
    let warm = logs.join("warm.xml");
    fs::write(&fresh, FRESH_XML).expect("fresh xml");
    if !remove_warm {
        fs::write(&warm, WARM_XML).expect("warm xml");
    }
    let lines = [
        result("//math:fresh", fresh_status, false, &fresh),
        result("//io:warm", "PASSED", true, &warm),
        json!({"id": {"buildMetrics": {}}, "lastMessage": true}),
    ];
    let bep = lines
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let bazel = bin.join("bazel");
    fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
    fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
    let output = Command::new(dx_testing::runfiles_root().join("cli/cli/dx"))
        .args([
            "test",
            "--output=json",
            "--report=junit=junit.xml",
            "--report=results=results.json",
            "//...",
        ])
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", &base)
        .env("TMPDIR", &tmp)
        .env("FAKE_BEP", bep)
        .env("FAKE_EXIT", exit.to_string())
        .stdin(Stdio::null())
        .output()
        .expect("dx runs");
    let report = fs::read_to_string(workspace.join("junit.xml")).ok();
    let results = fs::read(workspace.join("results.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("NDJSON"))
        .collect();
    (
        output.status.code(),
        events,
        String::from_utf8_lossy(&output.stderr).into_owned(),
        report,
        results,
    )
}

fn of_kind<'a>(events: &'a [Value], kind: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["event"] == kind)
        .collect()
}

#[test]
fn the_launcher_reports_executed_and_cached_cases_in_a_renamed_consumer() {
    let (code, events, err, report, results) = run("FAILED", false, 3);
    assert_eq!(code, Some(3), "{events:?}\n{err}");
    assert!(of_kind(&events, "test_case").is_empty(), "{events:?}");
    let outcomes = of_kind(&events, "test_outcome");
    assert_eq!(outcomes.len(), 1, "{events:?}");
    assert_eq!(outcomes[0]["target"], json!("//math:fresh"));
    assert_eq!(outcomes[0]["outcome"], json!("failed"));
    assert_eq!(
        outcomes[0]["failing_cases"],
        json!([{
            "classname": "math",
            "name": "divides",
            "outcome": "failed",
            "message": "expected 2, got 3",
        }])
    );
    let errors = of_kind(&events, "error");
    assert_eq!(errors.len(), 1, "{events:?}");
    assert_eq!(errors[0]["code"], json!("tests_failed"));
    let finished = events.last().expect("events");
    assert_eq!(finished["event"], json!("command_finished"));
    assert_eq!(finished["results_complete"], json!(true));
    assert_eq!(
        finished["tests"],
        json!({"passed": 1, "failed": 1, "cached": 1, "no_cases": 0, "skipped": 0})
    );
    let results = results.expect("results report");
    let cases: Vec<(String, String, String)> = results["results"]
        .as_array()
        .expect("results")
        .iter()
        .flat_map(|entry| {
            entry["cases"]
                .as_array()
                .expect("cases")
                .iter()
                .map(|case| {
                    (
                        entry["target"].as_str().unwrap_or_default().to_owned(),
                        case["name"].as_str().unwrap_or_default().to_owned(),
                        case["outcome"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let expect = |target: &str, name: &str, outcome: &str| {
        (target.to_owned(), name.to_owned(), outcome.to_owned())
    };
    assert_eq!(
        cases,
        vec![
            expect("//io:warm", "parses", "passed"),
            expect("//io:warm", "later", "skipped"),
            expect("//math:fresh", "adds", "passed"),
            expect("//math:fresh", "divides", "failed"),
        ],
        "{results}"
    );
    let report = report.expect("junit report");
    for name in ["parses", "later", "adds", "divides"] {
        assert!(report.contains(&format!("name=\"{name}\"")), "{report}");
    }
}

#[test]
fn a_removed_report_in_a_renamed_consumer_is_incomplete() {
    let (code, events, err, _, _) = run("PASSED", true, 0);
    assert_eq!(code, Some(1), "{events:?}\n{err}");
    let outcomes = of_kind(&events, "test_outcome");
    let warm = outcomes
        .iter()
        .find(|outcome| outcome["target"] == json!("//io:warm"))
        .expect("warm outcome");
    assert_eq!(warm["evidence_complete"], json!(false), "{events:?}");
    assert_eq!(outcomes.len(), 1, "{events:?}");
    let error = of_kind(&events, "error");
    assert!(
        error
            .iter()
            .any(|event| event["code"] == json!("incomplete_results")),
        "{events:?}"
    );
    let finished = events.last().expect("events");
    assert_eq!(finished["results_complete"], json!(false));
    assert_eq!(finished["exit_code"], json!(1));
}
