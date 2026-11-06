use serde_json::{Value, json};

use ci_workflow::{document, jobs, steps, triggers};
use dx_testing::read_runfiles;

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

const GATE_WORKFLOW: &str = ".github/workflows/gate.yml";

const GATE_ACTION: &str = ".github/actions/gate/action.yml";

const CONSUMER_WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const RULESET: &str = ".github/rulesets/main.json";

const GATE: &str = "ci";

const AGGREGATE: &str = "dx-ci";

/// Runs a gate on every dependency outcome, except a cancelled run outside merge_group.
const GATE_IF: &str = "${{ !cancelled() || github.event_name == 'merge_group' }}";

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
        .find(|line| line.trim_start().starts_with("required=("))
        .unwrap_or_else(|| panic!("aggregate script declares no required list: {script}"));
    line.trim_start()
        .trim_start_matches("required=(")
        .trim_end_matches(')')
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Asserts the job passes each required result to its script as NEEDS_<NAME>.
fn assert_results_passed(workflow: &str, job: &str, names: &[String]) {
    let env = &document(workflow)["jobs"][job]["steps"][0]["env"];
    for name in names {
        let var = format!("NEEDS_{}", name.to_uppercase().replace('-', "_"));
        assert_eq!(
            env[&var],
            json!(format!("${{{{ needs.{name}.result }}}}")),
            "{workflow} job {job} must pass {name} as {var}"
        );
    }
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
    let wanted = json!([
        "coverage-cells",
        "dogfood",
        "docs",
        "devcontainer-check",
        "families",
        "gate",
        "gcc-cross-run-arm64",
        "hermetic",
        "hermetic-arm64",
        "hermetic-macos",
        "msix-install",
        "network",
        "preflight",
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
        json!(GATE_IF),
        "{CI_WORKFLOW} job {GATE} must run on every dependency outcome unless the run is cancelled outside merge_group"
    );
    let script = aggregate_script(CI_WORKFLOW, GATE);
    assert_results_passed(CI_WORKFLOW, GATE, &required_of(&script));
    assert_eq!(
        json!(required_of(&script)),
        wanted,
        "{CI_WORKFLOW} job {GATE} must gate every name it needs"
    );
    for needle in [
        "= success ]",
        "\\\"aggregate\\\":\\\"ci\\\"",
        "failing checks",
        "[ ${#failing[@]} -eq 0 ]",
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
    let script = aggregate_script(CONSUMER_WORKFLOW, AGGREGATE);
    assert_results_passed(CONSUMER_WORKFLOW, AGGREGATE, &wanted);
    assert_eq!(
        json!(required_of(&script)),
        json!(wanted),
        "{CONSUMER_WORKFLOW} job {AGGREGATE} must gate every check it needs"
    );
    for needle in [
        "!= success ]",
        "skipped without disabled_checks",
        "missing checks",
        "-eq 0 ]",
        "\\\"aggregate\\\":\\\"dx-ci\\\"",
        "-z \"${CHECKS:-}\"",
    ] {
        assert!(
            script.contains(needle),
            "{CONSUMER_WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

/// Returns the job ids one job needs, whether `needs` is one name or a list.
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
        other => panic!("{workflow} job {id} needs is neither a name nor a list: {other}"),
    }
}

/// Returns true when a job reads `NEEDS_*` results, so it aggregates its dependencies.
fn aggregates_needs(job: &Value) -> bool {
    serde_json::to_string(job)
        .expect("serialize job")
        .to_lowercase()
        .contains("\"needs_")
}

#[test]
fn every_aggregate_job_needs_every_other_job() {
    for workflow in [CI_WORKFLOW, CONSUMER_WORKFLOW] {
        let all = jobs(workflow);
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
                    && (other == "repo-cache-prune" || other == "scheduled-report"))
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

/// Keeps every native platform in the dogfood and preflight matrices at full coverage.
#[test]
fn the_native_matrix_keeps_all_five_platforms() {
    let document = document(CI_WORKFLOW);
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
    for workflow in [
        CI_WORKFLOW,
        "examples/consumer-ci/caller.yml",
        "examples/docs-ci/caller.yml",
    ] {
        let on = triggers(workflow);
        assert_eq!(
            on["merge_group"]["types"],
            json!(["checks_requested"]),
            "{workflow} must subscribe to merge_group checks_requested: {on}"
        );
    }
}

#[test]
fn the_scheduled_report_only_follows_the_gate() {
    let document = document(CI_WORKFLOW);
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
fn ci_can_be_dispatched_but_takes_no_inputs() {
    let on = triggers(CI_WORKFLOW);
    assert!(
        on.get("workflow_dispatch").is_some(),
        "{CI_WORKFLOW} must be dispatchable so a maintainer can run the matrix: {on}"
    );
    assert!(
        on["workflow_dispatch"].get("inputs").is_none(),
        "{CI_WORKFLOW} dispatch must take no inputs so it cannot change the matrix: {on}"
    );
    assert_eq!(on["merge_group"]["types"], json!(["checks_requested"]));
}

#[test]
fn ci_runs_on_a_schedule_so_main_has_a_known_result() {
    let on = triggers(CI_WORKFLOW);
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
    for step in steps(CI_WORKFLOW, "preflight") {
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
        let all = steps(workflow, job);
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

/// One run per ref, and nothing is cancelled.
#[test]
fn one_run_per_ref_and_none_cancel() {
    let ci = document(CI_WORKFLOW);
    assert_eq!(
        ci["concurrency"]["group"],
        json!("ci-${{ github.workflow }}-${{ github.ref }}"),
    );
    assert_eq!(ci["concurrency"]["cancel-in-progress"], json!(false));
}

#[test]
fn pull_request_comments_stay_pull_request_only() {
    let text = read_runfiles(CONSUMER_WORKFLOW);
    assert!(
        text.contains("github.event_name == 'pull_request' }}\n        env:\n          GH_TOKEN"),
        "the PR comment step must stay gated to pull_request"
    );
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
fn the_gate_job_can_never_be_skipped() {
    let document = document(CI_WORKFLOW);
    let job = &document["jobs"]["gate"];
    assert_eq!(job["name"], json!("gate"));
    assert!(job.get("if").is_none(), "gate must not have an if: {job}");
    assert!(
        job.get("needs").is_none(),
        "gate must not have needs: {job}"
    );
}

#[test]
fn the_gate_runs_on_dispatch_and_merge_groups() {
    let on = triggers(GATE_WORKFLOW);
    assert!(
        on.get("workflow_dispatch").is_some() && on.get("pull_request").is_none(),
        "{GATE_WORKFLOW} must run on dispatch, not on every pull request: {on}"
    );
    assert_eq!(
        on["merge_group"]["types"],
        json!(["checks_requested"]),
        "{GATE_WORKFLOW} must subscribe to merge_group checks_requested: {on}"
    );
}

#[test]
fn both_workflows_run_the_same_gate_action() {
    for workflow in [CI_WORKFLOW, GATE_WORKFLOW] {
        let document = document(workflow);
        let job = &document["jobs"]["gate"];
        assert_eq!(job["name"], json!("gate"), "{workflow} gate job name");
        let steps = job["steps"].as_array().expect("gate steps");
        assert!(
            steps
                .iter()
                .any(|step| step["uses"] == json!("./.github/actions/gate")),
            "{workflow} job gate must use ./.github/actions/gate: {steps:?}"
        );
    }
}

#[test]
fn the_gate_fails_when_dx_check_fails() {
    let document = document(GATE_ACTION);
    let steps = document["runs"]["steps"].as_array().expect("gate steps");
    let piped: Vec<_> = steps
        .iter()
        .filter(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("check --affected") && run.contains('|'))
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
fn the_ruleset_requires_the_gate_job_name() {
    let ruleset = document(RULESET);
    let name = document(CI_WORKFLOW)["jobs"]["gate"]["name"].clone();
    assert_eq!(ruleset["enforcement"], json!("active"));
    assert_eq!(ruleset["bypass_actors"], json!([]));
    assert_eq!(ruleset["target"], json!("branch"));
    assert_eq!(
        ruleset["conditions"]["ref_name"]["include"],
        json!(["~DEFAULT_BRANCH"])
    );
    assert_eq!(
        ruleset["rules"].as_array().map(|rules| rules.len()),
        Some(1)
    );
    let rule = &ruleset["rules"][0];
    assert_eq!(rule["type"], json!("required_status_checks"));
    let parameters = &rule["parameters"];
    assert_eq!(
        parameters["strict_required_status_checks_policy"],
        json!(false)
    );
    assert_eq!(parameters["do_not_enforce_on_create"], json!(true));
    assert_eq!(
        parameters["required_status_checks"],
        json!([{"context": name, "integration_id": 15368}])
    );
}

#[test]
fn no_other_workflow_job_reports_the_gate_context() {
    let mut names = dx_testing::workflow_files();
    names.push("examples/consumer-ci/caller.yml".to_string());
    names.push("examples/docs-ci/caller.yml".to_string());
    for name in names {
        if name == CI_WORKFLOW || name == GATE_WORKFLOW {
            continue;
        }
        let document = document(&name);
        for (id, job) in document["jobs"].as_object().into_iter().flatten() {
            assert_ne!(id, "gate", "{name} job id collides with the gate context");
            assert_ne!(
                job["name"],
                json!("gate"),
                "{name} job {id} reports the gate context"
            );
        }
    }
}
