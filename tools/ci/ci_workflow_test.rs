//! Pins the workflow readers and the file and job their failures name.

use serde_json::json;

use ci_workflow::{
    called_workflow, job, jobs, reusable_calls, run_scripts, scripts, steps, strings, text_at,
    triggers, workflow_call,
};

const CI: &str = ".github/workflows/ci.yml";

const CONSUMER: &str = ".github/workflows/reusable-consumer.yml";

#[test]
#[should_panic(expected = ".github/workflows/reusable-consumer.yml names no \"no-such-job\" job")]
fn a_missing_job_names_the_file_and_the_job() {
    job(CONSUMER, "no-such-job");
}

#[test]
#[should_panic(expected = ".github/workflows/ci.yml job dogfood declares no steps")]
fn a_job_without_steps_names_the_file_and_the_job() {
    steps(CI, "dogfood");
}

#[test]
#[should_panic(expected = "x is not a string: 1")]
fn a_non_string_value_names_what_was_read() {
    text_at(&json!(1), "x");
}

#[test]
fn scripts_join_the_run_lines_of_one_job() {
    let all = scripts(CONSUMER, "validate");
    assert!(
        all.contains("bazel --nohome_rc --nosystem_rc info repository_cache"),
        "{CONSUMER} job validate scripts must include its repository cache lookup: {all}"
    );
}

#[test]
fn jobs_list_every_job_sorted_by_id() {
    let ids: Vec<String> = jobs(CONSUMER).into_iter().map(|(id, _)| id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "{CONSUMER} jobs must come back sorted by id");
    assert!(
        ids.iter().any(|id| id == "validate"),
        "{CONSUMER} jobs must include validate: {ids:?}"
    );
}

#[test]
fn run_scripts_join_the_run_lines_of_every_job() {
    let all = run_scripts(CONSUMER);
    assert_eq!(all.len(), jobs(CONSUMER).len(), "one entry per job");
    let validate = all
        .iter()
        .find(|(id, _)| id == "validate")
        .map(|(_, script)| script.clone())
        .expect("validate job scripts");
    assert_eq!(validate, scripts(CONSUMER, "validate"));
}

#[test]
fn triggers_read_the_on_table_whichever_way_it_is_keyed() {
    assert!(
        triggers(CI)["merge_group"].is_object(),
        "{CI} must read its merge_group trigger"
    );
    assert!(
        workflow_call(CONSUMER)["inputs"].is_object(),
        "{CONSUMER} must read its workflow_call inputs"
    );
}

#[test]
#[should_panic(expected = ".github/workflows/ci.yml declares no on.workflow_call")]
fn a_workflow_without_workflow_call_names_the_file() {
    workflow_call(CI);
}

#[test]
fn called_workflow_strips_the_ref_and_keeps_the_basename() {
    assert_eq!(
        called_workflow("./.github/workflows/x.yml").as_deref(),
        Some("x.yml")
    );
    assert_eq!(
        called_workflow("o/r/.github/workflows/x.yml@abc123").as_deref(),
        Some("x.yml")
    );
    assert_eq!(called_workflow("actions/checkout@abc123"), None);
}

#[test]
fn reusable_calls_name_each_calling_job_and_its_workflow() {
    let calls: Vec<(String, String)> = reusable_calls(CI)
        .into_iter()
        .map(|(id, called, _)| (id, called))
        .collect();
    assert!(
        calls.contains(&("dogfood".to_owned(), "reusable-consumer.yml".to_owned())),
        "{CI} job dogfood calls reusable-consumer.yml: {calls:?}"
    );
    assert!(
        calls.iter().all(|(_, called)| called.ends_with(".yml")),
        "{calls:?}"
    );
}

#[test]
fn strings_read_values_and_skip_comments() {
    let all = strings(CI);
    assert!(
        all.iter()
            .any(|text| text == "./.github/workflows/reusable-consumer.yml"),
        "{CI} strings must include the dogfood uses value"
    );
    assert!(
        !all.iter()
            .any(|text| text.contains("Dogfoods the reusable file end to end")),
        "{CI} strings must skip comments"
    );
}
