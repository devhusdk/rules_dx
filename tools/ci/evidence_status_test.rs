use dx_testing::read_runfiles;
use serde_json::{Value, json};

fn workflow() -> Value {
    yaml_serde::from_str(&read_runfiles(".github/workflows/evidence.yml"))
        .expect("evidence.yml is valid YAML")
}

fn run_text(workflow: &Value, job: &str) -> String {
    workflow["jobs"][job]["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("job {job} has steps"))
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_only_trigger_is_workflow_dispatch() {
    let workflow = workflow();
    let on = workflow
        .get("on")
        .and_then(Value::as_object)
        .expect("evidence.yml has an on map");
    assert_eq!(on.keys().collect::<Vec<_>>(), ["workflow_dispatch"]);
}

#[test]
fn only_the_status_jobs_hold_statuses_write() {
    let workflow = workflow();
    assert_eq!(
        workflow["jobs"]["pending"]["permissions"],
        json!({"statuses": "write"})
    );
    assert_eq!(
        workflow["jobs"]["finish"]["permissions"],
        json!({"actions": "read", "statuses": "write"})
    );
    assert_eq!(
        workflow["jobs"]["evidence"]["permissions"],
        json!({"contents": "read"})
    );
    assert_eq!(workflow["permissions"], json!({"contents": "read"}));
}

#[test]
fn finish_always_runs_after_the_evidence_job() {
    let workflow = workflow();
    let finish = &workflow["jobs"]["finish"];
    assert_eq!(finish["needs"], json!(["pending", "evidence"]));
    let condition = finish["if"].as_str().expect("finish has an if");
    assert!(condition.contains("always()"), "{condition}");
    assert!(workflow["jobs"]["pending"].get("needs").is_none());
}

#[test]
fn statuses_use_the_commit_context_and_run_url() {
    let workflow = workflow();
    for job in ["pending", "finish"] {
        let text = run_text(&workflow, job);
        assert!(
            text.contains("context=evidence/$platform/$task/$scope"),
            "{job}"
        );
        assert!(text.contains("statuses/$GITHUB_SHA"), "{job}");
        assert!(text.contains("actions/runs/$GITHUB_RUN_ID"), "{job}");
        assert!(!text.contains("${{"), "{job} puts an expression in run");
        assert!(text.contains("set -o pipefail"), "{job}");
        assert!(!text.contains("|| true"), "{job} swallows a failed post");
    }
}

#[test]
fn the_matrix_job_name_matches_what_finish_reads() {
    let workflow = workflow();
    assert_eq!(
        workflow["jobs"]["evidence"]["name"],
        "evidence (${{ matrix.platform }})"
    );
    assert!(run_text(&workflow, "finish").contains(r#""evidence ($platform)""#));
}

#[test]
fn only_the_evidence_job_receives_secrets() {
    let workflow = workflow();
    assert!(
        workflow
            .get("env")
            .is_none_or(|env| !env.to_string().contains("secrets."))
    );
    for job in ["pending", "finish"] {
        let job_env = workflow["jobs"][job].get("env");
        assert!(
            job_env.is_none_or(|env| !env.to_string().contains("secrets.")),
            "{job} has secrets in env"
        );
        for step in workflow["jobs"][job]["steps"].as_array().expect("steps") {
            assert!(
                step.get("env")
                    .is_none_or(|env| !env.to_string().contains("secrets.")),
                "{job} step has secrets in env"
            );
        }
    }
    assert!(workflow["jobs"]["evidence"]["env"]["BUILDBUDDY_API_KEY"].is_string());
}
