use ci_workflow::{job, text_at};
use serde_json::Value;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const MATRIX_JOB: &str = "checks";

#[cfg(unix)]
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

#[cfg(unix)]
const CASES: [(&str, &str); 4] = [
    ("", "<!-- dx-coverage-summary: coverage linux_x86_64 -->"),
    ("b", "<!-- dx-coverage-summary: coverage linux_x86_64 b -->"),
    (
        "bb",
        "<!-- dx-coverage-summary: coverage linux_x86_64 bb -->",
    ),
    ("c", "<!-- dx-coverage-summary: coverage linux_x86_64 c -->"),
];

#[cfg(unix)]
fn coverage_env(
    root: &std::path::Path,
    runner: &std::path::Path,
    invocation: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("RUNNER_TEMP", runner.display().to_string()),
        (
            "GITHUB_STEP_SUMMARY",
            root.join("summary.md").display().to_string(),
        ),
        ("GITHUB_SHA", "abc123".to_owned()),
        ("GITHUB_SERVER_URL", "https://github.example".to_owned()),
        ("GITHUB_REPOSITORY", "devhusdk/rules_dx".to_owned()),
        ("GITHUB_RUN_ID", "1".to_owned()),
        ("DX_MIN_CHANGED_LINE_COVERAGE", String::new()),
        ("DX_INVOCATION_ID", invocation.to_owned()),
        ("PLATFORM", "linux_x86_64".to_owned()),
    ]
}

#[cfg(unix)]
fn run_render(runner: &std::path::Path, script: &str, env: &[(&str, String)]) {
    let mut command = std::process::Command::new("bash");
    command
        .args(["-e", "-c", script])
        .current_dir(runner)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run bash");
    assert!(
        done.status.success(),
        "render step failed: {}",
        String::from_utf8_lossy(&done.stderr)
    );
}

#[cfg(unix)]
#[test]
fn each_invocation_has_its_own_coverage_marker() {
    let render = text_at(
        &coverage_step("Render first-party coverage summary")["run"],
        "render step run",
    );
    let scratch = dx_test_scratch::scratch("coverage-marker");
    let root = scratch.path();
    let mut bodies = Vec::new();
    for (index, &(invocation, marker)) in CASES.iter().enumerate() {
        let runner = root.join(format!("runner-{index}"));
        std::fs::create_dir_all(&runner).expect("create runner dir");
        run_render(&runner, &render, &coverage_env(root, &runner, invocation));
        let body = std::fs::read_to_string(runner.join("coverage_comment.md"))
            .expect("render writes the comment");
        assert_eq!(
            body.lines().next(),
            Some(marker),
            "marker for {invocation:?}"
        );
        bodies.push(body);
    }
    // The publish step finds a comment by substring match on its marker, so each
    // marker must appear in its own comment and in no other invocation's comment.
    for (index, &(invocation, marker)) in CASES.iter().enumerate() {
        for (other, body) in bodies.iter().enumerate() {
            assert_eq!(
                body.contains(marker),
                other == index,
                "{invocation:?} marker found in comment {other}"
            );
        }
    }
}
