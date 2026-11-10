//! Reads GitHub Actions workflows from runfiles for the CI policy tests.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use serde_json::Value;

use dx_testing::read_runfiles;

/// Returns the parsed workflow `text`, named `name` in failures, which must be valid Actions YAML.
#[track_caller]
pub fn parse(name: &str, text: &str) -> Value {
    match yaml_serde::from_str(text) {
        Ok(document) => document,
        Err(error) => panic!("{name} is not valid workflow YAML: {error}"),
    }
}

/// Returns the parsed workflow at `path`, which must be valid Actions YAML.
#[track_caller]
pub fn document(path: &str) -> Value {
    parse(path, &read_runfiles(path))
}

/// Returns (path, document) for every workflow in `DX_WORKFLOWS`, parsed once per process.
#[track_caller]
pub fn all() -> Vec<(String, Value)> {
    static PARSED: std::sync::OnceLock<Vec<(String, Value)>> = std::sync::OnceLock::new();
    PARSED
        .get_or_init(|| {
            dx_testing::workflow_files()
                .into_iter()
                .map(|path| {
                    let document = document(&path);
                    (path, document)
                })
                .collect()
        })
        .clone()
}

/// Returns (id, job) for every job of `document`, or none when it declares no jobs.
pub fn jobs_of(document: &Value) -> Vec<(&str, &Value)> {
    document
        .get("jobs")
        .and_then(Value::as_object)
        .map(|jobs| jobs.iter().map(|(id, job)| (id.as_str(), job)).collect())
        .unwrap_or_default()
}

/// Returns the steps of `job`, or none for a job that calls a reusable workflow.
pub fn steps_of(job: &Value) -> &[Value] {
    job.get("steps")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Returns the shell and `actions/github-script` code a step runs.
pub fn step_code(step: &Value) -> Vec<&str> {
    let mut code = Vec::new();
    if let Some(run) = step.get("run").and_then(Value::as_str) {
        code.push(run);
    }
    let is_script = step
        .get("uses")
        .and_then(Value::as_str)
        .is_some_and(|uses| uses.starts_with("actions/github-script@"));
    if is_script {
        if let Some(script) = step.pointer("/with/script").and_then(Value::as_str) {
            code.push(script);
        }
    }
    code
}

/// Returns the permissions a job holds: its own, else the workflow's.
pub fn effective_permissions<'a>(document: &'a Value, job: &'a Value) -> &'a Value {
    job.get("permissions").unwrap_or(&document["permissions"])
}

/// Returns the keys that a job's `strategy.matrix` defines, including `include` entries.
pub fn matrix_keys(job: &Value) -> Vec<String> {
    let Some(matrix) = job.pointer("/strategy/matrix").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = matrix
        .keys()
        .filter(|key| key.as_str() != "include" && key.as_str() != "exclude")
        .cloned()
        .collect();
    for entry in matrix
        .get("include")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        keys.extend(
            entry
                .as_object()
                .into_iter()
                .flat_map(|map| map.keys().cloned()),
        );
    }
    keys
}

/// Pushes the name of every `matrix.<name>` read inside a `${{ }}` expression of `value`.
pub fn matrix_refs(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for expression in text.split("${{").skip(1) {
                let expression = expression.split("}}").next().unwrap_or_default();
                for (at, _) in expression.match_indices("matrix.") {
                    let before = expression[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-')) {
                        continue;
                    }
                    let name: String = expression[at + "matrix.".len()..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
                        .collect();
                    if !name.is_empty() {
                        found.push(name);
                    }
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| matrix_refs(item, found)),
        Value::Object(map) => map.values().for_each(|item| matrix_refs(item, found)),
        _ => {}
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

/// Returns (job id, step name or `uses`, "|| true" or "continue-on-error") for every failure the workflow at `path` swallows.
#[track_caller]
pub fn swallows(path: &str) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for (id, job) in jobs(path) {
        if job.get("continue-on-error").is_some() {
            found.push((id.clone(), String::new(), "continue-on-error".to_owned()));
        }
        let Some(steps) = job["steps"].as_array() else {
            continue;
        };
        for step in steps {
            let label = step["name"]
                .as_str()
                .or_else(|| step["uses"].as_str())
                .unwrap_or_default()
                .to_owned();
            if step.get("continue-on-error").is_some() {
                found.push((id.clone(), label.clone(), "continue-on-error".to_owned()));
            }
            if step["run"]
                .as_str()
                .is_some_and(|run| run.contains("|| true"))
            {
                found.push((id.clone(), label, "|| true".to_owned()));
            }
        }
    }
    found
}
