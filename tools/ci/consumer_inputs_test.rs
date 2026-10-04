use serde_json::{json, Value};

use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const VALIDATE: &str = "validate";

const AGGREGATE: &str = "dx-ci";

const CHECKS: [&str; 9] = [
    "lint",
    "typecheck",
    "format",
    "generate",
    "security-audit",
    "license-audit",
    "test",
    "build",
    "coverage",
];

const PLATFORMS_REF: &str = "needs.validate.outputs.platforms";

const RUNNERS_REF: &str = "needs.validate.outputs.runners";

const DISABLED_REF: &str = "needs.validate.outputs.disabled";

const DISABLED: &str = "${{ needs.validate.outputs.disabled }}";

const THRESHOLD: &str = "${{ needs.validate.outputs.min_coverage }}";

const MATRIX: &str = "${{ fromJSON(needs.validate.outputs.platforms) }}";

const RUNNER: &str = "${{ fromJSON(needs.validate.outputs.runners)[matrix.platform] }}";

const RAW_INPUTS: [&str; 3] = [
    "inputs.platforms",
    "inputs.disabled_checks",
    "inputs.min_coverage",
];

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> Value {
    let text = read_runfiles(WORKFLOW);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
}

/// Returns the job names the parsed workflow declares, sorted.
fn job_names() -> Vec<String> {
    let document = document();
    let jobs = document["jobs"]
        .as_object()
        .unwrap_or_else(|| panic!("{WORKFLOW} has no jobs mapping"));
    let mut names: Vec<String> = jobs.keys().cloned().collect();
    names.sort();
    names
}

/// Returns one job of the parsed workflow.
fn job(name: &str) -> Value {
    let found = document()["jobs"][name].clone();
    assert!(
        found.is_object(),
        "{WORKFLOW} names no {name:?} job: {}",
        document()["jobs"]
    );
    found
}

/// Returns one string value of the parsed workflow.
fn text_at(value: &Value, what: &str) -> String {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{what} is not a string: {value}"))
        .to_owned()
}

#[test]
fn the_workflow_is_valid_yaml_and_names_only_the_expected_jobs() {
    let mut wanted: Vec<String> = std::iter::once(VALIDATE.to_owned())
        .chain(CHECKS.iter().map(|name| (*name).to_owned()))
        .chain(std::iter::once(AGGREGATE.to_owned()))
        .collect();
    wanted.sort();
    assert_eq!(
        job_names(),
        wanted,
        "{WORKFLOW} must name the validate job, the nine checks and the aggregate"
    );
}

#[test]
fn the_validate_job_needs_nothing_so_it_rejects_input_first() {
    let validate = job(VALIDATE);
    assert!(
        validate.get("needs").is_none(),
        "{WORKFLOW} job {VALIDATE} must declare no needs, so it runs before every matrix job"
    );
    let script = text_at(&validate["steps"][0]["run"], "validate steps.0.run");
    assert!(
        script.contains("DX_INPUT_VALIDATION"),
        "{WORKFLOW} job {VALIDATE} runs no DX_INPUT_VALIDATION validator"
    );
}

#[test]
fn every_check_job_waits_for_the_validated_inputs() {
    for name in CHECKS {
        assert_eq!(
            job(name)["needs"],
            json!([VALIDATE]),
            "{WORKFLOW} job {name} must need the validate job"
        );
        let gate = text_at(&job(name)["if"], &format!("{name} if"));
        assert!(
            gate.contains(&format!("contains(format(',{{0}},', {DISABLED_REF})")),
            "{WORKFLOW} job {name} must gate on {DISABLED_REF}: {gate}"
        );
        assert!(
            gate.contains(&format!(",{name},")),
            "{WORKFLOW} job {name} must gate on its own check ID: {gate}"
        );
    }
}

#[test]
fn every_check_job_expands_its_matrix_from_the_validated_platforms() {
    for name in CHECKS {
        let job = job(name);
        assert_eq!(
            job["strategy"]["matrix"]["platform"],
            json!(MATRIX),
            "{WORKFLOW} job {name} must expand from {PLATFORMS_REF}"
        );
        assert_eq!(
            job["runs-on"],
            json!(RUNNER),
            "{WORKFLOW} job {name} must pick its runner from {RUNNERS_REF}"
        );
    }
}

#[test]
fn the_coverage_job_takes_its_threshold_from_the_validated_inputs() {
    let coverage = job("coverage");
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job coverage has no steps"));
    let thresholds = steps
        .iter()
        .filter(|step| step["env"]["DX_MIN_COVERAGE"] == json!(THRESHOLD))
        .count();
    assert_eq!(
        thresholds, 1,
        "{WORKFLOW} job coverage must pass the validated threshold exactly once"
    );
}

#[test]
fn the_aggregate_separates_a_disabled_check_from_a_lost_one() {
    let aggregate = job(AGGREGATE);
    let wanted: Vec<&str> = std::iter::once(VALIDATE)
        .chain(CHECKS.iter().copied())
        .collect();
    assert_eq!(aggregate["needs"], json!(wanted));
    let step = aggregate["steps"][0].clone();
    assert_eq!(step["env"]["DISABLED"], json!(DISABLED));
    let script = text_at(&step["run"], "aggregate steps.0.run");
    for needle in [
        "os.environ[\"DISABLED\"]",
        "result == \"skipped\" and name not in disabled",
        "skipped without disabled_checks",
        "results.get(\"validate\") != \"success\"",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

#[test]
fn no_matrix_expands_from_a_raw_workflow_input() {
    let text = read_runfiles(WORKFLOW);
    for needle in RAW_INPUTS {
        let uses: Vec<&str> = text.lines().filter(|line| line.contains(needle)).collect();
        assert_eq!(
            uses.len(),
            1,
            "{WORKFLOW} reads {needle} outside the validate job: {uses:?}"
        );
        assert!(
            uses[0].contains("DX_"),
            "{WORKFLOW} must read {needle} only as a validate job environment value: {uses:?}"
        );
    }
}
