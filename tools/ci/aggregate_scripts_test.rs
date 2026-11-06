use std::path::Path;
use std::process::Command;

use ci_workflow::document;
use dx_testing::runfiles_root;
use serde_json::Value;

const AGGREGATE_FIXTURES: &str = "tools/ci/aggregate_fixtures";
const CI_GATE_FIXTURES: &str = "tools/ci/ci_gate_fixtures";
const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";
const CI_WORKFLOW: &str = ".github/workflows/ci.yml";
const RESULTS: [&str; 4] = ["success", "failure", "skipped", "cancelled"];

struct Expect {
    inputs: Value,
    status: i32,
    says: Vec<String>,
}

struct Outcome {
    status: i32,
    text: String,
}

fn script(workflow: &str, job: &str, step: usize) -> String {
    let run = document(workflow)["jobs"][job]["steps"][step]["run"].clone();
    run.as_str()
        .unwrap_or_else(|| panic!("{workflow} job {job} step {step} has no run script"))
        .to_owned()
}

fn run_script(dir: &Path, script: &str, env: &[(String, String)]) -> Outcome {
    let mut command = Command::new("bash");
    command.arg("-e").arg("-c").arg(script).current_dir(dir);
    command
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run bash");
    Outcome {
        status: done.status.code().unwrap_or(-1),
        text: format!(
            "{}{}",
            String::from_utf8_lossy(&done.stdout),
            String::from_utf8_lossy(&done.stderr)
        ),
    }
}

fn case_names(directory: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(runfiles_root().join(directory))
        .unwrap_or_else(|error| panic!("read {directory}: {error}"))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "{directory} holds no fixture");
    names
}

fn expectation(directory: &str, case: &str) -> Expect {
    let path = runfiles_root()
        .join(directory)
        .join(case)
        .join("expect.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let mut expect = Expect {
        inputs: Value::Null,
        status: -1,
        says: Vec::new(),
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("input ") {
            expect.inputs = serde_json::from_str(rest).expect("fixture input is JSON");
        } else if let Some(rest) = line.strip_prefix("exit ") {
            expect.status = rest.parse().expect("fixture exit is a number");
        } else if let Some(rest) = line.strip_prefix("says ") {
            expect.says.push(rest.to_owned());
        }
    }
    assert!(
        expect.status == 0 || expect.status == 1,
        "{case} declares no exit status"
    );
    assert!(!expect.says.is_empty(), "{case} expects no text");
    for result in expect.inputs["NEEDS"]
        .as_object()
        .unwrap_or_else(|| panic!("{case} has no NEEDS"))
        .values()
    {
        let result = result["result"].as_str().unwrap_or_default();
        assert!(RESULTS.contains(&result), "{case} holds result {result:?}");
    }
    expect
}

fn env_of(inputs: &Value) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for (key, value) in inputs.as_object().expect("fixture inputs are an object") {
        match (key.as_str(), value) {
            ("NEEDS", Value::Object(needs)) => {
                for (job, info) in needs {
                    let var = format!("NEEDS_{}", job.to_uppercase().replace('-', "_"));
                    env.push((var, info["result"].as_str().unwrap_or_default().to_owned()));
                }
            }
            (_, Value::String(text)) => env.push((key.clone(), text.clone())),
            (_, other) => env.push((key.clone(), other.to_string())),
        }
    }
    env
}

fn check_fixtures(directory: &str, script: &str, scratch: &str) {
    let dir = dx_test_scratch::scratch(scratch);
    for case in case_names(directory) {
        let expect = expectation(directory, &case);
        let done = run_script(dir.path(), script, &env_of(&expect.inputs));
        assert_eq!(
            done.status, expect.status,
            "{case} exited {}: {}",
            done.status, done.text
        );
        for says in &expect.says {
            assert!(
                done.text.contains(says),
                "{case} never said {says}: {}",
                done.text
            );
        }
    }
}

#[test]
fn consumer_aggregate_fixtures_report_the_expected_verdict() {
    let script = script(CONSUMER_WORKFLOW, "dx-ci", 0);
    check_fixtures(AGGREGATE_FIXTURES, &script, "aggregate-consumer");
}

#[test]
fn ci_gate_fixtures_report_the_expected_verdict() {
    let script = script(CI_WORKFLOW, "ci", 0);
    check_fixtures(CI_GATE_FIXTURES, &script, "aggregate-ci");
}

#[test]
fn named_fixtures_exist_with_the_expected_exit() {
    let rows: [(&str, &[(&str, i32)]); 2] = [
        (
            AGGREGATE_FIXTURES,
            &[
                ("all_checks_green", 0),
                ("failing_check_fails", 1),
                ("cancelled_check_fails", 1),
                ("disabled_check_stays_green", 0),
                ("skipped_without_disabled_fails", 1),
                ("missing_check_fails", 1),
                ("all_matrix_checks_disabled", 0),
                ("validator_without_checks_output", 1),
            ],
        ),
        (
            CI_GATE_FIXTURES,
            &[
                ("all_green", 0),
                ("failed_job_fails", 1),
                ("cancelled_job_fails", 1),
                ("skipped_job_fails", 1),
                ("missing_job_fails", 1),
            ],
        ),
    ];
    for (directory, cases) in rows {
        for (case, status) in cases {
            assert!(
                case_names(directory).iter().any(|n| n == case),
                "{directory} lacks {case}"
            );
            assert_eq!(expectation(directory, case).status, *status, "{case}");
        }
    }
}

fn table_of(lines: &[&str], platforms: &[&str]) -> (String, String) {
    let dir = dx_test_scratch::scratch("compiler-table");
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).expect("create evidence dir");
    std::fs::write(input.join("a.jsonl"), lines.join("\n") + "\n").expect("write evidence");
    let out = dir.path().join("all.jsonl");
    let done = Command::new("bash")
        .arg(runfiles_root().join("tools/ci/compiler_table.sh"))
        .arg(&input)
        .arg(&out)
        .args(platforms)
        .output()
        .expect("run compiler_table.sh");
    assert!(done.status.success(), "{done:?}");
    (
        String::from_utf8_lossy(&done.stdout).into_owned(),
        std::fs::read_to_string(out).expect("read merged evidence"),
    )
}

#[test]
fn compiler_table_keeps_passed_failed_skipped_and_missing_apart() {
    let (table, merged) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
            r#"{"platform":"linux_x86_64","row":"llvm","outcome":"failed"}"#,
            r#"{"platform":"macos_arm64","row":"llvm","outcome":"skipped"}"#,
        ],
        &["windows_x86_64"],
    );
    assert!(
        table.contains("| row | linux_x86_64 | macos_arm64 | windows_x86_64 |"),
        "{table}"
    );
    assert!(
        table.contains("| gcc | passed | no evidence | no evidence |"),
        "{table}"
    );
    assert!(
        table.contains("| llvm | failed | skipped | no evidence |"),
        "{table}"
    );
    assert_eq!(merged.lines().count(), 3);
}

#[test]
fn compiler_table_never_upgrades_a_skipped_cell() {
    let (table, _) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
        ],
        &[],
    );
    assert!(table.contains("| gcc | skipped |"), "{table}");
}

#[test]
fn compiler_table_reports_mismatch_over_passed() {
    let (table, _) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"mismatch"}"#,
        ],
        &[],
    );
    assert!(table.contains("| gcc | mismatch |"), "{table}");
}

#[test]
fn compiler_table_keeps_a_lone_mismatch() {
    let (table, _) = table_of(
        &[r#"{"platform":"windows_x86_64","row":"msvc","outcome":"mismatch"}"#],
        &[],
    );
    assert!(table.contains("| msvc | mismatch |"), "{table}");
}
