use serde_json::{json, Value};

use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const PREP: &str = "advisory-snapshots";

const AUDIT: &str = "security-audit";

const VALIDATE: &str = "validate";

const ARTIFACT: &str = "advisory-snapshots";

const RETRIEVED_AT: &str = "${{ needs.advisory-snapshots.outputs.retrieved_at }}";

const PREP_TARGET: &str = "@rules_dx//cli/advisory_prep";

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> Value {
    let text = read_runfiles(WORKFLOW);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
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

/// Returns the steps of one job, which must declare some.
fn steps(name: &str) -> Vec<Value> {
    let found = job(name)["steps"].as_array().cloned();
    found.unwrap_or_else(|| panic!("{WORKFLOW} job {name} declares no steps"))
}

/// Returns the `uses` action of every step that names one.
fn actions(name: &str) -> Vec<String> {
    steps(name)
        .iter()
        .filter_map(|step| step["uses"].as_str().map(str::to_owned))
        .collect()
}

/// Returns the `run` scripts of one job, joined.
fn scripts(name: &str) -> String {
    steps(name)
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the one step of a job that uses `action`, with its pin.
fn action_step<'a>(steps: &'a [Value], action: &str) -> &'a Value {
    let found: Vec<&Value> = steps
        .iter()
        .filter(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with(action))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{WORKFLOW} must run {action} exactly once, found {}",
        found.len()
    );
    found[0]
}

/// Returns the `if` gate of one job.
fn gate_of(name: &str) -> String {
    job(name)["if"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn one_preparation_job_runs_per_run_and_not_per_platform() {
    let prep = job(PREP);
    assert_eq!(prep["needs"], json!([VALIDATE]));
    assert!(
        prep.get("strategy").is_none(),
        "{WORKFLOW} job {PREP} must run once, so it expands no platform matrix"
    );
    assert_eq!(prep["runs-on"], json!("ubuntu-latest"));
    assert_eq!(prep["permissions"], json!({"contents": "read"}));
    let gate = gate_of(PREP);
    assert!(
        gate.contains(&format!(",{AUDIT},")),
        "{WORKFLOW} job {PREP} must gate on its consumer: {gate}"
    );
    assert!(
        gate.contains("needs.validate.outputs.disabled"),
        "{WORKFLOW} job {PREP} must gate on the validated disabled checks: {gate}"
    );
}

/// Returns the one run script of a job that mentions `needle`.
fn script_running(name: &str, needle: &str) -> String {
    let found = steps(name);
    let found: Vec<&Value> = found
        .iter()
        .filter(|step| {
            step["run"]
                .as_str()
                .is_some_and(|script| script.contains(needle))
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "{WORKFLOW} job {name} must run {needle} exactly once, found {}",
        found.len()
    );
    text_at(&found[0]["run"], &format!("{name} run"))
}

#[test]
fn the_preparation_runs_the_pinned_native_command_once() {
    let script = script_running(PREP, PREP_TARGET);
    for needle in ["--out .dx/advisory", "--date \"$retrieved_at\""] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {PREP} must pass {needle}: {script}"
        );
    }
    for absent in ["python3", "curl", "all.zip", "osv-vulnerabilities"] {
        assert!(
            !script.contains(absent),
            "{WORKFLOW} job {PREP} must run no {absent} of its own: {script}"
        );
    }
    assert!(
        !scripts(PREP).contains("osv-vulnerabilities"),
        "{WORKFLOW} job {PREP} names no advisory database URL: {}",
        scripts(PREP)
    );
    assert!(
        !scripts(PREP).contains("secrets."),
        "{WORKFLOW} job {PREP} must need no secret, so fork pull requests run it"
    );
}

#[test]
fn the_preparation_publishes_one_run_artifact() {
    let prep = steps(PREP);
    let upload = action_step(&prep, "actions/upload-artifact");
    assert_eq!(upload["with"]["name"], json!(ARTIFACT));
    assert_eq!(upload["with"]["path"], json!(".dx/advisory"));
    assert_eq!(upload["with"]["if-no-files-found"], json!("error"));
}

#[test]
fn the_preparation_stamps_every_snapshot_with_the_run_date() {
    assert_eq!(
        job(PREP)["outputs"]["retrieved_at"],
        json!("${{ steps.prepare.outputs.retrieved_at }}"),
        "{WORKFLOW} job {PREP} must publish the date it stamped"
    );
    let prep = steps(PREP);
    let dated: Vec<&Value> = prep
        .iter()
        .filter(|step| step["id"] == json!("prepare"))
        .collect();
    assert_eq!(
        dated.len(),
        1,
        "{WORKFLOW} job {PREP} must name one step prepare"
    );
    let script = text_at(&dated[0]["run"], "prepare run");
    assert!(
        script.contains("date -u +%F"),
        "{WORKFLOW} job {PREP} pins one UTC date per run: {script}"
    );
    assert!(
        script.contains("\"retrieved_at=$retrieved_at\" >> \"$GITHUB_OUTPUT\""),
        "{WORKFLOW} job {PREP} must publish the date it pinned: {script}"
    );
}

/// Returns one string value of the parsed workflow.
fn text_at(value: &Value, what: &str) -> String {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{what} is not a string: {value}"))
        .to_owned()
}

#[test]
fn every_audit_cell_restores_that_artifact_instead_of_downloading() {
    let audit = steps(AUDIT);
    let download = action_step(&audit, "actions/download-artifact");
    assert_eq!(download["with"]["name"], json!(ARTIFACT));
    assert_eq!(download["with"]["path"], json!(".dx/advisory"));
    let script = script_running(AUDIT, "@rules_dx//:dx -- security");
    for absent in [
        "curl",
        "all.zip",
        "osv-vulnerabilities",
        "date -u",
        "GITHUB_OUTPUT",
    ] {
        assert!(
            !script.contains(absent),
            "{WORKFLOW} job {AUDIT} must run no {absent} of its own: {script}"
        );
    }
    assert!(
        script.contains("@rules_dx//:dx -- security //..."),
        "{WORKFLOW} job {AUDIT} must still audit every scope: {script}"
    );
    assert!(
        !scripts(AUDIT).contains("osv-vulnerabilities"),
        "{WORKFLOW} job {AUDIT} names no advisory database URL: {}",
        scripts(AUDIT)
    );
}

#[test]
fn every_audit_cell_judges_freshness_against_the_shared_date() {
    let audit = steps(AUDIT);
    let dated: Vec<&Value> = audit
        .iter()
        .filter(|step| step["env"].get("DX_AUDIT_TODAY").is_some())
        .collect();
    assert_eq!(
        dated.len(),
        1,
        "{WORKFLOW} job {AUDIT} must pass the prepared date exactly once"
    );
    assert_eq!(dated[0]["env"]["DX_AUDIT_TODAY"], json!(RETRIEVED_AT));
    assert!(
        dated[0]["run"]
            .as_str()
            .unwrap_or_default()
            .contains("security"),
        "{WORKFLOW} job {AUDIT} must date the audit itself: {}",
        dated[0]["run"]
    );
}

#[test]
fn a_failed_preparation_skips_the_audit_and_fails_the_aggregate() {
    assert_eq!(
        job(AUDIT)["needs"],
        json!([VALIDATE, PREP]),
        "{WORKFLOW} job {AUDIT} must wait for the prepared snapshots"
    );
    let aggregate_needs: Vec<String> = document()["jobs"]["dx-ci"]["needs"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job dx-ci declares no needs"))
        .iter()
        .map(|name| name.as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        aggregate_needs.contains(&PREP.to_owned()),
        "{WORKFLOW} job dx-ci must need {PREP}, or a failed preparation reports green"
    );
    assert!(
        aggregate_needs.contains(&AUDIT.to_owned()),
        "{WORKFLOW} job dx-ci must need {AUDIT}"
    );
    let aggregate = scripts("dx-ci");
    for needle in [
        "result == \"skipped\" and name not in disabled",
        "skipped without disabled_checks",
    ] {
        assert!(
            aggregate.contains(needle),
            "{WORKFLOW} job dx-ci must fail a skipped {AUDIT}: {aggregate}"
        );
    }
}

#[test]
fn a_disabled_audit_disables_its_preparation_too() {
    let aggregate = scripts("dx-ci");
    assert!(
        aggregate.contains("if \"security-audit\" in disabled:")
            && aggregate.contains("disabled.add(\"advisory-snapshots\")"),
        "{WORKFLOW} job dx-ci must read a disabled {AUDIT} as a disabled {PREP}: {aggregate}"
    );
    assert!(
        gate_of(PREP).contains(&format!(",{AUDIT},")),
        "{WORKFLOW} job {PREP} must be gated by the same input as {AUDIT}"
    );
}

#[test]
fn the_shared_artifact_needs_no_token_scope_or_secret() {
    assert_eq!(
        job(PREP)["permissions"],
        json!({"contents": "read"}),
        "{WORKFLOW} job {PREP} must read the repository and nothing else"
    );
    assert_eq!(
        job(AUDIT)["permissions"],
        json!({"contents": "read", "checks": "write"}),
        "{WORKFLOW} job {AUDIT} keeps the check-run scope it already had"
    );
    assert!(
        !actions(PREP)
            .iter()
            .any(|uses| uses.starts_with("actions/download-artifact")),
        "{WORKFLOW} job {PREP} publishes the artifact instead of restoring one"
    );
    assert!(
        !actions(AUDIT)
            .iter()
            .any(|uses| uses.starts_with("actions/upload-artifact")),
        "{WORKFLOW} job {AUDIT} never publishes snapshots of its own"
    );
}
