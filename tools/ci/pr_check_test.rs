use ci_workflow::{job, steps};
use dx_testing::read_runfiles;

const CI: &str = ".github/workflows/ci.yml";
const WORKFLOW: &str = ".github/workflows/pr-check.yml";
const SCRIPT: &str = "tools/ci/pr_check.sh";
const SETS: [&str; 7] = [
    "workflow",
    "preflight",
    "starter",
    "hermetic",
    "docs",
    "families",
    "cli",
];

/// Returns every `//` label in the text.
fn labels(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .filter(|word| {
            word.starts_with("//") && word.contains(':')
                || word.starts_with("//") && word.ends_with("/...")
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn script_covers_the_ci_preflight_labels() {
    let script = read_runfiles(SCRIPT);
    let preflight = job(CI, "preflight");
    let matrix = preflight["strategy"]["matrix"]["include"]
        .as_array()
        .expect("preflight matrix");
    let linux = matrix
        .iter()
        .find(|cell| cell["platform"] == "linux_x86_64")
        .expect("linux_x86_64 cell");
    let mut wanted = labels(linux["quality"].as_str().expect("quality labels"));
    for step in steps(CI, "preflight") {
        let run = step["run"].as_str().unwrap_or_default();
        if run.contains("bazel build --noshow_progress")
            || run.contains("bazel test --noshow_progress")
        {
            wanted.extend(labels(run));
        }
    }
    assert!(wanted.len() > 30, "found only {} labels", wanted.len());
    let missing: Vec<_> = wanted
        .iter()
        .filter(|label| !script.contains(label.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "{SCRIPT} lacks the ci.yml preflight labels {missing:?}"
    );
}

#[test]
fn starter_set_runs_the_golden_presence_test() {
    let script = read_runfiles(SCRIPT);
    let starter = script
        .split("starter() {")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("starter set");
    assert!(
        starter.contains("bazel test \"$@\" //tools/testing/starters:starter_goldens_test"),
        "starter set does not run starter_goldens_test: {starter}"
    );
}

#[test]
fn workflow_runs_every_set_in_its_own_step() {
    let names: Vec<String> = steps(WORKFLOW, "analysis")
        .iter()
        .filter(|step| step["run"].is_string())
        .map(|step| step["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, SETS);
    for step in steps(WORKFLOW, "analysis") {
        if let Some(run) = step["run"].as_str() {
            let name = step["name"].as_str().unwrap_or_default();
            assert!(
                run.contains(&format!("{SCRIPT} {name} ")),
                "step {name} runs {run}"
            );
        }
    }
    let script = read_runfiles(SCRIPT);
    for set in SETS {
        assert!(
            script.contains(&format!("{set}()")),
            "{SCRIPT} has no {set} set"
        );
    }
}

#[test]
fn job_stays_inside_the_time_budget() {
    let analysis = job(WORKFLOW, "analysis");
    let minutes = analysis["timeout-minutes"]
        .as_u64()
        .expect("timeout-minutes");
    assert!(minutes <= 15, "timeout-minutes is {minutes}");
}
