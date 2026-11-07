//! Pins the workflow readers and the file and job their failures name.

use serde_json::json;

use ci_workflow::{
    called_workflow, job, jobs, reusable_calls, run_scripts, scripts, steps, strings, text_at,
    triggers, workflow_call,
};

const CI: &str = ".github/workflows/ci.yml";

const GATE_WORKFLOW: &str = ".github/workflows/gate.yml";

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
        all.contains("completion bash") && all.contains("input_validation_bin"),
        "{CONSUMER} job validate scripts must include the preset and validate steps: {all}"
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

#[test]
fn the_network_job_selects_opted_in_consumers() {
    let network = job(CI, "network");
    assert_eq!(network["runs-on"], json!("ubuntu-latest"));
    let minutes = network["timeout-minutes"]
        .as_i64()
        .unwrap_or_else(|| panic!("{CI} job network needs an integer timeout-minutes"));
    assert!(
        (1..=90).contains(&minutes),
        "{CI} job network timeout-minutes must be at most 90: {minutes}"
    );
    let all = scripts(CI, "network");
    for needle in [
        "attr(tags, network-consumer, //...)",
        "test -n \"$targets\"",
        "--test_timeout=",
        "DX_NESTED_BAZEL_REPOSITORY_CACHE=",
    ] {
        assert!(
            all.contains(needle),
            "{CI} job network scripts must include {needle}: {all}"
        );
    }
    assert!(
        steps(CI, "network").iter().any(|step| step["uses"]
            .as_str()
            .is_some_and(|uses| uses == "./.github/actions/setup-bazel")),
        "{CI} job network must use the local setup-bazel action, which restores the repository cache"
    );
}

#[test]
fn the_gate_job_has_a_bounded_timeout() {
    let minutes = job(CI, "gate")["timeout-minutes"]
        .as_i64()
        .unwrap_or_else(|| panic!("{CI} job gate needs an integer timeout-minutes"));
    assert!(
        (1..=60).contains(&minutes),
        "{CI} job gate timeout-minutes must be at most 60: {minutes}"
    );
}

#[test]
fn the_gate_jobs_raise_the_bazel_heap_before_setup_bazel_calls_bazel() {
    for workflow in [CI, GATE_WORKFLOW] {
        let gate = steps(workflow, "gate");
        let heap = gate.iter().position(|step| {
            step["run"].as_str().is_some_and(|run| {
                run.contains("MaxRAMPercentage=50") && run.contains(">> user.bazelrc")
            })
        });
        let setup = gate
            .iter()
            .position(|step| step["uses"].as_str() == Some("./.github/actions/setup-bazel"));
        let heap = heap.unwrap_or_else(|| panic!("{workflow} job gate must raise the Bazel heap"));
        let setup = setup.unwrap_or_else(|| panic!("{workflow} job gate must run setup-bazel"));
        assert!(
            heap < setup,
            "{workflow} job gate must raise the heap before setup-bazel calls bazel"
        );
    }
}

#[test]
fn preflight_runs_a_windows_genrule_from_cmd_without_bazel_sh() {
    let step = steps(CI, "preflight")
        .into_iter()
        .find(|step| step["shell"].as_str() == Some("cmd"))
        .unwrap_or_else(|| panic!("{CI} job preflight must run a genrule from cmd (#3377)"));
    assert_eq!(step["if"].as_str(), Some("${{ runner.os == 'Windows' }}"));
    let run = text_at(&step["run"], "the cmd step run");
    assert!(
        run.starts_with("set BAZEL_SH=\n")
            && run.contains("bazel build ")
            && run.contains(":store_build_h"),
        "{CI} job preflight must build the store genrule with BAZEL_SH unset: {run}"
    );
}

#[test]
fn the_families_job_runs_every_module_before_it_fails() {
    let all = scripts(CI, "families");
    assert!(
        all.contains("tools/dx test //... -- --keep_going"),
        "{CI} job families must keep going past one module's analysis error: {all}"
    );
    assert!(
        all.contains("|| failed=1") && all.contains("exit \"$failed\""),
        "{CI} job families must record a failed module and exit non-zero after the loop: {all}"
    );
}

#[test]
fn consumer_jobs_write_the_preset_layers_before_the_first_bazel_call() {
    let writer = "@rules_dx//:dx -- completion bash";
    for (id, script) in run_scripts(CONSUMER) {
        let Some(first) = script.lines().find(|line| line.contains("bazel ")) else {
            continue;
        };
        assert!(
            first.contains(writer),
            "{CONSUMER} job {id} must write the preset layers with `bazel run {writer}` before any other bazel call, not `{first}`"
        );
    }
}

#[test]
fn consumer_jobs_write_the_preset_layers_through_the_launcher_before_bazel_info() {
    for (id, script) in run_scripts(CONSUMER) {
        if script.is_empty() {
            continue;
        }
        let job_steps = steps(CONSUMER, &id);
        let run_of = |step: &serde_json::Value| step["run"].as_str().unwrap_or("").to_owned();
        let Some(write) = job_steps
            .iter()
            .position(|step| run_of(step).contains("completion bash"))
        else {
            continue;
        };
        let script = run_of(&job_steps[write]);
        assert!(
            script.contains("tools/dx completion bash")
                && script.contains("tools\\\\dx.cmd completion bash")
                && script
                    .contains("bazel run --lockfile_mode=error @rules_dx//:dx -- completion bash"),
            "{CONSUMER} job {id} must prefer the launcher and keep the bazel run fallback: {script}"
        );
        let info = job_steps
            .iter()
            .position(|step| run_of(step).contains("bazel info"));
        assert!(
            info.is_none_or(|info| write < info),
            "{CONSUMER} job {id} must write the preset layers before `bazel info`"
        );
    }
}
