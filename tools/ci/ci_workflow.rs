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
