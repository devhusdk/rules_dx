use serde_json::{json, Value};

use ci_workflow::document;
use dx_testing::read_runfiles;

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const CANCEL_WORKFLOW: &str = ".github/workflows/cancel-stale.yml";

const GATE: &str = "ci";

const AGGREGATE: &str = "dx-ci";

/// Runs a gate on every dependency outcome, except a cancelled run outside merge_group.
const GATE_IF: &str = "${{ !cancelled() || github.event_name == 'merge_group' }}";

/// Returns the aggregate run script of one job.
fn aggregate_script(workflow: &str, job: &str) -> String {
    let document = document(workflow);
    let run = document["jobs"][job]["steps"][0]["run"].clone();
    run.as_str()
        .unwrap_or_else(|| panic!("{workflow} job {job} steps.0.run is not a script: {run}"))
        .to_owned()
}

/// Returns the required list one aggregate script declares.
fn required_of(script: &str) -> Vec<String> {
    let line = script
        .lines()
        .find(|line| line.trim_start().starts_with("required = ["))
        .unwrap_or_else(|| panic!("aggregate script declares no required list: {script}"));
    let raw = line
        .trim_start()
        .trim_start_matches("required = ")
        .trim_end();
    serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("required list {raw:?} is not a JSON array: {error}"))
}

/// Returns the declared needs of one job as names.
fn needs_of(workflow: &str, job: &str) -> Vec<String> {
    let document = document(workflow);
    document["jobs"][job]["needs"]
        .as_array()
        .unwrap_or_else(|| panic!("{workflow} job {job} declares no needs"))
        .iter()
        .map(|name| name.as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_outer_gate_requires_every_declared_dependency() {
    let document = document(CI_WORKFLOW);
    let wanted = json!([
        "dogfood",
        "docs",
        "devcontainer-check",
        "hermetic",
        "preflight",
        "workflow-check"
    ]);
    assert_eq!(
        document["jobs"][GATE]["needs"], wanted,
        "{CI_WORKFLOW} job {GATE} must need every job it gates"
    );
    assert_eq!(
        document["jobs"][GATE]["name"],
        json!(GATE),
        "{CI_WORKFLOW} job {GATE} keeps the branch-protection check name"
    );
    assert_eq!(
        document["jobs"][GATE]["if"],
        json!(GATE_IF),
        "{CI_WORKFLOW} job {GATE} must run on every dependency outcome unless the run is cancelled outside merge_group"
    );
    let script = aggregate_script(CI_WORKFLOW, GATE);
    assert_eq!(
        json!(required_of(&script)),
        wanted,
        "{CI_WORKFLOW} job {GATE} must gate every name it needs"
    );
    for needle in [
        "!= \"success\"",
        "\"aggregate\": \"ci\"",
        "\"failing checks\"",
        "sys.exit(1 if failing else 0)",
    ] {
        assert!(
            script.contains(needle),
            "{CI_WORKFLOW} job {GATE} must state {needle:?}: {script}"
        );
    }
    let summary = document["jobs"][GATE]["steps"][1]["run"]
        .as_str()
        .unwrap_or_else(|| panic!("{CI_WORKFLOW} job {GATE} steps.1.run is not a script"));
    assert!(
        summary.contains("every needed job must succeed"),
        "{CI_WORKFLOW} job {GATE} summary must state the success rule: {summary}"
    );
}

#[test]
fn the_consumer_gate_requires_every_declared_check() {
    let document = document(CONSUMER_WORKFLOW);
    assert_eq!(
        document["jobs"][AGGREGATE]["name"],
        json!("dx-ci (aggregate)"),
        "{CONSUMER_WORKFLOW} job {AGGREGATE} keeps the branch-protection check name"
    );
    assert_eq!(
        document["jobs"][AGGREGATE]["if"],
        json!(GATE_IF),
        "{CONSUMER_WORKFLOW} job {AGGREGATE} must run on every dependency outcome unless the run is cancelled outside merge_group"
    );
    let wanted: Vec<String> = needs_of(CONSUMER_WORKFLOW, AGGREGATE)
        .into_iter()
        .filter(|name| name != "validate")
        .collect();
    assert_eq!(wanted.len(), 10, "{CONSUMER_WORKFLOW} declares the checks");
    let script = aggregate_script(CONSUMER_WORKFLOW, AGGREGATE);
    assert_eq!(
        json!(required_of(&script)),
        json!(wanted),
        "{CONSUMER_WORKFLOW} job {AGGREGATE} must gate every check it needs"
    );
    for needle in [
        "results.get(\"validate\") != \"success\"",
        "result == \"skipped\" and name not in disabled",
        "skipped without disabled_checks",
        "missing checks",
        "if bad or lost or missing:",
        "\"aggregate\": \"dx-ci\"",
    ] {
        assert!(
            script.contains(needle),
            "{CONSUMER_WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

/// Returns the trigger table of a workflow, whichever way the parser keys `on`.
fn triggers(workflow: &str) -> Value {
    let document = document(workflow);
    if document["on"].is_object() {
        return document["on"].clone();
    }
    document["true"].clone()
}

#[test]
fn every_gate_workflow_runs_on_merge_group_checks_requested() {
    for workflow in [
        CI_WORKFLOW,
        "examples/consumer-ci/caller.yml",
        "examples/docs-ci/caller.yml",
    ] {
        let on = triggers(workflow);
        assert_eq!(
            on["merge_group"]["types"],
            json!(["checks_requested"]),
            "{workflow} must subscribe to merge_group checks_requested: {on}"
        );
    }
}

/// Returns the env table and run script of one job's only step.
fn cancel_step(job: &str) -> (Value, String) {
    let document = document(CANCEL_WORKFLOW);
    let step = &document["jobs"][job]["steps"][0];
    let run = step["run"]
        .as_str()
        .unwrap_or_else(|| panic!("{CANCEL_WORKFLOW} job {job} steps.0.run is not a script"))
        .to_owned();
    (step["env"].clone(), run)
}

/// Returns the run statuses one cancel job sweeps.
fn statuses_of(env: &Value) -> Vec<String> {
    env["STATUSES"]
        .as_str()
        .unwrap_or_else(|| panic!("{CANCEL_WORKFLOW} step declares no STATUSES: {env}"))
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn cancel_stale_runs_on_closed_pull_requests_and_main_pushes() {
    let on = triggers(CANCEL_WORKFLOW);
    assert_eq!(
        on["pull_request_target"]["types"],
        json!(["closed"]),
        "{on}"
    );
    assert_eq!(on["push"]["branches"], json!(["main"]), "{on}");
    let document = document(CANCEL_WORKFLOW);
    assert_eq!(document["permissions"]["actions"], "write");
    assert_eq!(document["permissions"]["pull-requests"], "read");
    assert_eq!(
        document["jobs"]["cancel-closed-prs"]["if"],
        json!("github.event_name == 'push' || !github.event.pull_request.merged"),
    );
    assert_eq!(
        document["jobs"]["cancel-replaced-main"]["if"],
        json!("github.event_name == 'push'"),
    );
    for job in ["cancel-closed-prs", "cancel-replaced-main"] {
        assert_eq!(
            document["jobs"][job]["runs-on"],
            json!("ubuntu-slim"),
            "{job}"
        );
    }
}

/// Closing a pull request cancels its ci run through the shared concurrency group.
#[test]
fn closed_pull_requests_join_the_ci_concurrency_group() {
    let ci = document(CI_WORKFLOW);
    assert_eq!(ci["name"], json!("ci"));
    assert_eq!(
        ci["concurrency"]["group"],
        json!("ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}"),
    );
    assert_eq!(
        ci["concurrency"]["cancel-in-progress"],
        json!("${{ github.event_name == 'pull_request' }}"),
    );
    let cancel = document(CANCEL_WORKFLOW);
    assert_eq!(
        cancel["concurrency"]["group"],
        json!("${{ github.event_name == 'pull_request_target' && format('ci-ci-{0}', github.event.pull_request.number) || 'cancel-stale-main' }}"),
    );
    assert_eq!(
        cancel["concurrency"]["cancel-in-progress"],
        json!("${{ github.event_name == 'pull_request_target' }}"),
    );
}

#[test]
fn closed_pull_requests_lose_queued_and_running_runs() {
    let (env, script) = cancel_step("cancel-closed-prs");
    assert_eq!(
        statuses_of(&env),
        ["queued", "in_progress", "waiting", "pending", "requested"],
    );
    for needle in [
        "-f state=open",
        "-f event=pull_request",
        "<<'DX_STALE_PR_RUNS'",
        "actions/runs/${id}/force-cancel",
    ] {
        assert!(
            script.contains(needle),
            "cancel-closed-prs must state {needle:?}: {script}"
        );
    }
}

#[test]
fn main_pushes_cancel_only_queued_runs_of_older_commits() {
    let (env, script) = cancel_step("cancel-replaced-main");
    let statuses = statuses_of(&env);
    assert_eq!(statuses, ["queued", "waiting", "pending", "requested"]);
    assert_eq!(env["NEW_SHA"], json!("${{ github.sha }}"));
    for needle in [
        "-f event=push -f branch=main",
        "<<'DX_STALE_MAIN_RUNS'",
        "actions/runs/${id}/cancel\"",
    ] {
        assert!(
            script.contains(needle),
            "cancel-replaced-main must state {needle:?}: {script}"
        );
    }
}

#[test]
fn cancel_failures_fail_the_job() {
    for job in ["cancel-closed-prs", "cancel-replaced-main"] {
        let (_, script) = cancel_step(job);
        assert!(
            !script.contains("|| true"),
            "{job} hides failures: {script}"
        );
        for needle in [
            "set -euo pipefail",
            "*\"(HTTP 409)\"*)",
            "failed=1",
            "exit \"$failed\"",
        ] {
            assert!(
                script.contains(needle),
                "{job} must state {needle:?}: {script}"
            );
        }
    }
}

#[test]
fn pull_request_comments_stay_pull_request_only() {
    let text = read_runfiles(CONSUMER_WORKFLOW);
    assert!(
        text.contains(
            "if: always() && github.event_name == 'pull_request'\n        env:\n          GH_TOKEN"
        ),
        "the PR comment step must stay gated to pull_request"
    );
}

#[test]
fn pages_publication_stays_on_main_push_only() {
    let text = read_runfiles("examples/docs-ci/caller.yml");
    assert!(
        text.contains(
            "publish: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"
        ),
        "publish must be limited to pushes to main"
    );
}
