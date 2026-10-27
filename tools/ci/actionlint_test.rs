use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dx_testing::{mkscratch, read_runfiles, resolve_runfiles, runfiles_root};

const CONFIG: &str = ".github/actionlint.yaml";

const PROJECT_MARKER: &str = ".git";

const WORKFLOWS: &str = ".github/workflows";

const CALLERS: [&str; 2] = [
    "examples/consumer-ci/caller.yml",
    "examples/docs-ci/caller.yml",
];

const FIXTURE_DIR: &str = "tools/ci/actionlint_fixtures";

const FIXTURE_WORKFLOWS: [&str; 2] = ["invalid.yml", "reusable.yml"];

const PLATFORM_RUNNERS: &str = "tools/ci/input_validation.py";

const SUPPORTED_PLATFORMS: usize = 5;

const EXPECTED_FIXTURE_FINDINGS: [&str; 5] = [
    "input \"runner\" is required by \"./.github/workflows/reusable.yml\" reusable workflow [workflow-call]",
    "input \"platform\" is not defined in \"./.github/workflows/reusable.yml\" reusable workflow",
    "could not read reusable workflow file for \"./.github/workflows/absent.yml\"",
    "got unexpected character",
    "shellcheck reported issue in this script: SC2086:",
];

/// Returns the pinned tool one env var names.
fn tool(variable: &str) -> PathBuf {
    let value =
        std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name the pinned tool"));
    let path = resolve_runfiles(&value);
    assert!(
        path.is_file(),
        "{variable} names {} which is absent",
        path.display()
    );
    path
}

/// Returns one staged project root holding every named file at its own path.
fn stage(files: &[(String, String)]) -> PathBuf {
    let root = mkscratch("actionlint").expect("scratch project root");
    std::fs::create_dir_all(root.join(PROJECT_MARKER)).expect("project marker");
    for (source, target) in files {
        let path = root.join(target);
        std::fs::create_dir_all(path.parent().expect("staged parent")).expect("staged directory");
        std::fs::copy(runfiles_root().join(source), &path).expect("staged workflow");
    }
    root
}

/// Returns one actionlint run over the named workflows of one staged project.
fn lint(root: &Path, workflows: &[String]) -> Output {
    let mut command = Command::new(tool("DX_ACTIONLINT"));
    command.current_dir(root);
    command.arg("-config-file").arg(root.join(CONFIG));
    command.arg("-shellcheck").arg(tool("DX_SHELLCHECK"));
    command.arg("-pyflakes=");
    command.arg("-no-color");
    command.arg("-oneline");
    for workflow in workflows {
        command.arg(workflow);
    }
    command.output().expect("actionlint runs")
}

/// Returns the findings one actionlint run printed, one per line.
fn findings(run: &Output) -> Vec<String> {
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// Returns the shipped workflows and consumer starters as runfiles to staged paths.
fn shipped() -> Vec<(String, String)> {
    let mut files = vec![(CONFIG.to_owned(), CONFIG.to_owned())];
    for workflow in dx_testing::workflow_files() {
        files.push((workflow.clone(), workflow));
    }
    for caller in CALLERS {
        files.push((caller.to_owned(), caller.to_owned()));
    }
    files
}

/// Returns the invalid fixture as runfiles to staged paths.
fn fixture() -> Vec<(String, String)> {
    let mut files = vec![(CONFIG.to_owned(), CONFIG.to_owned())];
    for name in FIXTURE_WORKFLOWS {
        files.push((
            format!("{FIXTURE_DIR}/{WORKFLOWS}/{name}"),
            format!("{WORKFLOWS}/{name}"),
        ));
    }
    files
}

/// Returns the staged paths every file list names, without the config.
fn workflows(files: &[(String, String)]) -> Vec<String> {
    files
        .iter()
        .map(|(_, target)| target.clone())
        .skip(1)
        .collect()
}

#[test]
fn every_shipped_workflow_passes_actionlint() {
    let files = shipped();
    let targets = workflows(&files);
    let exported = dx_testing::workflow_files().len();
    assert_eq!(
        targets.len(),
        exported + CALLERS.len(),
        "the sweep must cover every exported workflow plus both consumer starters"
    );
    let root = stage(&files);
    let run = lint(&root, &targets);
    assert!(
        run.status.success(),
        "actionlint rejected {} workflows:\n{}",
        targets.len(),
        String::from_utf8_lossy(&run.stdout)
    );
}

#[test]
fn the_invalid_fixture_reports_every_semantic_failure() {
    let files = fixture();
    let targets = workflows(&files);
    let root = stage(&files);
    let run = lint(&root, &targets);
    let reported = findings(&run);
    assert!(
        !run.status.success(),
        "actionlint passed the invalid fixture:\n{}",
        String::from_utf8_lossy(&run.stdout)
    );
    assert_eq!(
        reported.len(),
        EXPECTED_FIXTURE_FINDINGS.len(),
        "the invalid fixture must report exactly its own failures:\n{}",
        reported.join("\n")
    );
    for expected in EXPECTED_FIXTURE_FINDINGS {
        assert!(
            reported.iter().any(|line| line.contains(expected)),
            "the invalid fixture never reported {expected:?}:\n{}",
            reported.join("\n")
        );
    }
}

/// Returns the runner labels the actionlint config declares, sorted.
fn configured_runners() -> Vec<String> {
    let text = read_runfiles(CONFIG);
    let document: serde_json::Value = yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{CONFIG} is not valid YAML: {error}"));
    let labels = document["self-hosted-runner"]["labels"]
        .as_array()
        .unwrap_or_else(|| panic!("{CONFIG} declares no self-hosted-runner labels"));
    let mut runners: Vec<String> = labels
        .iter()
        .map(|label| {
            label
                .as_str()
                .unwrap_or_else(|| panic!("{CONFIG} holds a label that is not a string"))
                .to_owned()
        })
        .collect();
    runners.sort();
    runners
}

/// Returns the platform table, platform to runner label, that `input_validation.py` declares.
fn platform_table() -> BTreeMap<String, String> {
    let text = read_runfiles(PLATFORM_RUNNERS);
    let start = text
        .find("PLATFORMS = {")
        .unwrap_or_else(|| panic!("{PLATFORM_RUNNERS} declares no PLATFORMS table"));
    let table = &text[start..];
    let end = table
        .find("\n}")
        .unwrap_or_else(|| panic!("{PLATFORM_RUNNERS} leaves the PLATFORMS table open"));
    table[..end]
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once("\": ")?;
            Some((
                key.trim().trim_matches('"').to_owned(),
                value.trim_end_matches(',').trim_matches('"').to_owned(),
            ))
        })
        .collect()
}

/// Returns the runner labels the platform table maps every supported platform to.
fn platform_runners() -> Vec<String> {
    let mut runners: Vec<String> = platform_table().into_values().collect();
    runners.sort();
    runners
}

/// Returns one problem per literal matrix runner in `text` that disagrees with `table`,
/// and how many literal matrix runners it checked.
fn matrix_runner_problems(
    name: &str,
    text: &str,
    table: &BTreeMap<String, String>,
) -> (Vec<String>, usize) {
    let document: serde_json::Value = yaml_serde::from_str(text)
        .unwrap_or_else(|error| panic!("{name} is not valid YAML: {error}"));
    let mut problems = Vec::new();
    let mut checked = 0;
    let Some(jobs) = document["jobs"].as_object() else {
        return (problems, checked);
    };
    for (job, body) in jobs {
        let Some(include) = body["strategy"]["matrix"]["include"].as_array() else {
            continue;
        };
        for entry in include {
            let Some(runner) = entry["runner"].as_str() else {
                continue;
            };
            if runner.starts_with("${{") {
                continue;
            }
            checked += 1;
            match entry["platform"].as_str() {
                Some(platform) => match table.get(platform) {
                    Some(expected) if expected == runner => {}
                    Some(expected) => problems.push(format!(
                        "{name} job {job}: {platform} runs on {runner}, but {PLATFORM_RUNNERS} maps it to {expected}"
                    )),
                    None => problems.push(format!(
                        "{name} job {job}: {platform} is not a platform in {PLATFORM_RUNNERS}"
                    )),
                },
                None if table.values().any(|label| label == runner) => {}
                None => problems.push(format!(
                    "{name} job {job}: runner {runner} is not a runner in {PLATFORM_RUNNERS}"
                )),
            }
        }
    }
    (problems, checked)
}

#[test]
fn every_matrix_runner_is_a_platform_runner() {
    let table = platform_table();
    let mut problems = Vec::new();
    let mut checked = 0;
    for workflow in dx_testing::workflow_files() {
        let (found, count) = matrix_runner_problems(&workflow, &read_runfiles(&workflow), &table);
        problems.extend(found);
        checked += count;
    }
    assert!(
        checked > 0,
        "no workflow matrix names a literal runner; the parity check checked nothing"
    );
    assert!(
        problems.is_empty(),
        "workflow matrix runners drifted from {PLATFORM_RUNNERS}:\n{}",
        problems.join("\n")
    );
}

#[test]
fn a_retired_matrix_runner_is_caught() {
    let text = "jobs:\n  build:\n    strategy:\n      matrix:\n        include:\n          - platform: macos_arm64\n            runner: macos-14\n          - name: dx-macos-arm64\n            runner: macos-14\n";
    let (problems, checked) = matrix_runner_problems("retired.yml", text, &platform_table());
    assert_eq!(checked, 2);
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].contains("macos_arm64 runs on macos-14"), "{problems:?}");
    assert!(problems[1].contains("runner macos-14 is not a runner"), "{problems:?}");
}

#[test]
fn the_actionlint_config_names_every_platform_runner() {
    let runners = platform_runners();
    assert_eq!(
        runners.len(),
        SUPPORTED_PLATFORMS,
        "{PLATFORM_RUNNERS} must map every supported platform"
    );
    assert_eq!(
        configured_runners(),
        runners,
        "{CONFIG} must accept every runner the matrix runs on"
    );
}
