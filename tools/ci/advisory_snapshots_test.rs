use serde_json::{Value, json};

use ci_workflow::{document, job, jobs, scripts, steps, text_at};

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const PREP: &str = "advisory-snapshots";

const AUDIT: &str = "security-audit";

const VALIDATE: &str = "validate";

const ARTIFACT: &str = "${{ needs.validate.outputs.snapshot_artifact }}";

const RETRIEVED_AT: &str = "${{ needs.advisory-snapshots.outputs.retrieved_at }}";

const PREP_TARGET: &str = "@rules_dx//dx/advisory_prep";

fn actions(name: &str) -> Vec<String> {
    steps(WORKFLOW, name)
        .unwrap()
        .iter()
        .filter_map(|step| step["uses"].as_str().map(str::to_owned))
        .collect()
}

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

#[test]
fn one_preparation_job_runs_per_run_and_not_per_platform() {
    let prep = job(WORKFLOW, PREP).unwrap();
    assert_eq!(prep["needs"], json!([VALIDATE]));
    assert!(
        prep.get("strategy").is_none(),
        "{WORKFLOW} job {PREP} must run once, so it expands no platform matrix"
    );
    assert_eq!(prep["runs-on"], json!("ubuntu-latest"));
    assert_eq!(prep["permissions"], json!({"contents": "read"}));
    for name in [PREP, AUDIT] {
        assert!(
            job(WORKFLOW, name).unwrap().get("if").is_none(),
            "{WORKFLOW} job {name} runs on every call"
        );
    }
}

fn script_running(name: &str, needle: &str) -> String {
    let found = steps(WORKFLOW, name).unwrap();
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
    text_at(&found[0]["run"], &format!("{name} run")).unwrap()
}

#[test]
fn the_preparation_runs_the_pinned_native_command_once() {
    let script = script_running(PREP, PREP_TARGET);
    for needle in [
        "@rules_dx//dx/advisory_prep -- \\",
        "--workspace \"$GITHUB_WORKSPACE\"",
        "--out \"$GITHUB_WORKSPACE/.dx/local/advisory\"",
        "--ci",
        "--status-file \"$GITHUB_OUTPUT\"",
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
        !scripts(WORKFLOW, PREP)
            .unwrap()
            .contains("osv-vulnerabilities"),
        "{WORKFLOW} job {PREP} names no advisory database URL: {}",
        scripts(WORKFLOW, PREP).unwrap()
    );
    assert!(
        !scripts(WORKFLOW, PREP).unwrap().contains("secrets."),
        "{WORKFLOW} job {PREP} must need no secret, so fork pull requests run it"
    );
}

#[test]
fn the_preparation_publishes_one_run_artifact() {
    assert_eq!(
        job(WORKFLOW, PREP).unwrap()["outputs"]["trivy"],
        json!("${{ steps.prepare.outputs.trivy }}")
    );
    let prep = steps(WORKFLOW, PREP).unwrap();
    let scan = prep
        .iter()
        .find(|step| step["id"] == json!("scan-advisory"))
        .expect("the scan-advisory step exists");
    assert_eq!(
        scan["if"],
        json!(
            "${{ steps.prepare.outputs.status != 'empty' || steps.prepare.outputs.trivy == 'true' }}"
        )
    );
    let upload = action_step(&prep, "actions/upload-artifact");
    assert_eq!(upload["with"]["name"], json!(ARTIFACT));
    assert_eq!(upload["with"]["path"], json!(".dx/local/advisory"));
    assert_eq!(upload["with"]["if-no-files-found"], json!("error"));
}

#[test]
fn the_preparation_stamps_every_snapshot_with_the_run_date() {
    assert_eq!(
        job(WORKFLOW, PREP).unwrap()["outputs"]["retrieved_at"],
        json!("${{ steps.prepare.outputs.retrieved_at }}"),
        "{WORKFLOW} job {PREP} must publish the date it stamped"
    );
    let prep = steps(WORKFLOW, PREP).unwrap();
    let dated: Vec<&Value> = prep
        .iter()
        .filter(|step| step["id"] == json!("prepare"))
        .collect();
    assert_eq!(
        dated.len(),
        1,
        "{WORKFLOW} job {PREP} must name one step prepare"
    );
    let script = text_at(&dated[0]["run"], "prepare run").unwrap();
    for needle in [PREP_TARGET, "--ci", "--status-file \"$GITHUB_OUTPUT\""] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {PREP} step prepare must run {needle} so advisory_prep appends retrieved_at: {script}"
        );
    }
    assert!(
        !script.contains("--date"),
        "{WORKFLOW} job {PREP} lets advisory_prep --ci stamp one UTC date per run: {script}"
    );
}

#[test]
fn every_audit_cell_restores_that_artifact_instead_of_downloading() {
    let audit = steps(WORKFLOW, AUDIT).unwrap();
    let download = action_step(&audit, "actions/download-artifact");
    assert_eq!(download["with"]["name"], json!(ARTIFACT));
    assert_eq!(download["with"]["path"], json!(".dx/local/advisory"));
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
        !scripts(WORKFLOW, AUDIT)
            .unwrap()
            .contains("osv-vulnerabilities"),
        "{WORKFLOW} job {AUDIT} names no advisory database URL: {}",
        scripts(WORKFLOW, AUDIT).unwrap()
    );
}

#[test]
fn every_audit_cell_judges_freshness_against_the_shared_date() {
    let audit = steps(WORKFLOW, AUDIT).unwrap();
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
        job(WORKFLOW, AUDIT).unwrap()["needs"],
        json!([VALIDATE, PREP]),
        "{WORKFLOW} job {AUDIT} must wait for the prepared snapshots"
    );
    let aggregate_needs: Vec<String> = document(WORKFLOW).unwrap()["jobs"]["dx-ci"]["needs"]
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
    let aggregate = document(WORKFLOW).unwrap()["jobs"]["dx-ci"]["steps"].to_string();
    assert!(
        aggregate.contains("contains(needs.*.result, 'skipped')"),
        "{WORKFLOW} job dx-ci must fail a skipped {AUDIT}: {aggregate}"
    );
}

#[test]
fn the_shared_artifact_needs_no_token_scope_or_secret() {
    assert_eq!(
        job(WORKFLOW, PREP).unwrap()["permissions"],
        json!({"contents": "read"}),
        "{WORKFLOW} job {PREP} must read the repository and nothing else"
    );
    assert_eq!(
        job(WORKFLOW, AUDIT).unwrap()["permissions"],
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
        job(WORKFLOW, PREP).unwrap()["outputs"]["status"],
        json!("${{ steps.prepare.outputs.status }}")
    );
    assert_eq!(
        job(WORKFLOW, PREP).unwrap()["outputs"]["ecosystems"],
        json!("${{ steps.prepare.outputs.ecosystems }}")
    );
    let prep = steps(WORKFLOW, PREP).unwrap();
    let upload = action_step(&prep, "actions/upload-artifact");
    assert_eq!(
        upload["if"],
        json!("${{ always() && steps.scan-advisory.outcome == 'success' }}")
    );
    let script = script_running(PREP, PREP_TARGET);
    assert!(
        script.contains("--status-file \"$GITHUB_OUTPUT\""),
        "{WORKFLOW} job {PREP} must publish the native status: {script}"
    );
    let audit = steps(WORKFLOW, AUDIT).unwrap();
    let download = action_step(&audit, "actions/download-artifact");
    assert_eq!(
        download["if"],
        json!(
            "${{ needs.advisory-snapshots.outputs.status != 'empty' || needs.advisory-snapshots.outputs.trivy == 'true' }}"
        )
    );
}

#[test]
fn the_audit_still_runs_when_the_preparation_is_empty() {
    let audit = steps(WORKFLOW, AUDIT).unwrap();
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
    let document = document(WORKFLOW).unwrap();
    let group = document["concurrency"]["group"]
        .as_str()
        .expect("concurrency group");
    assert!(
        group.contains("inputs.invocation_id"),
        "{WORKFLOW} concurrency group must separate sibling calls: {group}"
    );
    assert_eq!(
        job(WORKFLOW, VALIDATE).unwrap()["outputs"]["snapshot_artifact"],
        json!("${{ steps.validate.outputs.snapshot_artifact }}")
    );
    assert!(
        document["on"]["workflow_call"]["inputs"]["invocation_id"].is_object(),
        "{WORKFLOW} must declare the invocation_id input"
    );
}

fn names_an_advisory_database(job: &str) -> bool {
    job.contains("all.zip") || job.contains("osv-vulnerabilities")
}

#[test]
fn only_the_preparation_job_names_an_advisory_database() {
    let prep = serde_json::to_string(&job(WORKFLOW, PREP).unwrap()).expect("serialize job");
    assert_eq!(
        prep.matches(&format!("{PREP_TARGET} -- ")).count(),
        1,
        "the {PREP} job runs {PREP_TARGET} exactly once, so one run prepares one snapshot set"
    );
    assert!(
        !prep.contains("@rules_dx//:dx -- security"),
        "the {PREP} job prepares snapshots; it must not audit a scope"
    );
    for (other, body) in jobs(WORKFLOW).unwrap() {
        let body = serde_json::to_string(&body).expect("serialize job");
        assert!(
            !names_an_advisory_database(&body),
            "{WORKFLOW} job {other} must audit the shared snapshots instead of downloading a database"
        );
    }
}
