#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dx_testing::runfiles_root;

const CONFIG: &str = ".github/actionlint.yaml";

const PROJECT_MARKER: &str = ".git";

const WORKFLOWS: &str = ".github/workflows";

const CALLERS: [&str; 1] = ["examples/docs-ci/caller.yml"];

const FIXTURE_DIR: &str = "tools/ci/actionlint_fixtures";

const FIXTURE_WORKFLOWS: [&str; 2] = ["invalid.yml", "reusable.yml"];

const OVERSIZED_HEADER: &str =
    "on: push\njobs:\n  oversized:\n    runs-on: ubuntu-latest\n    steps:\n      - run: |\n";

const OVERSIZED_SCRIPT_LINES: usize = 2048;

const LARGEST_PIPE_BUFFER: usize = 64 * 1024;

const EXPECTED_FIXTURE_FINDINGS: [&str; 5] = [
    "input \"runner\" is required by \"./.github/workflows/reusable.yml\" reusable workflow [workflow-call]",
    "input \"platform\" is not defined in \"./.github/workflows/reusable.yml\" reusable workflow",
    "could not read reusable workflow file for \"./.github/workflows/absent.yml\"",
    "got unexpected character",
    "shellcheck reported issue in this script: SC2086:",
];

fn tool(variable: &str) -> PathBuf {
    let path = dx_testing::env_file(variable);
    assert!(
        path.is_file(),
        "{variable} names {} which is absent",
        path.display()
    );
    path
}

fn stage(files: &[(String, String)]) -> dx_test_scratch::TempDir {
    let staged = dx_test_scratch::scratch("actionlint").unwrap();
    let root = staged.path();
    std::fs::create_dir_all(root.join(PROJECT_MARKER)).expect("project marker");
    for (source, target) in files {
        let path = root.join(target);
        std::fs::create_dir_all(path.parent().expect("staged parent")).expect("staged directory");
        std::fs::copy(runfiles_root().join(source), &path).expect("staged workflow");
    }
    staged
}

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

fn findings(run: &Output) -> Vec<String> {
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

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
    let staged = stage(&files);
    let root = staged.path();
    let run = lint(root, &targets);
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
    let staged = stage(&files);
    let root = staged.path();
    let run = lint(root, &targets);
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

fn oversized_script() -> String {
    let mut lines: Vec<String> = (0..OVERSIZED_SCRIPT_LINES)
        .map(|line| {
            format!("          echo \"line {line} pads the script past every pipe buffer\"")
        })
        .collect();
    lines.push("          echo $HOME".to_owned());
    lines.join("\n")
}

#[test]
fn a_run_script_larger_than_a_pipe_buffer_reaches_shellcheck() {
    let script = oversized_script();
    assert!(
        script.len() > LARGEST_PIPE_BUFFER,
        "the script must overrun the pipe buffer on every platform"
    );
    let staged = stage(&[(CONFIG.to_owned(), CONFIG.to_owned())]);
    let root = staged.path();
    let target = format!("{WORKFLOWS}/oversized.yml");
    std::fs::create_dir_all(root.join(WORKFLOWS)).expect("workflow directory");
    std::fs::write(root.join(&target), format!("{OVERSIZED_HEADER}{script}\n"))
        .expect("oversized workflow");
    let run = lint(root, &[target]);
    let reported = findings(&run);
    assert!(
        matches!(
            reported.as_slice(),
            [only] if only.contains("shellcheck reported issue in this script: SC2086:")
        ),
        "shellcheck must report only the unquoted expansion at the end of the oversized script:\n{}",
        reported.join("\n")
    );
}
