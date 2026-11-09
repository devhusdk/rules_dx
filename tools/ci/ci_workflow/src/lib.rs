//! Reads GitHub Actions workflows from runfiles for the CI policy tests.

use serde_json::Value;

use dx_testing::read_runfiles;

/// Returns the parsed workflow at `path`, which must be valid Actions YAML.
#[track_caller]
pub fn document(path: &str) -> Value {
    let text = read_runfiles(path);
    match yaml_serde::from_str(&text) {
        Ok(document) => document,
        Err(error) => panic!("{path} is not valid workflow YAML: {error}"),
    }
}

/// Returns one job of the workflow at `path`, which must declare it.
#[track_caller]
pub fn job(path: &str, name: &str) -> Value {
    let document = document(path);
    let found = document["jobs"][name].clone();
    assert!(
        found.is_object(),
        "{path} names no {name:?} job: {}",
        document["jobs"]
    );
    found
}

/// Returns the steps of one job, which must declare some.
#[track_caller]
pub fn steps(path: &str, name: &str) -> Vec<Value> {
    let Some(found) = job(path, name)["steps"].as_array().cloned() else {
        panic!("{path} job {name} declares no steps");
    };
    found
}

/// Returns the `run` scripts of one job, joined.
#[track_caller]
pub fn scripts(path: &str, name: &str) -> String {
    steps(path, name)
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns one string value of a parsed workflow.
#[track_caller]
pub fn text_at(value: &Value, what: &str) -> String {
    let Some(text) = value.as_str() else {
        panic!("{what} is not a string: {value}");
    };
    text.to_owned()
}

/// Returns every job of the workflow at `path` with its id, sorted by id.
#[track_caller]
pub fn jobs(path: &str) -> Vec<(String, Value)> {
    let document = document(path);
    let Some(found) = document["jobs"].as_object() else {
        panic!("{path} declares no jobs");
    };
    found
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect()
}

/// Returns the joined `run` scripts of every job, with the job id.
#[track_caller]
pub fn run_scripts(path: &str) -> Vec<(String, String)> {
    jobs(path)
        .into_iter()
        .map(|(id, job)| {
            let scripts = job["steps"]
                .as_array()
                .map(|steps| {
                    steps
                        .iter()
                        .filter_map(|step| step["run"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            (id, scripts)
        })
        .collect()
}

/// Returns the trigger table of a workflow, whichever way the parser keys `on`.
#[track_caller]
pub fn triggers(path: &str) -> Value {
    let document = document(path);
    if document["on"].is_object() {
        return document["on"].clone();
    }
    document["true"].clone()
}

/// Returns `on.workflow_call` of the reusable workflow at `path`, which must declare it.
#[track_caller]
pub fn workflow_call(path: &str) -> Value {
    let found = triggers(path)["workflow_call"].clone();
    assert!(found.is_object(), "{path} declares no on.workflow_call");
    found
}

/// Returns the called workflow file name of a `uses` value, when it names a reusable workflow.
pub fn called_workflow(uses: &str) -> Option<String> {
    if !uses.contains("/.github/workflows/") {
        return None;
    }
    let pinned = uses.split('@').next().unwrap_or(uses);
    Some(pinned.rsplit('/').next().unwrap_or(pinned).to_owned())
}

/// Returns (job id, called workflow file name, job) for every job that calls a reusable workflow.
#[track_caller]
pub fn reusable_calls(path: &str) -> Vec<(String, String, Value)> {
    jobs(path)
        .into_iter()
        .filter_map(|(id, job)| {
            let called = called_workflow(job["uses"].as_str()?)?;
            Some((id, called, job))
        })
        .collect()
}

/// Returns every string value of the workflow at `path`, so scans skip comments and keys.
#[track_caller]
pub fn strings(path: &str) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => out.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => map.values().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(&document(path), &mut out);
    out
}
