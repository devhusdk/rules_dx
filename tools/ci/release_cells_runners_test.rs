use serde_json::Value;

use ci_workflow::document;
use dx_testing::resolve_runfiles;

const CELLS: &str = ".github/workflows/release-cells.yml";

const RUNNERS: [(&str, &str, &str); 5] = [
    ("ubuntu-latest", "linux", "x86_64"),
    ("ubuntu-24.04-arm", "linux", "aarch64"),
    ("macos-15", "macos", "aarch64"),
    ("windows-latest", "windows", "x86_64"),
    ("windows-11-arm", "windows", "aarch64"),
];

fn matrix_cells() -> Vec<(String, String, String)> {
    let rel = std::env::var("DX_RELEASE_MATRIX").expect("DX_RELEASE_MATRIX names the matrix");
    let text = std::fs::read_to_string(resolve_runfiles(&rel)).expect("release matrix");
    let value: Value = serde_json::from_str(&text).expect("release matrix JSON");
    value["cells"]
        .as_array()
        .expect("cells")
        .iter()
        .map(|cell| {
            let field = |key: &str| cell[key].as_str().expect("cell field").to_owned();
            (field("name"), field("os"), field("machine"))
        })
        .collect()
}

fn runner_map(workflow: &Value) -> Vec<(String, String)> {
    let jobs = workflow["jobs"].as_object().expect("jobs");
    let mut found = Vec::new();
    for job in jobs.values() {
        let Some(include) = job["strategy"]["matrix"]["include"].as_array() else {
            continue;
        };
        for entry in include {
            found.push((
                entry["name"].as_str().expect("matrix name").to_owned(),
                entry["runner"].as_str().expect("matrix runner").to_owned(),
            ));
        }
    }
    found
}

fn problems(workflow: &Value, cells: &[(String, String, String)]) -> Vec<String> {
    let mut problems = Vec::new();
    let map = runner_map(workflow);
    let have: Vec<&str> = map.iter().map(|(name, _)| name.as_str()).collect();
    let want: Vec<&str> = cells.iter().map(|(name, _, _)| name.as_str()).collect();
    if have != want {
        problems.push(format!("cell names are {have:?}, want {want:?}"));
    }
    for (name, runner) in &map {
        let Some((_, os, machine)) = RUNNERS.iter().find(|(label, _, _)| label == runner) else {
            problems.push(format!("{name}: unknown runner label {runner:?}"));
            continue;
        };
        if let Some((_, cell_os, cell_machine)) = cells.iter().find(|(cell, _, _)| cell == name)
            && (*os, *machine) != (cell_os.as_str(), cell_machine.as_str())
        {
            problems.push(format!(
                "{name}: runner {runner:?} is {os}/{machine}, the cell is {cell_os}/{cell_machine}"
            ));
        }
    }
    problems
}

fn policy_problems(workflow: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let mut scopes = vec![("workflow".to_owned(), workflow["permissions"].clone())];
    for (id, job) in workflow["jobs"].as_object().expect("jobs") {
        if job["permissions"].is_object() {
            scopes.push((id.clone(), job["permissions"].clone()));
        } else {
            problems.push(format!("job {id} sets no permissions of its own"));
        }
        if let Some(strategy) = job.get("strategy")
            && strategy["fail-fast"] != Value::Bool(false)
        {
            problems.push(format!("job {id} must set fail-fast: false"));
        }
    }
    for (scope, permissions) in scopes {
        for (key, value) in permissions.as_object().cloned().unwrap_or_default() {
            if value.as_str() != Some("read") {
                problems.push(format!("{scope}: permission {key}: {value} is not read"));
            }
        }
    }
    problems
}

const MAX_TIMEOUT_MINUTES: u64 = 360;

fn timeout_problems(workflow: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, job) in workflow["jobs"].as_object().expect("jobs") {
        let Some(include) = job["strategy"]["matrix"]["include"].as_array() else {
            continue;
        };
        if job["timeout-minutes"].as_str() != Some("${{ matrix.timeout }}") {
            problems.push(format!(
                "job {id}: timeout-minutes is {}, want ${{{{ matrix.timeout }}}}",
                job["timeout-minutes"]
            ));
        }
        for entry in include {
            let name = entry["name"].as_str().unwrap_or("<unnamed>");
            match entry["timeout"].as_u64() {
                Some(minutes) if (1..=MAX_TIMEOUT_MINUTES).contains(&minutes) => {}
                Some(minutes) => problems.push(format!(
                    "{name}: timeout {minutes} is outside 1..={MAX_TIMEOUT_MINUTES}"
                )),
                None => problems.push(format!(
                    "{name}: timeout is {}, want an integer number of minutes",
                    entry["timeout"]
                )),
            }
        }
    }
    problems
}

fn parse(text: &str) -> Value {
    yaml_serde::from_str(text).expect("inline YAML")
}

fn cell_rows() -> Vec<(String, String, String)> {
    RUNNERS
        .iter()
        .zip([
            "dx-linux-x86_64",
            "dx-linux-arm64",
            "dx-macos-arm64",
            "dx-windows-x86_64",
            "dx-windows-arm64",
        ])
        .map(|((_, os, machine), name)| (name.to_owned(), (*os).to_owned(), (*machine).to_owned()))
        .collect()
}

#[test]
fn release_cells_runner_map_matches_the_release_matrix() {
    let problems = problems(&document(CELLS).unwrap(), &matrix_cells());
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn release_cells_is_read_only_and_reports_every_cell() {
    let problems = policy_problems(&document(CELLS).unwrap());
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn release_cells_sets_a_timeout_per_cell() {
    let problems = timeout_problems(&document(CELLS).unwrap());
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_test_matrix_rows_match_the_checked_in_matrix() {
    assert_eq!(cell_rows(), matrix_cells());
}

fn workflow_with(entries: &[(&str, &str)]) -> Value {
    let rows: String = entries
        .iter()
        .map(|(name, runner)| format!("          - name: {name}\n            runner: {runner}\n"))
        .collect::<Vec<String>>()
        .concat();
    parse(&format!(
        "jobs:\n  cell:\n    strategy:\n      matrix:\n        include:\n{rows}"
    ))
}

#[test]
fn a_missing_cell_is_reported() {
    let workflow = workflow_with(&[
        ("dx-linux-x86_64", "ubuntu-latest"),
        ("dx-linux-arm64", "ubuntu-24.04-arm"),
        ("dx-macos-arm64", "macos-15"),
        ("dx-windows-x86_64", "windows-latest"),
    ]);
    let found = problems(&workflow, &cell_rows());
    assert!(
        found.iter().any(|p| p.contains("cell names are")),
        "{found:?}"
    );
}

#[test]
fn windows_latest_for_windows_arm64_is_reported() {
    let workflow = workflow_with(&[
        ("dx-linux-x86_64", "ubuntu-latest"),
        ("dx-linux-arm64", "ubuntu-24.04-arm"),
        ("dx-macos-arm64", "macos-15"),
        ("dx-windows-x86_64", "windows-latest"),
        ("dx-windows-arm64", "windows-latest"),
    ]);
    let found = problems(&workflow, &cell_rows());
    assert!(
        found.iter().any(|p| p.contains("windows/x86_64")),
        "{found:?}"
    );
}

#[test]
fn an_unknown_runner_label_is_reported() {
    let workflow = workflow_with(&[
        ("dx-linux-x86_64", "ubuntu-latest"),
        ("dx-linux-arm64", "self-hosted"),
        ("dx-macos-arm64", "macos-15"),
        ("dx-windows-x86_64", "windows-latest"),
        ("dx-windows-arm64", "windows-11-arm"),
    ]);
    let found = problems(&workflow, &cell_rows());
    assert!(
        found.iter().any(|p| p.contains("unknown runner label")),
        "{found:?}"
    );
}

#[test]
fn write_scopes_and_fail_fast_are_reported() {
    let workflow = parse(
        "permissions:\n  contents: read\n  id-token: write\njobs:\n  cell:\n    strategy:\n      fail-fast: true\n    permissions:\n      contents: read\n  verify:\n    permissions:\n      attestations: write\n",
    );
    let found = policy_problems(&workflow);
    assert!(found.iter().any(|p| p.contains("id-token")), "{found:?}");
    assert!(
        found.iter().any(|p| p.contains("attestations")),
        "{found:?}"
    );
    assert!(found.iter().any(|p| p.contains("fail-fast")), "{found:?}");
}

fn timed_workflow(timeout_minutes: &str, rows: &str) -> Value {
    parse(&format!(
        "jobs:\n  cell:\n    strategy:\n      matrix:\n        include:\n{rows}    timeout-minutes: {timeout_minutes}\n"
    ))
}

const MATRIX_TIMEOUT: &str = "${{ matrix.timeout }}";

#[test]
fn a_row_without_a_timeout_is_reported() {
    let workflow = timed_workflow(
        MATRIX_TIMEOUT,
        "          - name: dx-linux-x86_64\n            timeout: 90\n          - name: dx-windows-x86_64\n",
    );
    let found = timeout_problems(&workflow);
    assert!(
        found
            .iter()
            .any(|p| p.starts_with("dx-windows-x86_64: timeout is null")),
        "{found:?}"
    );
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_literal_timeout_minutes_is_reported() {
    let workflow = timed_workflow(
        "90",
        "          - name: dx-linux-x86_64\n            timeout: 90\n",
    );
    let found = timeout_problems(&workflow);
    assert!(
        found.iter().any(|p| p.contains("timeout-minutes is 90")),
        "{found:?}"
    );
}

#[test]
fn a_timeout_above_the_hosted_runner_maximum_is_reported() {
    let workflow = timed_workflow(
        MATRIX_TIMEOUT,
        "          - name: dx-windows-x86_64\n            timeout: 400\n",
    );
    let found = timeout_problems(&workflow);
    assert!(
        found.iter().any(|p| p.contains("timeout 400 is outside")),
        "{found:?}"
    );
}

#[test]
fn a_quoted_timeout_is_reported() {
    let workflow = timed_workflow(
        MATRIX_TIMEOUT,
        "          - name: dx-windows-x86_64\n            timeout: \"180\"\n",
    );
    let found = timeout_problems(&workflow);
    assert!(
        found.iter().any(|p| p.contains("want an integer")),
        "{found:?}"
    );
}
