//! Pins the runner map of release-cells.yml (and the release.yml build matrix) to the release matrix.

use serde_json::Value;

use ci_workflow::document;
use dx_testing::resolve_runfiles;

const CELLS: &str = ".github/workflows/release-cells.yml";
const RELEASE: &str = ".github/workflows/release.yml";

/// GitHub-hosted runner labels with the release matrix `(os, machine)` each one runs.
const RUNNERS: [(&str, &str, &str); 5] = [
    ("ubuntu-latest", "linux", "x86_64"),
    ("ubuntu-24.04-arm", "linux", "aarch64"),
    ("macos-15", "macos", "aarch64"),
    ("windows-latest", "windows", "x86_64"),
    ("windows-11-arm", "windows", "aarch64"),
];

/// Returns the release matrix cells as `(name, os, machine)` in matrix order.
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

/// Returns `(name, runner)` for every `matrix.include` entry of the one job that has a matrix.
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

/// Returns every problem between a workflow and the release matrix.
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
        if let Some((_, cell_os, cell_machine)) = cells.iter().find(|(cell, _, _)| cell == name) {
            if (*os, *machine) != (cell_os.as_str(), cell_machine.as_str()) {
                problems.push(format!(
                    "{name}: runner {runner:?} is {os}/{machine}, the cell is {cell_os}/{cell_machine}"
                ));
            }
        }
    }
    problems
}

/// Returns the problems with the permissions and failure policy of release-cells.yml.
fn policy_problems(workflow: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let mut scopes = vec![("workflow".to_owned(), workflow["permissions"].clone())];
    for (id, job) in workflow["jobs"].as_object().expect("jobs") {
        if job["permissions"].is_object() {
            scopes.push((id.clone(), job["permissions"].clone()));
        } else {
            problems.push(format!("job {id} sets no permissions of its own"));
        }
        if let Some(strategy) = job.get("strategy") {
            if strategy["fail-fast"] != Value::Bool(false) {
                problems.push(format!("job {id} must set fail-fast: false"));
            }
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
    let problems = problems(&document(CELLS), &matrix_cells());
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn release_yml_build_matrix_uses_the_same_runner_map() {
    // Temporary drift guard until release.yml calls release-cells.yml.
    let cells = runner_map(&document(CELLS));
    let release = runner_map(&document(RELEASE));
    assert_eq!(release, cells, "{RELEASE} differs from {CELLS}");
}

#[test]
fn release_cells_is_read_only_and_reports_every_cell() {
    let problems = policy_problems(&document(CELLS));
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
        .collect();
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
