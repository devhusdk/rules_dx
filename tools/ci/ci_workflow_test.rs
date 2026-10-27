//! Pins the workflow readers and the file and job their failures name.

use serde_json::json;

use ci_workflow::{job, scripts, steps, text_at};

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
