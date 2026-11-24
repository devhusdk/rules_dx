use serde_json::{Value, json};
use starlark_syntax as _;
use yaml_serde as _;

use ci_workflow::{document, jobs, steps, swallows, triggers};
use dx_testing::read_runfiles;

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

const GATE_ACTION: &str = ".github/actions/gate/action.yml";

const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const RULESET: &str = ".github/rulesets/main.json";

const GATE: &str = "ci";

const AGGREGATE: &str = "dx-ci";

const GATE_IF: &str = "${{ !cancelled() || github.event_name == 'merge_group' }}";

const NIGHTLY: &str = "github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'";

const FAIL_IF: &str = "${{ contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') || contains(needs.*.result, 'skipped') }}";

fn assert_fails_when(step: &Value, condition: &str, context: &str) {
    assert_eq!(step["if"], json!(condition), "{context} step if");
    let run = step["run"].as_str().unwrap_or_default();
    assert!(run.contains("exit 1"), "{context} step must exit 1: {run}");
}

fn needs_of(workflow: &str, job: &str) -> Vec<String> {
    let document = document(workflow).unwrap();
    document["jobs"][job]["needs"]
        .as_array()
        .unwrap_or_else(|| panic!("{workflow} job {job} declares no needs"))
        .iter()
        .map(|name| name.as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_outer_gate_requires_every_declared_dependency() {
    let document = document(CI_WORKFLOW).unwrap();
    let wanted = json!([
        "check",
        "coverage",
        "coverage-cells",
        "dogfood",
        "docs",
        "analysis-scaling",
        "in-graph-prebuilt-guard",
        "determinism",
        "devcontainer-check",
        "hermetic",
        "hermetic-macos",
        "hermetic-windows",
        "launch-heavy",
        "msix-install",
        "network",
        "preflight",
        "quality-matrix",
        "starter-consumer",
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
        json!(format!("${{{{ !cancelled() && ({NIGHTLY}) }}}}")),
        "{CI_WORKFLOW} job {GATE} must run on every nightly dependency outcome unless the run is cancelled"
    );
    let step = &document["jobs"][GATE]["steps"][0];
    assert_fails_when(step, FAIL_IF, CI_WORKFLOW);
    assert_eq!(step["env"]["NEEDS_JSON"], json!("${{ toJSON(needs) }}"));
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
    let document = document(CONSUMER_WORKFLOW).unwrap();
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
    assert_eq!(wanted.len(), 3, "{CONSUMER_WORKFLOW} declares the checks");
    let steps = &document["jobs"][AGGREGATE]["steps"];
    assert_fails_when(
        &steps[0],
        "${{ needs.validate.result != 'success' }}",
        CONSUMER_WORKFLOW,
    );
    assert_fails_when(
        &steps[1],
        "${{ needs.validate.outputs.checks == '' }}",
        CONSUMER_WORKFLOW,
    );
    assert_fails_when(&steps[2], FAIL_IF, CONSUMER_WORKFLOW);
    assert_eq!(steps[2]["env"]["NEEDS_JSON"], json!("${{ toJSON(needs) }}"));
}

fn job_needs(workflow: &str, id: &str, job: &Value) -> Vec<String> {
    match &job["needs"] {
        Value::Null => Vec::new(),
        Value::String(one) => vec![one.clone()],
        Value::Array(many) => many
            .iter()
            .map(|name| {
                name.as_str()
                    .unwrap_or_else(|| panic!("{workflow} job {id} needs a non-string: {name}"))
                    .to_owned()
            })
            .collect(),
        other @ (Value::Bool(_) | Value::Number(_) | Value::Object(_)) => {
            panic!("{workflow} job {id} needs is neither a name nor a list: {other}")
        }
    }
}

fn aggregates_needs(job: &Value) -> bool {
    serde_json::to_string(job)
        .expect("serialize job")
        .to_lowercase()
        .contains("\"needs_")
}

#[test]
fn every_aggregate_job_needs_every_other_job() {
    for workflow in [CI_WORKFLOW, CONSUMER_WORKFLOW] {
        let all = jobs(workflow).unwrap();
        assert!(all.len() > 1, "{workflow} declares no aggregate job");
        let aggregates: Vec<&(String, Value)> = all
            .iter()
            .filter(|(_, job)| aggregates_needs(job))
            .collect();
        assert_eq!(
            aggregates.len(),
            1,
            "{workflow} must aggregate over its needs exactly once"
        );
        let (id, job) = aggregates[0];
        let needs = job_needs(workflow, id, job);
        for (other, _) in &all {
            if other == id
                || (workflow == CI_WORKFLOW
                    && (other == "scheduled-report" || other == BASELINE_DIFF))
                || (workflow == CONSUMER_WORKFLOW
                    && (other == "advisory-bump"
                        || other == "bench"
                        || other == "deploy-dry-run"
                        || other == "fuzz"))
            {
                continue;
            }
            assert!(
                needs.contains(other),
                "{workflow}: the {id} required check must need {other}; without it a failing \
                 {other} reports green behind the aggregate"
            );
        }
    }
}

#[test]
fn the_native_matrix_keeps_all_five_platforms() {
    let document = document(CI_WORKFLOW).unwrap();
    let wanted = json!([
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
        "windows_arm64"
    ]);
    let raw = document["jobs"]["dogfood"]["with"]["platforms"].clone();
    let raw = raw.as_str().unwrap_or_else(|| {
        panic!("{CI_WORKFLOW} job dogfood with.platforms is not a string: {raw}")
    });
    let platforms: Value = serde_json::from_str(raw).unwrap_or_else(|error| {
        panic!("{CI_WORKFLOW} job dogfood with.platforms {raw:?} is not a JSON array: {error}")
    });
    assert_eq!(
        platforms, wanted,
        "{CI_WORKFLOW} job dogfood with.platforms must list every native platform"
    );
    let preflight: Vec<Value> = document["jobs"]["preflight"]["strategy"]["matrix"]["include"]
        .as_array()
        .unwrap_or_else(|| panic!("{CI_WORKFLOW} job preflight declares no matrix.include"))
        .iter()
        .map(|cell| cell["platform"].clone())
        .collect();
    assert_eq!(
        json!(preflight),
        wanted,
        "{CI_WORKFLOW} job preflight strategy.matrix.include must list every native platform"
    );
    assert_eq!(
        document["jobs"]["dogfood"]["with"]["min_coverage"],
        json!("100"),
        "{CI_WORKFLOW} job dogfood with.min_coverage must stay at 100"
    );
}

#[test]
fn every_gate_workflow_runs_on_merge_group_checks_requested() {
    for workflow in [CI_WORKFLOW, "examples/docs-ci/caller.yml"] {
        let on = triggers(workflow).unwrap();
        assert_eq!(
            on["merge_group"]["types"],
            json!(["checks_requested"]),
            "{workflow} must subscribe to merge_group checks_requested: {on}"
        );
    }
}

#[test]
fn the_scheduled_report_only_follows_the_gate() {
    let document = document(CI_WORKFLOW).unwrap();
    let job = &document["jobs"]["scheduled-report"];
    assert_eq!(job["needs"], json!(["ci"]));
    assert!(
        job["if"]
            .as_str()
            .unwrap_or_default()
            .contains("github.event_name == 'schedule'")
    );
    let step = &job["steps"][0];
    assert_eq!(step["env"]["CI_RESULT"], json!("${{ needs.ci.result }}"));
    let run = step["run"].as_str().unwrap_or_default();
    for word in ["cancelled", "skipped", "timed_out"] {
        assert!(
            run.contains(word),
            "scheduled-report script must handle {word}"
        );
    }
}

#[test]
fn coverage_cells_runs_only_after_dogfood_succeeds() {
    let document = document(CI_WORKFLOW).unwrap();
    let job = &document["jobs"]["coverage-cells"];
    assert_eq!(job["needs"], json!(["dogfood"]));
    assert_eq!(
        job["if"],
        json!(format!(
            "${{{{ !cancelled() && ({NIGHTLY}) && needs.dogfood.result == 'success' }}}}"
        )),
        "coverage-cells must skip when dogfood was skipped or failed, not report every cell missing"
    );
}

#[test]
fn ci_can_be_dispatched_but_takes_no_inputs() {
    let on = triggers(CI_WORKFLOW).unwrap();
    assert!(
        on.get("workflow_dispatch").is_some(),
        "{CI_WORKFLOW} must be dispatchable so a maintainer can run the matrix: {on}"
    );
    assert!(
        on["workflow_dispatch"].get("inputs").is_none(),
        "{CI_WORKFLOW} dispatch must take no inputs so it cannot change the matrix: {on}"
    );
}

#[test]
fn ci_runs_on_a_schedule_so_main_has_a_known_result() {
    let on = triggers(CI_WORKFLOW).unwrap();
    let cron = on["schedule"][0]["cron"]
        .as_str()
        .expect("ci.yml needs a schedule cron");
    let minute = cron.split_whitespace().next().unwrap();
    assert_ne!(
        minute, "0",
        "schedule must avoid the top of the hour: {cron}"
    );
    assert!(minute.chars().all(|c| c.is_ascii_digit()), "{cron}");
}

#[test]
fn preflight_passes_no_runner_specific_test_args() {
    for step in steps(CI_WORKFLOW, "preflight").unwrap() {
        let run = step["run"].as_str().unwrap_or_default();
        assert!(
            !run.contains("--test_arg"),
            "preflight mixes Rust, Zig, Julia, C and quality runners, so no global --test_arg: {run}"
        );
    }
}

#[test]
fn every_matrix_cell_records_its_runner_image() {
    for (workflow, job) in [(CI_WORKFLOW, "preflight"), (CONSUMER_WORKFLOW, "checks")] {
        let all = steps(workflow, job).unwrap();
        let found = all
            .iter()
            .find(|step| step["name"] == json!("Record the runner image"))
            .unwrap_or_else(|| panic!("{workflow} job {job} must record the runner image"));
        assert_eq!(found["if"], json!("always()"), "{workflow} {job} if");
        let run = found["run"].as_str().unwrap_or_default();
        for needle in [
            "RUNNER_OS",
            "RUNNER_ARCH",
            "${ImageOS:-unknown}",
            "${ImageVersion:-unknown}",
            "bazel --version",
            "|| true",
        ] {
            assert!(
                run.contains(needle),
                "{workflow} {job} step lacks {needle}: {run}"
            );
        }
    }
}

#[test]
fn one_run_per_ref_and_only_pull_requests_and_pushes_cancel() {
    let ci = document(CI_WORKFLOW).unwrap();
    assert_eq!(
        ci["concurrency"]["group"],
        json!(
            "ci-${{ github.workflow }}-${{ github.event_name == 'push' && 'push-' || '' }}${{ github.ref }}"
        ),
    );
    assert_eq!(
        ci["concurrency"]["cancel-in-progress"],
        json!("${{ github.event_name == 'pull_request' || github.event_name == 'push' }}")
    );
}

#[test]
fn ci_runs_on_push_only_for_main() {
    let on = triggers(CI_WORKFLOW).unwrap();
    assert_eq!(on["push"], json!({"branches": ["main"]}));
}

#[test]
fn every_check_and_coverage_job_records_its_failing_labels() {
    let document = document(CI_WORKFLOW).unwrap();
    for id in REQUIRED_JOBS {
        let job = &document["jobs"][id];
        let gate = job["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .find(|step| step["uses"] == "./.github/actions/gate")
            .unwrap_or_else(|| panic!("{id} has no gate step"));
        assert_eq!(
            gate["with"]["baseline"],
            json!(format!("{id}-${{{{ matrix.platform }}}}"))
        );
        assert_eq!(job["permissions"]["actions"], json!("read"), "{id}");
    }
}

#[test]
fn baseline_diff_runs_on_pull_requests_after_every_gate_job() {
    let document = document(CI_WORKFLOW).unwrap();
    let job = &document["jobs"][BASELINE_DIFF];
    assert_eq!(
        job["if"],
        json!("${{ !cancelled() && github.event_name == 'pull_request' }}")
    );
    assert_eq!(job["needs"], json!(REQUIRED_JOBS));
    assert_eq!(job["permissions"], json!({"actions": "read"}));
    let script = job["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<String>();
    for needle in [
        "main-baseline-$job",
        ".workflow_run.head_branch == \"main\"",
        ".workflow_run.head_repository_id == .workflow_run.repository_id",
        "$GITHUB_STEP_SUMMARY",
        "exit \"$bad\"",
    ] {
        assert!(script.contains(needle), "{BASELINE_DIFF} must use {needle}");
    }
}

#[test]
fn pull_request_comments_stay_pull_request_only() {
    let comments: Vec<Value> = jobs(CONSUMER_WORKFLOW)
        .unwrap()
        .iter()
        .flat_map(|(_, job)| job["steps"].as_array().cloned().unwrap_or_default())
        .filter(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("Publish PR summary comment"))
        })
        .collect();
    assert!(
        !comments.is_empty(),
        "{CONSUMER_WORKFLOW} has no PR comment step"
    );
    for step in comments {
        assert!(
            step["if"]
                .as_str()
                .is_some_and(|gate| gate.contains("github.event_name == 'pull_request'")),
            "the PR comment step must stay gated to pull_request"
        );
        assert!(!step["env"]["DX_INVOCATION_ID"].is_null(), "{step}");
    }
}

#[test]
fn pages_publication_stays_on_default_branch_push() {
    let text = read_runfiles("examples/docs-ci/caller.yml");
    assert!(
        text.contains(
            "publish: ${{ github.event_name == 'push' && github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}"
        ),
        "publish must be limited to pushes to the default branch"
    );
}

#[test]
fn the_check_and_coverage_jobs_run_on_every_event() {
    let document = document(CI_WORKFLOW).unwrap();
    for id in REQUIRED_JOBS {
        let job = &document["jobs"][id];
        assert!(job.get("if").is_none(), "{id} must not have an if: {job}");
        assert!(
            job.get("needs").is_none(),
            "{id} must not have needs: {job}"
        );
    }
}

#[test]
fn every_other_job_runs_only_on_schedule_or_dispatch() {
    let document = document(CI_WORKFLOW).unwrap();
    for (id, job) in document["jobs"].as_object().expect("jobs") {
        if REQUIRED_JOBS.contains(&id.as_str()) || id == BASELINE_DIFF {
            continue;
        }
        let condition = job["if"].as_str().unwrap_or_default();
        assert!(
            condition.contains("github.event_name == 'schedule'")
                && condition.contains("github.event_name == 'workflow_dispatch'"),
            "{CI_WORKFLOW} job {id} must run only on schedule or dispatch: {condition:?}"
        );
    }
}

#[test]
fn the_gate_fails_when_dx_check_fails() {
    let document = document(GATE_ACTION).unwrap();
    let steps = document["runs"]["steps"].as_array().expect("gate steps");
    let piped: Vec<_> = steps
        .iter()
        .filter(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("--affected") && run.contains('|'))
        })
        .collect();
    assert!(
        !piped.is_empty(),
        "no piped dx check step in gate: {steps:?}"
    );
    for step in piped {
        assert_eq!(
            step["shell"],
            json!("bash"),
            "piped step needs pipefail: {step}"
        );
    }
}

#[test]
fn the_gate_action_runs_dx_with_the_lock_check_and_forwards_bazel_options() {
    let action = document(GATE_ACTION).unwrap();
    let run = action["runs"]["steps"]
        .as_array()
        .expect("gate steps")
        .iter()
        .filter_map(|step| step["run"].as_str())
        .find(|run| run.contains("\"$COMMAND\" --affected"))
        .expect("dx step");
    assert!(
        run.contains("set -- --lockfile_mode=error $BAZEL_CONFIG $BB_ARGS"),
        "{run}"
    );
    assert!(run.contains("-- \"$@\""), "{run}");
}

#[test]
fn the_gate_runs_a_copy_of_dx_that_bazel_cannot_rewrite_under_it() {
    let action = document(GATE_ACTION).unwrap();
    let run = action["runs"]["steps"]
        .as_array()
        .expect("gate steps")
        .iter()
        .filter_map(|step| step["run"].as_str())
        .find(|run| run.contains("\"$COMMAND\" --affected"))
        .expect("dx step");
    assert!(run.contains("bazel build \"$@\" @rules_dx//:dx"), "{run}");
    assert!(run.contains("dx=\"${RUNNER_TEMP:-/tmp}/"), "{run}");
    assert!(run.contains("\"$dx\" \"$COMMAND\" --affected"), "{run}");
    assert!(!run.contains("bazel run"), "{run}");
}

#[test]
fn the_gate_pushes_build_on_the_last_main_baseline() {
    let action = document(GATE_ACTION).unwrap();
    let steps = action["runs"]["steps"].as_array().expect("gate steps");
    let step = |name: &str| {
        steps
            .iter()
            .find(|step| step["name"] == name)
            .unwrap_or_else(|| panic!("gate has no step {name:?}"))
    };
    assert_eq!(
        step("Pin the affected base")["env"]["PUSH_BEFORE"],
        json!("${{ steps.previous.outputs.sha || github.event.before }}")
    );
    let record = step("Record the failing labels");
    let run = record["run"].as_str().expect("record run");
    assert!(
        run.contains("--failing-labels --out \"$out\" --dx \"$RUNNER_TEMP/gate.json\""),
        "{run}"
    );
    assert!(run.contains("--previous \"$previous\""), "{run}");
    assert_eq!(
        step("Upload the failing labels")["with"]["name"],
        json!(
            "${{ github.event_name == 'push' && 'main-baseline' || 'failing' }}-${{ inputs.baseline }}"
        )
    );
}

#[test]
fn the_gate_verifies_the_base_even_when_dx_fails() {
    let document = document(GATE_ACTION).unwrap();
    let verify = document["runs"]["steps"]
        .as_array()
        .expect("gate steps")
        .iter()
        .find(|step| step["name"] == "Verify the base dx compared against")
        .expect("verify step");
    assert_eq!(
        verify["if"],
        json!("${{ !cancelled() && steps.check.outcome != 'skipped' }}")
    );
}

fn reported_contexts(id: &str, job: &Value) -> Vec<String> {
    let name = job["name"].as_str().unwrap_or(id);
    if !name.contains("${{ matrix.platform }}") {
        return vec![name.to_owned()];
    }
    job["strategy"]["matrix"]["include"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|cell| cell["platform"].as_str())
        .map(|platform| name.replace("${{ matrix.platform }}", platform))
        .collect()
}

const REQUIRED_JOBS: [&str; 2] = ["check", "coverage"];

const BASELINE_DIFF: &str = "baseline-diff";

fn required_contexts() -> Vec<String> {
    let document = document(CI_WORKFLOW).unwrap();
    REQUIRED_JOBS
        .iter()
        .flat_map(|id| reported_contexts(id, &document["jobs"][id]))
        .collect()
}

#[test]
fn the_ruleset_requires_every_check_and_coverage_job_and_queues_merges() {
    let ruleset = document(RULESET).unwrap();
    assert_eq!(ruleset["enforcement"], json!("active"));
    assert_eq!(ruleset["bypass_actors"], json!([]));
    assert_eq!(ruleset["target"], json!("branch"));
    assert_eq!(
        ruleset["conditions"]["ref_name"]["include"],
        json!(["~DEFAULT_BRANCH"])
    );
    assert_eq!(ruleset["rules"].as_array().map(Vec::len), Some(2));
    let rule = &ruleset["rules"][0];
    assert_eq!(rule["type"], json!("required_status_checks"));
    let parameters = &rule["parameters"];
    assert_eq!(
        parameters["strict_required_status_checks_policy"],
        json!(false)
    );
    assert_eq!(parameters["do_not_enforce_on_create"], json!(true));
    let contexts = required_contexts();
    assert_eq!(contexts.len(), 10, "{contexts:?}");
    let wanted: Vec<Value> = contexts
        .iter()
        .map(|context| json!({"context": context, "integration_id": 15368}))
        .collect();
    assert_eq!(parameters["required_status_checks"], json!(wanted));
    let queue = &ruleset["rules"][1];
    assert_eq!(queue["type"], json!("merge_queue"));
    assert_eq!(
        queue["parameters"],
        json!({
            "merge_method": "SQUASH",
            "grouping_strategy": "ALLGREEN",
            "max_entries_to_build": 5,
            "min_entries_to_merge": 1,
            "max_entries_to_merge": 5,
            "min_entries_to_merge_wait_minutes": 1,
            "check_response_timeout_minutes": 180
        })
    );
}

#[test]
fn every_required_context_is_reported_by_exactly_one_job() {
    let ruleset = document(RULESET).unwrap();
    let required: Vec<&str> = ruleset["rules"][0]["parameters"]["required_status_checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["context"].as_str().unwrap_or_default())
        .collect();
    let mut names = dx_testing::workflow_files();
    names.push("examples/docs-ci/caller.yml".to_string());
    for context in required {
        let mut reporters = Vec::new();
        for name in &names {
            let document = document(name).unwrap();
            for (id, job) in document["jobs"].as_object().into_iter().flatten() {
                if id == context || reported_contexts(id, job).iter().any(|c| c == context) {
                    reporters.push((name.clone(), id.clone()));
                }
            }
        }
        let job = context.split(' ').next().unwrap_or_default();
        assert_eq!(
            reporters,
            vec![(CI_WORKFLOW.to_string(), job.to_string())],
            "exactly one job must report the {context} context"
        );
    }
}

const ALLOWED_SWALLOWS: [(&str, &str, &str, &str, &str); 16] = [
    (
        "ci.yml",
        "preflight",
        "Record the Windows rmeta diagnostics (#4850)",
        "|| true",
        "diagnostics only; the build under diagnosis reports its own failure",
    ),
    (
        "ci.yml",
        "preflight",
        "Record the runner image",
        "|| true",
        "the image record is informational",
    ),
    (
        "ci.yml",
        "scheduled-report",
        "Open, update or close the tracking issue",
        "|| true",
        "a failed issue lookup must not fail the report",
    ),
    (
        "evidence.yml",
        "evidence",
        "Build and qualify the release cell",
        "|| true",
        "the cell's own check step reports the failure",
    ),
    (
        "evidence.yml",
        "evidence",
        "Install the Android SDK packages",
        "|| true",
        "sdkmanager exits non-zero when the pipe closes after the licenses are accepted",
    ),
    (
        "evidence.yml",
        "evidence",
        "Record the Apple executor",
        "|| true",
        "the executor record is informational",
    ),
    (
        "evidence.yml",
        "evidence",
        "Collect test logs",
        "|| true",
        "a missing log must not hide the test failure",
    ),
    (
        "issue-labels.yml",
        "missing-labels",
        "Warn about documented labels missing from the repository",
        "|| true",
        "an empty match list is the expected case",
    ),
    (
        "issue-labels.yml",
        "blocked-issues",
        "Check that blocked issues name a blocker, and unblock closed ones",
        "|| true",
        "an issue without a Blocked on line is the expected case",
    ),
    (
        "release-cells.yml",
        "cell",
        "Collect the cell files",
        "|| true",
        "a cell may produce no files",
    ),
    (
        "release-cells.yml",
        "verify",
        "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333",
        "continue-on-error",
        "a cell may upload no artifact",
    ),
    (
        "release.yml",
        "dry-run",
        "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333",
        "continue-on-error",
        "a cell may upload no artifact",
    ),
    (
        "reusable-consumer.yml",
        "checks",
        "Run the checks",
        "|| true",
        "the exit status is recorded and reported by a later step",
    ),
    (
        "reusable-consumer.yml",
        "checks",
        "Check changed-line coverage",
        "|| true",
        "the coverage verdict is read from the event stream",
    ),
    (
        "reusable-consumer.yml",
        "checks",
        "Publish PR summary comment (dedup marker, fork-safe)",
        "|| true",
        "a failed comment lookup must not fail the checks",
    ),
    (
        "reusable-consumer.yml",
        "checks",
        "Record the runner image",
        "|| true",
        "the image record is informational",
    ),
];

#[test]
fn every_workflow_swallows_no_failure_outside_the_allow_list() {
    let mut found = Vec::new();
    for path in dx_testing::workflow_files() {
        let file = path.rsplit('/').next().unwrap_or(&path).to_owned();
        for (job, step, kind) in swallows(&path).unwrap() {
            found.push((file.clone(), job, step, kind));
        }
    }
    for (file, job, step, kind) in &found {
        assert!(
            ALLOWED_SWALLOWS
                .iter()
                .any(|(f, j, s, k, _)| f == file && j == job && s == step && k == kind),
            "{file} job {job} step {step:?} swallows a failure with {kind}"
        );
    }
    for (file, job, step, kind, reason) in ALLOWED_SWALLOWS {
        assert!(
            found
                .iter()
                .any(|(f, j, s, k)| f == file && j == job && s == step && k == kind),
            "stale allow-list entry: {file} {job} {step:?} {kind} ({reason})"
        );
    }
}

#[test]
fn the_swallow_scanner_finds_entries() {
    assert!(
        swallows(CONSUMER_WORKFLOW)
            .unwrap()
            .iter()
            .any(|(_, _, kind)| kind == "|| true"),
        "{CONSUMER_WORKFLOW} swallows failures the scanner must report"
    );
}

#[test]
fn compiler_probes_run_before_the_evidence_step_and_fail_normally() {
    const EVIDENCE_ACTION: &str = ".github/actions/compiler-evidence/action.yml";
    for job in ["hermetic", "starter-consumer"] {
        let all = steps(CI_WORKFLOW, job).unwrap();
        let probe = all
            .iter()
            .position(|step| step["name"] == json!("Run the compiler probe tests"))
            .unwrap_or_else(|| panic!("{job} has no compiler probe step"));
        let evidence = all
            .iter()
            .position(|step| step["uses"] == json!("./.github/actions/compiler-evidence"))
            .unwrap_or_else(|| panic!("{job} has no compiler-evidence step"));
        assert!(probe < evidence, "{job} probe step must precede evidence");
        let probe_run = all[probe]["run"].as_str().unwrap_or_default();
        assert!(!probe_run.contains("|| true"), "{job} probe step swallows");
        assert!(probe_run.contains("bazel test"), "{job} probe runs no test");
        assert!(
            all[evidence]["if"]
                .as_str()
                .unwrap_or_default()
                .starts_with("always()"),
            "{job} evidence step must run always"
        );
    }
    let action = document(EVIDENCE_ACTION).unwrap();
    let record = &action["runs"]["steps"][0];
    assert_eq!(record["name"], json!("Record compiler evidence"));
    assert_eq!(record["if"], json!("always()"));
    let evidence_run = record["run"].as_str().unwrap_or_default();
    assert!(
        !evidence_run
            .lines()
            .any(|line| line.trim_start().starts_with("bazel test")),
        "evidence step runs tests"
    );
}

#[test]
fn the_gate_job_merges_evidence_without_swallowing() {
    let all = steps(CI_WORKFLOW, GATE).unwrap();
    for step in all.iter().filter(|step| {
        let uses = step["uses"].as_str().unwrap_or_default();
        step["name"] == json!("Merge compiler evidence")
            || uses.starts_with("actions/download-artifact@")
            || uses.starts_with("actions/upload-artifact@")
    }) {
        assert!(
            step.get("continue-on-error").is_none(),
            "{GATE} step swallows: {step}"
        );
        assert!(
            step["if"]
                .as_str()
                .unwrap_or_default()
                .starts_with("always()")
                || step["if"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("${{ always()"),
            "{GATE} step must run always: {step}"
        );
    }
}

#[test]
fn issue_label_jobs_read_gh_output_before_searching_it() {
    let workflow = ".github/workflows/issue-labels.yml";
    for (id, _) in jobs(workflow).unwrap() {
        for line in ci_workflow::scripts(workflow, &id).unwrap().lines() {
            let (producer, consumer) = line.rsplit_once('|').unwrap_or(("", line));
            let quiet = consumer
                .split_whitespace()
                .skip_while(|word| *word != "grep")
                .any(|word| word.starts_with('-') && !word.starts_with("--") && word.contains('q'));
            assert!(
                !(quiet && producer.contains("gh ")),
                "{workflow} job {id} pipes gh into a quiet grep, which exits before gh finishes writing: {line}"
            );
        }
    }
    for id in ["blocked-issues", "unreviewed-merges"] {
        let script = ci_workflow::scripts(workflow, id).unwrap();
        assert!(
            script.contains("comments=$(gh ")
                && script.contains(r#"[[ "$comments" != *"$marker"* ]]"#),
            "{workflow} job {id} must test the reminder marker against the comments it has read in full"
        );
    }
}
