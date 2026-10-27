use serde_json::{json, Value};

use ci_workflow::{document, job, jobs, scripts, steps, text_at};

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const PREP: &str = "advisory-snapshots";

const AUDIT: &str = "security-audit";

const VALIDATE: &str = "validate";

const ARTIFACT: &str = "${{ needs.validate.outputs.snapshot_artifact }}";

const RETRIEVED_AT: &str = "${{ needs.advisory-snapshots.outputs.retrieved_at }}";

const PREP_TARGET: &str = "@rules_dx//cli/advisory_prep";

/// Returns the `uses` action of every step that names one.
fn actions(name: &str) -> Vec<String> {
    steps(WORKFLOW, name)
        .iter()
        .filter_map(|step| step["uses"].as_str().map(str::to_owned))
        .collect()
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
    job(WORKFLOW, name)["if"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn one_preparation_job_runs_per_run_and_not_per_platform() {
    let prep = job(WORKFLOW, PREP);
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
    let found = steps(WORKFLOW, name);
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
    for needle in [
        "@rules_dx//cli/advisory_prep -- \\",
        "--workspace \"$GITHUB_WORKSPACE\"",
        "--out \"$GITHUB_WORKSPACE/.dx/advisory\"",
        "--date \"$retrieved_at\"",
    ] {
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
        !scripts(WORKFLOW, PREP).contains("osv-vulnerabilities"),
        "{WORKFLOW} job {PREP} names no advisory database URL: {}",
        scripts(WORKFLOW, PREP)
    );
    assert!(
        !scripts(WORKFLOW, PREP).contains("secrets."),
        "{WORKFLOW} job {PREP} must need no secret, so fork pull requests run it"
    );
}

#[test]
fn the_preparation_publishes_one_run_artifact() {
    let prep = steps(WORKFLOW, PREP);
    let upload = action_step(&prep, "actions/upload-artifact");
    assert_eq!(upload["with"]["name"], json!(ARTIFACT));
    assert_eq!(upload["with"]["path"], json!(".dx/advisory"));
    assert_eq!(upload["with"]["if-no-files-found"], json!("error"));
}

#[test]
fn the_preparation_stamps_every_snapshot_with_the_run_date() {
    assert_eq!(
        job(WORKFLOW, PREP)["outputs"]["retrieved_at"],
        json!("${{ steps.prepare.outputs.retrieved_at }}"),
        "{WORKFLOW} job {PREP} must publish the date it stamped"
    );
    let prep = steps(WORKFLOW, PREP);
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

#[test]
fn every_audit_cell_restores_that_artifact_instead_of_downloading() {
    let audit = steps(WORKFLOW, AUDIT);
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
        !scripts(WORKFLOW, AUDIT).contains("osv-vulnerabilities"),
        "{WORKFLOW} job {AUDIT} names no advisory database URL: {}",
        scripts(WORKFLOW, AUDIT)
    );
}

#[test]
fn every_audit_cell_judges_freshness_against_the_shared_date() {
    let audit = steps(WORKFLOW, AUDIT);
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
        job(WORKFLOW, AUDIT)["needs"],
        json!([VALIDATE, PREP]),
        "{WORKFLOW} job {AUDIT} must wait for the prepared snapshots"
    );
    let aggregate_needs: Vec<String> = document(WORKFLOW)["jobs"]["dx-ci"]["needs"]
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
    let aggregate = scripts(WORKFLOW, "dx-ci");
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
    let aggregate = scripts(WORKFLOW, "dx-ci");
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
        job(WORKFLOW, PREP)["permissions"],
        json!({"contents": "read"}),
        "{WORKFLOW} job {PREP} must read the repository and nothing else"
    );
    assert_eq!(
        job(WORKFLOW, AUDIT)["permissions"],
        json!({"contents": "read"}),
        "{WORKFLOW} job {AUDIT} must read the repository and nothing else"
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

#[test]
fn an_empty_preparation_skips_the_upload_and_the_download() {
    assert_eq!(
        job(WORKFLOW, PREP)["outputs"]["status"],
        json!("${{ steps.prepare.outputs.status }}")
    );
    assert_eq!(
        job(WORKFLOW, PREP)["outputs"]["ecosystems"],
        json!("${{ steps.prepare.outputs.ecosystems }}")
    );
    let prep = steps(WORKFLOW, PREP);
    let upload = action_step(&prep, "actions/upload-artifact");
    assert_eq!(
        upload["if"],
        json!("${{ steps.prepare.outputs.status != 'empty' }}")
    );
    let script = script_running(PREP, PREP_TARGET);
    assert!(
        script.contains("--status-file \"$GITHUB_OUTPUT\""),
        "{WORKFLOW} job {PREP} must publish the native status: {script}"
    );
    let audit = steps(WORKFLOW, AUDIT);
    let download = action_step(&audit, "actions/download-artifact");
    assert_eq!(
        download["if"],
        json!("${{ needs.advisory-snapshots.outputs.status != 'empty' }}")
    );
}

#[test]
fn the_audit_still_runs_when_the_preparation_is_empty() {
    let audit = steps(WORKFLOW, AUDIT);
    let run = audit
        .iter()
        .find(|step| {
            step["run"]
                .as_str()
                .is_some_and(|script| script.contains("@rules_dx//:dx -- security"))
        })
        .expect("the audit step exists");
    assert!(run.get("if").is_none(), "secret scanning must always run");
    let gitleaks = audit
        .iter()
        .find(|step| step["name"] == json!("Resolve hermetic Gitleaks"))
        .expect("gitleaks step exists");
    assert!(gitleaks.get("if").is_none());
}

#[test]
fn the_artifact_name_and_concurrency_carry_the_validated_invocation() {
    let document = document(WORKFLOW);
    let group = document["concurrency"]["group"]
        .as_str()
        .expect("concurrency group");
    assert!(
        group.contains("inputs.invocation_id"),
        "{WORKFLOW} concurrency group must separate sibling calls: {group}"
    );
    assert_eq!(
        job(WORKFLOW, VALIDATE)["outputs"]["snapshot_artifact"],
        json!("${{ steps.validate.outputs.snapshot_artifact }}")
    );
    assert!(
        document["on"]["workflow_call"]["inputs"]["invocation_id"].is_object(),
        "{WORKFLOW} must declare the invocation_id input"
    );
}

/// Returns true when a job names an advisory database it would download itself.
fn names_an_advisory_database(job: &str) -> bool {
    job.contains("all.zip") || job.contains("osv-vulnerabilities")
}

#[test]
fn only_the_preparation_job_names_an_advisory_database() {
    let prep = serde_json::to_string(&job(WORKFLOW, PREP)).expect("serialize job");
    assert_eq!(
        prep.matches(PREP_TARGET).count(),
        1,
        "the {PREP} job runs {PREP_TARGET} exactly once, so one run prepares one snapshot set"
    );
    assert!(
        !prep.contains("@rules_dx//:dx -- security"),
        "the {PREP} job prepares snapshots; it must not audit a scope"
    );
    for (other, body) in jobs(WORKFLOW) {
        let body = serde_json::to_string(&body).expect("serialize job");
        assert!(
            !names_an_advisory_database(&body),
            "{WORKFLOW} job {other} must audit the shared snapshots instead of downloading a database"
        );
    }
}
