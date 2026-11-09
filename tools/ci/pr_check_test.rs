use ci_workflow::{job, steps};
use dx_testing::read_runfiles;

const CI: &str = ".github/workflows/ci.yml";
const WORKFLOW: &str = ".github/workflows/pr-check.yml";
const SCRIPT: &str = "tools/ci/pr_check.sh";
const SETS: [&str; 8] = [
    "workflow",
    "preflight",
    "preflight-windows-arm64",
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
fn workflow_set_runs_the_action_and_workflow_tests() {
    let script = read_runfiles(SCRIPT);
    let body = script
        .split("workflow() {")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("workflow set");
    for label in [
        "//tools/ci:secret_flags_test",
        "//tools/ci:workflow_pins_test",
    ] {
        assert!(body.contains(label), "workflow set does not run {label}");
    }
}

#[test]
fn windows_arm64_set_targets_the_platform() {
    let script = read_runfiles(SCRIPT);
    let start = script
        .find("preflight-windows-arm64() {")
        .expect("windows_arm64 set");
    let body = &script[start..];
    let body = &body[..body.find("\n}\n").expect("end of set")];
    assert!(body.contains("--platforms=@llvm//platforms:windows_aarch64"));
    assert!(body.contains("--skip_incompatible_explicit_targets"));
    assert!(body.contains("//dart/tools:all"));
}

#[test]
fn windows_arm64_set_or_blocklist_covers_ci_preflight() {
    let script = read_runfiles(SCRIPT);
    let start = script
        .find("preflight-windows-arm64() {")
        .expect("windows_arm64 set");
    let body = &script[start..];
    let body = &body[..body.find("\n}\n").expect("end of set")];
    let blocked = script
        .lines()
        .find(|line| line.starts_with("WINDOWS_ARM64_BLOCKED="))
        .expect("WINDOWS_ARM64_BLOCKED line");
    let mut wanted = Vec::new();
    for step in steps(CI, "preflight") {
        let run = step["run"].as_str().unwrap_or_default();
        if run.contains("bazel build --noshow_progress")
            || run.contains("bazel test --noshow_progress")
        {
            wanted.extend(labels(run));
        }
    }
    let matrix = job(CI, "preflight")["strategy"]["matrix"]["include"]
        .as_array()
        .expect("preflight matrix")
        .clone();
    let cell = matrix
        .iter()
        .find(|cell| cell["platform"] == "windows_arm64")
        .expect("windows_arm64 cell");
    wanted.extend(labels(cell["quality"].as_str().expect("quality labels")));
    assert!(wanted.len() > 10, "found only {} labels", wanted.len());
    let uncovered: Vec<_> = wanted
        .iter()
        .filter(|label| !labels(body).contains(label) && !labels(blocked).contains(label))
        .collect();
    assert!(
        uncovered.is_empty(),
        "preflight-windows-arm64 and WINDOWS_ARM64_BLOCKED lack {uncovered:?}"
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

#[test]
fn workflow_runs_on_pull_requests() {
    let body = read_runfiles(WORKFLOW);
    assert!(
        body.contains("\n  pull_request:\n"),
        "{WORKFLOW} needs a pull_request trigger"
    );
    assert!(
        !body.contains("pull_request_target"),
        "{WORKFLOW} must not use pull_request_target"
    );
}

#[test]
fn consumer_queries_skip_the_private_upstream_copies() {
    for path in [CI, SCRIPT] {
        let text = read_runfiles(path);
        let mut seen = 0;
        for line in text.lines() {
            for tag in ["starter-consumer", "hermetic-consumer"] {
                if line.contains(&format!("attr(tags, {tag}")) && line.contains("bazel query") {
                    seen += 1;
                    assert!(
                        line.contains("except filter(\"_upstream$\", //...)"),
                        "{path}: query for {tag} keeps _upstream copies: {line}"
                    );
                }
            }
        }
        assert!(seen > 0, "{path} has no consumer query");
    }
}
