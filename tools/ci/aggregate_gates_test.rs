use serde_json::{json, Value};

use dx_testing::read_runfiles;

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const GATE: &str = "ci";

const AGGREGATE: &str = "dx-ci";

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document(workflow: &str) -> Value {
    let text = read_runfiles(workflow);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{workflow} is not valid workflow YAML: {error}"))
}

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
    let wanted = json!(["dogfood", "docs", "devcontainer-check", "workflow-check"]);
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
        json!("${{ always() }}"),
        "{CI_WORKFLOW} job {GATE} must run on every dependency outcome"
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
