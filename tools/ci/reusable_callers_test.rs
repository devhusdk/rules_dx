//! Pins the contract between each reusable workflow, its callers and the GitHub CI page.

use serde_json::Value;

use ci_workflow::{jobs, reusable_calls, workflow_call};
use dx_testing::read_runfiles;

const DOCS: &str = "docs/github-ci.md";

const CI: &str = ".github/workflows/ci.yml";

/// Each reusable workflow with the starter that calls it.
const PAIRS: [(&str, &str); 2] = [
    ("reusable-consumer.yml", "examples/consumer-ci/caller.yml"),
    ("reusable-docs.yml", "examples/docs-ci/caller.yml"),
];

fn workflow_path(name: &str) -> String {
    format!(".github/workflows/{name}")
}

/// Returns the starters and ci.yml, every file that calls a reusable workflow.
fn callers() -> Vec<&'static str> {
    let mut found: Vec<&str> = PAIRS.iter().map(|(_, caller)| *caller).collect();
    found.push(CI);
    found
}

/// Returns the keys of one map value, sorted; a missing value has none.
fn keys(value: &Value, what: &str) -> Vec<String> {
    if value.is_null() {
        return Vec::new();
    }
    let Some(map) = value.as_object() else {
        panic!("{what} is not a map: {value}");
    };
    map.keys().cloned().collect()
}

#[test]
fn callers_forward_every_workflow_secret() {
    let page = read_runfiles(DOCS);
    for (workflow, caller) in PAIRS {
        let path = workflow_path(workflow);
        let secrets = keys(
            &workflow_call(&path)["secrets"],
            &format!("{path} on.workflow_call.secrets"),
        );
        assert!(!secrets.is_empty(), "{workflow} declares no secrets");
        let calls: Vec<(String, String, Value)> = reusable_calls(caller)
            .into_iter()
            .filter(|(_, called, _)| called == workflow)
            .collect();
        assert!(!calls.is_empty(), "{caller} never calls {workflow}");
        for secret in &secrets {
            assert!(
                page.contains(secret.as_str()),
                "{DOCS} never names the {workflow} secret {secret}"
            );
            for (id, _, job) in &calls {
                let forwarded = &job["secrets"];
                assert!(
                    forwarded.as_str() == Some("inherit") || forwarded.get(secret).is_some(),
                    "{caller} job {id} never forwards the {workflow} secret {secret}"
                );
            }
        }
    }
}

#[test]
fn docs_page_names_every_workflow_check() {
    let page = read_runfiles(DOCS);
    for (workflow, _) in PAIRS {
        let path = workflow_path(workflow);
        let checks: Vec<String> = jobs(&path)
            .into_iter()
            .map(|(id, job)| job["name"].as_str().map_or(id, str::to_owned))
            .filter(|name| !name.contains("${{"))
            .collect();
        assert!(!checks.is_empty(), "{workflow} declares no jobs");
        for check in checks {
            assert!(
                page.contains(&format!("`{check}`")),
                "{DOCS} never names the {workflow} check {check}"
            );
        }
    }
}

#[test]
fn every_caller_input_is_a_workflow_input() {
    for caller in callers() {
        let calls = reusable_calls(caller);
        assert!(!calls.is_empty(), "{caller} calls no reusable workflow");
        for (id, workflow, job) in calls {
            let path = workflow_path(&workflow);
            let inputs = keys(
                &workflow_call(&path)["inputs"],
                &format!("{path} on.workflow_call.inputs"),
            );
            let passed = keys(&job["with"], &format!("{caller} job {id} with"));
            assert!(!passed.is_empty(), "{caller} job {id} passes no inputs");
            for key in passed {
                assert!(
                    inputs.contains(&key),
                    "{caller} job {id} passes {key}, which {workflow} does not declare"
                );
            }
        }
    }
}

#[test]
fn every_caller_passes_every_required_workflow_input() {
    for caller in callers() {
        for (id, workflow, job) in reusable_calls(caller) {
            let path = workflow_path(&workflow);
            let inputs = workflow_call(&path)["inputs"].clone();
            let passed = keys(&job["with"], &format!("{caller} job {id} with"));
            for name in keys(&inputs, &format!("{path} on.workflow_call.inputs")) {
                if inputs[&name]["required"] != Value::Bool(true) {
                    continue;
                }
                assert!(
                    passed.contains(&name),
                    "{caller} job {id} does not pass {name}, which {workflow} requires"
                );
            }
        }
    }
}

#[test]
fn reusable_workflows_launch_dx_through_the_public_label() {
    const CRATE_LABEL: &str = "//cli/cli:dx";
    const PUBLIC_LABEL: &str = "@rules_dx//:dx";
    for (workflow, _) in PAIRS {
        let text = read_runfiles(&workflow_path(workflow));
        assert!(
            text.contains(PUBLIC_LABEL),
            "{workflow} must launch dx through {PUBLIC_LABEL}"
        );
        assert!(
            !text.contains(CRATE_LABEL),
            "{workflow} must launch dx through {PUBLIC_LABEL}, never the crate label"
        );
    }
}
