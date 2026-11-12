const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

fn prepare_script() -> String {
    let document = ci_workflow::document(WORKFLOW);
    let steps = document["jobs"]["advisory-snapshots"]["steps"]
        .as_array()
        .expect("steps")
        .clone();
    let step = steps
        .iter()
        .find(|step| step["id"] == "prepare")
        .expect("prepare step");
    step["run"].as_str().expect("run script").to_owned()
}

#[test]
fn the_prepare_step_is_one_advisory_prep_run() {
    let script = prepare_script();
    let commands: Vec<&str> = script
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    assert_eq!(
        commands
            .iter()
            .filter(|line| line.contains("bazel "))
            .count(),
        1,
        "{script}"
    );
    for needle in [
        "bazel run --lockfile_mode=error",
        "@rules_dx//cli/advisory_prep",
        "--ci",
        "--workspace \"$GITHUB_WORKSPACE\"",
        "--out \"$GITHUB_WORKSPACE/.dx/local/advisory\"",
        "--status-file \"$GITHUB_OUTPUT\"",
    ] {
        assert!(script.contains(needle), "{needle} missing:\n{script}");
    }
    for logic in [
        "git ",
        "grep",
        "date ",
        "bazel build",
        "bazel cquery",
        "if ",
    ] {
        assert!(
            !script.contains(logic),
            "{logic} left in the step:\n{script}"
        );
    }
}
