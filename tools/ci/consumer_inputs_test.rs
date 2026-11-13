use ci_workflow::{document, jobs_of, steps_of};
#[cfg(unix)]
use ci_workflow::{job, text_at};
use serde_json::Value;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

#[cfg(unix)]
const MATRIX_JOB: &str = "checks";

fn coverage_step(prefix: &str) -> Value {
    let coverage = job(WORKFLOW, MATRIX_JOB);
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job checks has no steps"))
        .clone();
    steps
        .into_iter()
        .find(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} job checks has no step named {prefix:?}"))
}

#[test]
fn the_render_step_runs_coverage_summary_for_its_cell() {
    let step = coverage_step("Render first-party coverage summary");
    let render = text_at(&step["run"], "render step run");
    let words: Vec<&str> = render
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter(|word| *word != "\\")
        .collect();
    assert_eq!(
        words,
        [
            "bazel",
            "run",
            "--lockfile_mode=error",
            "$BAZEL_CONFIG",
            "$BB_ARGS",
            "@rules_dx//cli/coverage_summary",
            "--",
            "--runner-temp",
            "\"$RUNNER_TEMP\"",
            "--step-summary",
            "\"$GITHUB_STEP_SUMMARY\"",
            "--platform",
            "\"$PLATFORM\"",
            "--invocation-id",
            "\"$DX_INVOCATION_ID\"",
            "--min-changed-line-coverage",
            "\"$DX_MIN_CHANGED_LINE_COVERAGE\"",
            "--sha",
            "\"$GITHUB_SHA\"",
            "--server-url",
            "\"$GITHUB_SERVER_URL\"",
            "--repository",
            "\"$GITHUB_REPOSITORY\"",
            "--run-id",
            "\"$GITHUB_RUN_ID\"",
        ],
        "{WORKFLOW} render step must only run coverage_summary"
    );
    assert_eq!(
        step["env"]["DX_INVOCATION_ID"], "${{ inputs.invocation_id }}",
        "the cell marker carries the invocation id"
    );
    assert_eq!(step["env"]["PLATFORM"], "${{ matrix.platform }}");
    assert_eq!(
        step["env"]["DX_MIN_CHANGED_LINE_COVERAGE"],
        "${{ needs.validate.outputs.min_changed_line_coverage }}"
    );
    assert_eq!(step["if"], "${{ always() }}");
}

#[test]
fn piped_status_capture_steps_run_under_bash() {
    let workflow = document(WORKFLOW);
    let mut checked = 0;
    for (id, job) in jobs_of(&workflow) {
        let default = job
            .pointer("/defaults/run/shell")
            .or_else(|| workflow.pointer("/defaults/run/shell"))
            .and_then(Value::as_str);
        for step in steps_of(job) {
            let Some(run) = step["run"].as_str() else {
                continue;
            };
            let captures = run
                .lines()
                .any(|line| line.contains(" | ") && line.contains("|| status=$?"));
            if !captures {
                continue;
            }
            checked += 1;
            let shell = step["shell"].as_str().or(default);
            assert_eq!(
                shell,
                Some("bash"),
                "{WORKFLOW} job {id} step {:?} pipes into a status capture without pipefail",
                step["name"]
            );
        }
    }
    assert!(checked > 0, "{WORKFLOW} has no piped status-capture step");
}
