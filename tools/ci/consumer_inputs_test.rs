use serde_json::{Value, json};

use ci_workflow::{document, job, text_at};
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const VALIDATE: &str = "validate";

const PREP: &str = "advisory-snapshots";

const AGGREGATE: &str = "dx-ci";

const MATRIX_JOB: &str = "checks";

const PLATFORM_JOBS: [&str; 2] = [MATRIX_JOB, "security-audit"];

const CHECKS_LIST: &str = "${{ needs.validate.outputs.checks }}";

const PLATFORMS_REF: &str = "needs.validate.outputs.platforms";

const RUNNERS_REF: &str = "needs.validate.outputs.runners";

const DISABLED_REF: &str = "needs.validate.outputs.disabled";

const DISABLED: &str = "${{ needs.validate.outputs.disabled }}";

const THRESHOLD: &str = "${{ needs.validate.outputs.min_coverage }}";

const MATRIX: &str = "${{ fromJSON(needs.validate.outputs.platforms) }}";

const RUNNER: &str = "${{ fromJSON(needs.validate.outputs.runners)[matrix.platform] }}";

const RAW_INPUTS: [&str; 4] = [
    "inputs.platforms",
    "inputs.disabled_checks",
    "inputs.min_coverage",
    "inputs.min_changed_line_coverage",
];

/// Returns the job names the parsed workflow declares, sorted.
fn job_names() -> Vec<String> {
    let document = document(WORKFLOW);
    let jobs = document["jobs"]
        .as_object()
        .unwrap_or_else(|| panic!("{WORKFLOW} has no jobs mapping"));
    let mut names: Vec<String> = jobs.keys().cloned().collect();
    names.sort();
    names
}

#[test]
fn the_workflow_is_valid_yaml_and_names_only_the_expected_jobs() {
    let mut wanted: Vec<String> = std::iter::once(VALIDATE.to_owned())
        .chain(PLATFORM_JOBS.iter().map(|name| (*name).to_owned()))
        .chain(std::iter::once(PREP.to_owned()))
        .chain(std::iter::once(AGGREGATE.to_owned()))
        .chain([
            "fuzz".to_owned(),
            "bench".to_owned(),
            "advisory-bump".to_owned(),
        ])
        .collect();
    wanted.sort();
    assert_eq!(
        job_names(),
        wanted,
        "{WORKFLOW} must name the validate job, the checks job, security-audit, the \
shared advisory preparation, the aggregate and fuzz"
    );
}

#[test]
fn the_validate_job_needs_nothing_so_it_rejects_input_first() {
    let validate = job(WORKFLOW, VALIDATE);
    assert!(
        validate.get("needs").is_none(),
        "{WORKFLOW} job {VALIDATE} must declare no needs, so it runs before every matrix job"
    );
    let steps = validate["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job {VALIDATE} has no steps"));
    assert!(
        steps.iter().any(|step| step["run"]
            .as_str()
            .is_some_and(|run| run.contains("@rules_dx//tools/ci:input_validation_bin"))),
        "{WORKFLOW} job {VALIDATE} never runs @rules_dx//tools/ci:input_validation_bin"
    );
}

#[test]
fn every_check_job_waits_for_the_validated_inputs() {
    for name in PLATFORM_JOBS {
        let wanted: Vec<&str> = if name == "security-audit" {
            vec![VALIDATE, PREP]
        } else {
            vec![VALIDATE]
        };
        assert_eq!(
            job(WORKFLOW, name)["needs"],
            json!(wanted),
            "{WORKFLOW} job {name} must need the validate job and nothing else it can skip"
        );
        let gate = text_at(&job(WORKFLOW, name)["if"], &format!("{name} if"));
        if name == MATRIX_JOB {
            assert_eq!(
                gate,
                "${{ needs.validate.outputs.checks != '[]' || !contains(format(',{0},', needs.validate.outputs.disabled), ',coverage,') }}",
                "{WORKFLOW} job {name} must skip only when no check and no coverage is enabled"
            );
            continue;
        }
        assert!(
            gate.contains(&format!("contains(format(',{{0}},', {DISABLED_REF})")),
            "{WORKFLOW} job {name} must gate on {DISABLED_REF}: {gate}"
        );
        assert!(
            gate.contains(&format!(",{name},")),
            "{WORKFLOW} job {name} must gate on its own check ID: {gate}"
        );
    }
}

#[test]
fn every_check_job_expands_its_matrix_from_the_validated_platforms() {
    for name in PLATFORM_JOBS {
        let job = job(WORKFLOW, name);
        assert_eq!(
            job["strategy"]["matrix"]["platform"],
            json!(MATRIX),
            "{WORKFLOW} job {name} must expand from {PLATFORMS_REF}"
        );
        assert_eq!(
            job["runs-on"],
            json!(RUNNER),
            "{WORKFLOW} job {name} must pick its runner from {RUNNERS_REF}"
        );
        assert_eq!(
            job["strategy"]["fail-fast"],
            json!(false),
            "{WORKFLOW} job {name} must let every cell finish"
        );
    }
}

/// Returns the checks job step whose name starts with the given prefix.
fn matrix_step(prefix: &str) -> Value {
    job(WORKFLOW, MATRIX_JOB)["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job {MATRIX_JOB} has no steps"))
        .iter()
        .find(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.starts_with(prefix))
        })
        .cloned()
        .unwrap_or_else(|| panic!("{WORKFLOW} job {MATRIX_JOB} has no step named {prefix:?}"))
}

#[test]
fn the_checks_job_runs_every_validated_check_in_one_job_per_platform() {
    let checks = job(WORKFLOW, MATRIX_JOB);
    assert_eq!(
        checks["name"],
        json!("checks (${{ matrix.platform }})"),
        "{WORKFLOW} job {MATRIX_JOB} is one job per platform"
    );
    assert_eq!(
        checks["timeout-minutes"],
        json!("${{ fromJSON(needs.validate.outputs.checks_timeout) }}")
    );
    assert!(
        checks["strategy"]["matrix"].get("check").is_none(),
        "{WORKFLOW} job {MATRIX_JOB} must not expand a per-check matrix"
    );
    let run = matrix_step("Run the checks");
    assert_eq!(
        run["env"]["DX_CHECK_LINES"],
        json!("${{ needs.validate.outputs.check_lines }}"),
        "{WORKFLOW} run step reads the validated check lines"
    );
    let script = text_at(&run["run"], "checks run step");
    assert!(
        !script.contains("${{"),
        "{WORKFLOW} job {MATRIX_JOB} must read the lines through env: {script}"
    );
    for needle in [
        "done <<< \"$DX_CHECK_LINES\"",
        "$GITHUB_STEP_SUMMARY",
        "exit \"$failed\"",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} run step must state {needle:?}: {script}"
        );
    }
    for id in ["lint", "typecheck", "test", "build"] {
        let upload = matrix_step(&format!("Upload the {id} reports"));
        assert_eq!(
            upload["with"]["name"],
            json!(format!(
                "dx-reports-{id}-${{{{ matrix.platform }}}}${{{{ needs.validate.outputs.artifact_suffix }}}}"
            )),
            "{WORKFLOW} job {MATRIX_JOB} must keep the report artifact names"
        );
        let scan = job(WORKFLOW, MATRIX_JOB)["steps"]
            .as_array()
            .and_then(|steps| steps.iter().find(|step| step["id"] == format!("scan-{id}")))
            .cloned()
            .unwrap_or_else(|| panic!("{WORKFLOW} job {MATRIX_JOB} has no scan-{id} step"));
        assert_eq!(
            scan["if"],
            json!(format!(
                "${{{{ always() && contains(needs.validate.outputs.checks, '\"id\":\"{id}\"') }}}}"
            )),
            "{WORKFLOW} scan {id} runs only when the check is enabled"
        );
        assert_eq!(
            upload["if"],
            json!(format!(
                "${{{{ always() && steps.scan-{id}.outcome == 'success' }}}}"
            )),
            "{WORKFLOW} upload {id} runs only after its scan passed"
        );
    }
    assert_eq!(
        matrix_step("Prove dx run works noninteractively in CI")["if"],
        json!("${{ !cancelled() && contains(needs.validate.outputs.checks, '\"id\":\"build\"') }}"),
        "{WORKFLOW} job {MATRIX_JOB} must prove dx run only when build is enabled"
    );
}

#[test]
fn the_coverage_steps_take_their_threshold_from_the_validated_inputs() {
    let coverage = job(WORKFLOW, MATRIX_JOB);
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job checks has no steps"));
    let thresholds = steps
        .iter()
        .filter(|step| step["env"]["DX_MIN_COVERAGE"] == json!(THRESHOLD))
        .count();
    assert_eq!(
        thresholds, 1,
        "{WORKFLOW} job checks must pass the validated threshold exactly once"
    );
}

/// Returns the coverage job step whose name starts with the given prefix.
fn coverage_step(prefix: &str) -> Value {
    let coverage = job(WORKFLOW, MATRIX_JOB);
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job checks has no steps"))
        .clone();
    steps
        .into_iter()
        .find(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} job checks has no step named {prefix:?}"))
}

#[test]
fn coverage_runs_once_and_keeps_no_text_log() {
    let coverage = job(WORKFLOW, MATRIX_JOB);
    let steps = coverage["steps"].as_array().expect("steps");
    assert!(
        !steps
            .iter()
            .any(|step| step["name"] == "Save the coverage event"),
        "{WORKFLOW} must not run coverage a second time"
    );
    let text = read_runfiles(WORKFLOW);
    assert!(
        !text.contains("dx_coverage.log"),
        "{WORKFLOW} must not read a text log"
    );
}

#[test]
fn the_coverage_comment_publishes_after_a_failed_gate() {
    let changed = coverage_step("Check changed-line coverage");
    let changed_run = text_at(&changed["run"], "changed-line step run");
    assert!(
        changed_run.contains("echo \"$rc\" > \"${RUNNER_TEMP}/dx_changed_lines.rc\""),
        "{WORKFLOW} changed-line step must record its exit code: {changed_run}"
    );
    assert!(
        changed_run.contains("coverage.ndjson") && changed_run.contains("changed_line_coverage"),
        "{WORKFLOW} changed-line step must read the coverage events: {changed_run}"
    );
    let changed_if = text_at(&changed["if"], "changed-line step if");
    assert!(
        changed_if.contains("always()"),
        "{WORKFLOW} changed-line step must record a failed gate: {changed_if}"
    );
    let run = coverage_step("Run coverage");
    let coverage_run = text_at(&run["run"], "coverage step run");
    for needle in [
        "extra+=(--min-changed-line-coverage \"$DX_MIN_CHANGED_LINE_COVERAGE\")",
        "echo skipped > \"${RUNNER_TEMP}/dx_changed_lines.rc\"",
        "coverage --output=json",
        "> \"$reports/coverage.ndjson\"",
    ] {
        assert!(
            coverage_run.contains(needle),
            "{WORKFLOW} coverage step must state {needle:?}: {coverage_run}"
        );
    }

    let render = coverage_step("Render first-party coverage summary");
    assert_eq!(
        render["if"],
        json!(
            "${{ always() && !contains(format(',{0},', needs.validate.outputs.disabled), ',coverage,') }}"
        )
    );
    let render_run = text_at(&render["run"], "render step run");
    for needle in [
        "dx_changed_lines.rc",
        "coverage.ndjson",
        "if [[ \"$dx_rc\" != \"0\" ]]; then status=\"FAIL\"; fi",
        "if [[ \"$changed_rc\" =~ ^[0-9]+$ && \"$changed_rc\" != \"0\" ]]; then status=\"FAIL\"; fi",
        "Changed-line gate exit: $changed_rc",
    ] {
        assert!(
            render_run.contains(needle),
            "{WORKFLOW} render step must state {needle:?}: {render_run}"
        );
    }

    let publish = coverage_step("Publish PR summary comment");
    assert_eq!(
        publish["if"],
        json!(
            "${{ always() && !contains(format(',{0},', needs.validate.outputs.disabled), ',coverage,') && github.event_name == 'pull_request' }}"
        ),
        "{WORKFLOW} must publish the coverage comment even after a failed gate"
    );
    let publish_run = text_at(&publish["run"], "publish step run");
    assert!(
        publish_run
            .contains("gh api --paginate \"repos/$base_repo/issues/$pr/comments?per_page=100\""),
        "{WORKFLOW} must read every comment page to find the marker: {publish_run}"
    );
    assert!(
        publish_run.contains("| tail -n 1"),
        "{WORKFLOW} must pick the last marker id across pages: {publish_run}"
    );
}

#[test]
fn the_aggregate_separates_a_disabled_check_from_a_lost_one() {
    let aggregate = job(WORKFLOW, AGGREGATE);
    assert_eq!(
        aggregate["needs"],
        json!([VALIDATE, MATRIX_JOB, "security-audit", PREP])
    );
    let step = aggregate["steps"][0].clone();
    assert_eq!(step["env"]["DISABLED"], json!(DISABLED));
    assert_eq!(step["env"]["CHECKS"], json!(CHECKS_LIST));
    let script = text_at(&step["run"], "aggregate steps.0.run");
    for needle in [
        "required=(checks security-audit advisory-snapshots)",
        "*,security-audit,*) add_disabled advisory-snapshots",
        "skipped) is_disabled \"$name\" || lost+=(\"$name\")",
        "skipped without disabled_checks",
        "\"$(result VALIDATE)\" != success",
        "[ \"$CHECKS\" = \"[]\" ] && is_disabled coverage",
        "add_disabled checks",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

#[test]
fn no_job_reads_a_raw_workflow_input() {
    let text = read_runfiles(WORKFLOW);
    for needle in RAW_INPUTS {
        let uses: Vec<&str> = text.lines().filter(|line| line.contains(needle)).collect();
        assert_eq!(
            uses.len(),
            1,
            "{WORKFLOW} reads {needle} outside the validate job: {uses:?}"
        );
        assert!(
            uses[0].contains("DX_"),
            "{WORKFLOW} must read {needle} only as a validate job environment value: {uses:?}"
        );
    }
}

const SCOPED: [&str; 5] = ["lint", "typecheck", "format", "test", "build"];

fn scope_text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

#[test]
fn affected_on_pr_is_a_boolean_defaulting_to_true() {
    let doc = document(WORKFLOW);
    let input = &doc["on"]["workflow_call"]["inputs"]["affected_on_pr"];
    assert_eq!(
        input["type"],
        json!("boolean"),
        "{WORKFLOW} affected_on_pr type"
    );
    assert_eq!(
        input["default"],
        json!(true),
        "{WORKFLOW} affected_on_pr default"
    );
}

#[test]
fn link_check_online_is_off_by_default_and_never_runs_on_pull_requests() {
    let doc = document(WORKFLOW);
    let input = &doc["on"]["workflow_call"]["inputs"]["link_check_online"];
    assert_eq!(input["type"], json!("boolean"), "{WORKFLOW} type");
    assert_eq!(input["default"], json!(false), "{WORKFLOW} default");
    let checks = job(WORKFLOW, MATRIX_JOB);
    let online = scope_text(&checks["env"]["DX_LINK_CHECK_ONLINE"]);
    for needle in ["!= 'pull_request'", "inputs.link_check_online"] {
        assert!(online.contains(needle), "DX_LINK_CHECK_ONLINE: {online}");
    }
    let script = text_at(&matrix_step("Run the checks")["run"], "checks run step");
    assert!(script.contains("args+=(--online)"), "{script}");
}

#[test]
fn buf_breaking_is_off_by_default_and_runs_only_on_pull_requests() {
    let doc = document(WORKFLOW);
    let input = &doc["on"]["workflow_call"]["inputs"]["buf_breaking_on_pr"];
    assert_eq!(input["type"], json!("boolean"), "{WORKFLOW} type");
    assert_eq!(input["default"], json!(false), "{WORKFLOW} default");
    let checks = job(WORKFLOW, MATRIX_JOB);
    let breaking = scope_text(&checks["env"]["DX_BUF_BREAKING"]);
    for needle in ["== 'pull_request'", "inputs.buf_breaking_on_pr"] {
        assert!(breaking.contains(needle), "DX_BUF_BREAKING: {breaking}");
    }
    let script = text_at(&matrix_step("Run the checks")["run"], "checks run step");
    assert!(script.contains("args+=(--breaking-against)"), "{script}");
}

#[test]
fn fuzz_is_off_by_default_and_uploads_crashes_for_thirty_days() {
    let doc = document(WORKFLOW);
    let inputs = &doc["on"]["workflow_call"]["inputs"];
    assert_eq!(inputs["fuzz"]["type"], json!("boolean"), "{WORKFLOW} type");
    assert_eq!(
        inputs["fuzz"]["default"],
        json!(false),
        "{WORKFLOW} default"
    );
    assert_eq!(inputs["fuzz_time"]["default"], json!("300s"));
    let fuzz = job(WORKFLOW, "fuzz");
    assert_eq!(scope_text(&fuzz["if"]), "${{ inputs.fuzz }}");
    let steps = fuzz["steps"].to_string();
    for needle in [
        "dx-fuzz-corpus-",
        ".dx/local/fuzz",
        "dx-fuzz-crashes",
        "\"retention-days\":30",
        "-- fuzz --time",
    ] {
        assert!(steps.contains(needle), "fuzz steps miss {needle}: {steps}");
    }
}

#[test]
fn bench_is_off_by_default_and_uploads_the_baseline_for_ninety_days() {
    let doc = document(WORKFLOW);
    let inputs = &doc["on"]["workflow_call"]["inputs"];
    assert_eq!(inputs["bench"]["type"], json!("boolean"), "{WORKFLOW} type");
    assert_eq!(
        inputs["bench"]["default"],
        json!(false),
        "{WORKFLOW} default"
    );
    assert_eq!(inputs["bench_regression_pct"]["default"], json!("20"));
    let bench = job(WORKFLOW, "bench");
    assert!(scope_text(&bench["if"]).contains("inputs.bench"));
    let steps = bench["steps"].to_string();
    for needle in [
        "dx-bench-linux_x86_64",
        "\"retention-days\":90",
        "bench --compare",
        "--max-regression=",
        "GITHUB_STEP_SUMMARY",
    ] {
        assert!(steps.contains(needle), "bench steps miss {needle}: {steps}");
    }
}

#[test]
fn the_checks_scope_is_affected_only_for_scoped_checks_on_pull_request() {
    let checks = job(WORKFLOW, MATRIX_JOB);
    let affected = scope_text(&checks["env"]["DX_AFFECTED"]);
    for needle in ["'pull_request'", "inputs.affected_on_pr"] {
        assert!(
            affected.contains(needle),
            "{WORKFLOW} {MATRIX_JOB} DX_AFFECTED misses {needle}: {affected}"
        );
    }
    assert!(
        !affected.contains("pull_request_target"),
        "{WORKFLOW} DX_AFFECTED: {affected}"
    );
    let script = text_at(&matrix_step("Run the checks")["run"], "checks run step");
    let ids = SCOPED.join("|");
    assert!(
        script.contains(&format!("{ids})")),
        "{WORKFLOW} run step must scope exactly {ids}: {script}"
    );
    for id in ["generate", "license-audit"] {
        assert!(
            !script.contains(&format!("|{id}")),
            "{WORKFLOW} run step must keep {id} on //...: {script}"
        );
    }
}

#[test]
fn the_checks_job_pins_the_base_and_runs_dx_on_the_scope() {
    let steps = job(WORKFLOW, MATRIX_JOB)["steps"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let checkout = steps
        .iter()
        .position(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/checkout"))
        })
        .unwrap_or_else(|| panic!("no checkout"));
    let depth = scope_text(&steps[checkout]["with"]["fetch-depth"]);
    assert!(
        depth.starts_with("${{ needs.validate.outputs.min_changed_line_coverage != '' && '0'"),
        "fetch-depth: {depth}"
    );
    assert!(depth.contains("DX_AFFECTED"), "fetch-depth: {depth}");
    let pin = steps
        .iter()
        .position(|s| {
            s["name"]
                .as_str()
                .is_some_and(|n| n.starts_with("Pin the pull request base"))
        })
        .unwrap_or_else(|| panic!("no pin step"));
    assert!(pin > checkout, "pins before checkout");
    let guard = scope_text(&steps[pin]["if"]);
    assert!(guard.contains("DX_AFFECTED"), "pin guard: {guard}");
    assert!(
        guard.contains("min_changed_line_coverage"),
        "pin guard: {guard}"
    );
    assert!(
        scope_text(&steps[pin]["env"]["GITHUB_BASE_REF"]).contains("github.base_ref"),
        "pin must read the base ref through env"
    );
    assert!(
        !scope_text(&steps[pin]["run"]).contains("${{"),
        "pin run has an expression"
    );
    let dx = steps
        .iter()
        .position(|s| s["name"] == json!("Run the checks"))
        .unwrap_or_else(|| panic!("no dx step"));
    assert!(pin < dx, "pins after dx");
    let run = scope_text(&steps[dx]["run"]);
    assert!(run.contains("\"$scope\""), "dx line: {run}");
}

#[test]
fn security_audit_keeps_the_whole_workspace() {
    let text = serde_json::to_string(&job(WORKFLOW, "security-audit")).unwrap_or_default();
    assert!(text.contains("//..."), "security-audit lost //...");
    assert!(
        !text.contains("DX_SCOPE"),
        "security-audit mentions DX_SCOPE"
    );
}

#[test]
fn coverage_scope_falls_back_when_a_threshold_is_set() {
    let run = text_at(&coverage_step("Run coverage")["run"], "coverage run");
    assert!(
        run.contains("\"$DX_AFFECTED\" = true ] && [ -z \"$DX_MIN_COVERAGE\" ] && [ -z \"$DX_MIN_CHANGED_LINE_COVERAGE\" ]"),
        "coverage scope: {run}"
    );
    assert!(run.contains("scope=//..."), "coverage scope: {run}");
    assert!(run.contains("\"$scope\""), "coverage scope: {run}");
}

#[cfg(unix)]
const CASES: [(&str, &str); 4] = [
    ("", "<!-- dx-coverage-summary: coverage linux_x86_64 -->"),
    ("b", "<!-- dx-coverage-summary: coverage linux_x86_64 b -->"),
    (
        "bb",
        "<!-- dx-coverage-summary: coverage linux_x86_64 bb -->",
    ),
    ("c", "<!-- dx-coverage-summary: coverage linux_x86_64 c -->"),
];

#[cfg(unix)]
fn coverage_env(
    root: &std::path::Path,
    runner: &std::path::Path,
    invocation: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("RUNNER_TEMP", runner.display().to_string()),
        (
            "GITHUB_STEP_SUMMARY",
            root.join("summary.md").display().to_string(),
        ),
        ("GITHUB_SHA", "abc123".to_owned()),
        ("GITHUB_SERVER_URL", "https://github.example".to_owned()),
        ("GITHUB_REPOSITORY", "devhusdk/rules_dx".to_owned()),
        ("GITHUB_RUN_ID", "1".to_owned()),
        ("DX_MIN_CHANGED_LINE_COVERAGE", String::new()),
        ("DX_INVOCATION_ID", invocation.to_owned()),
        ("PLATFORM", "linux_x86_64".to_owned()),
    ]
}

#[cfg(unix)]
fn run_render(runner: &std::path::Path, script: &str, env: &[(&str, String)]) {
    let mut command = std::process::Command::new("bash");
    command
        .args(["-e", "-c", script])
        .current_dir(runner)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run bash");
    assert!(
        done.status.success(),
        "render step failed: {}",
        String::from_utf8_lossy(&done.stderr)
    );
}

#[cfg(unix)]
#[test]
fn each_invocation_has_its_own_coverage_marker() {
    let render = text_at(
        &coverage_step("Render first-party coverage summary")["run"],
        "render step run",
    );
    let publish = coverage_step("Publish PR summary comment");
    let publish_run = text_at(&publish["run"], "publish step run");
    assert_eq!(
        publish["env"]["DX_INVOCATION_ID"],
        json!("${{ inputs.invocation_id }}")
    );
    assert_eq!(
        coverage_step("Render first-party coverage summary")["env"]["DX_INVOCATION_ID"],
        json!("${{ inputs.invocation_id }}")
    );
    assert!(
        publish_run.contains(
            r#"if [[ -n "$DX_INVOCATION_ID" ]]; then cell="$cell $DX_INVOCATION_ID"; fi"#
        ),
        "{WORKFLOW} publish step must add the invocation id to the cell: {publish_run}"
    );
    assert!(
        publish_run.contains(r#"marker="<!-- dx-coverage-summary: $cell -->""#),
        "{WORKFLOW} publish step must look up the whole marker line: {publish_run}"
    );
    assert!(
        publish_run.contains(r#"contains(\"$marker\")"#),
        "{WORKFLOW} publish step must match the marker as a substring: {publish_run}"
    );

    let scratch = dx_test_scratch::scratch("coverage-marker");
    let root = scratch.path();
    let mut bodies = Vec::new();
    for (index, &(invocation, marker)) in CASES.iter().enumerate() {
        let runner = root.join(format!("runner-{index}"));
        std::fs::create_dir_all(&runner).expect("create runner dir");
        run_render(&runner, &render, &coverage_env(root, &runner, invocation));
        let body = std::fs::read_to_string(runner.join("coverage_comment.md"))
            .expect("render writes the comment");
        assert_eq!(
            body.lines().next(),
            Some(marker),
            "marker for {invocation:?}"
        );
        bodies.push(body);
    }
    // The publish step finds a comment by substring match on its marker, so each
    // marker must appear in its own comment and in no other invocation's comment.
    for (index, &(invocation, marker)) in CASES.iter().enumerate() {
        for (other, body) in bodies.iter().enumerate() {
            assert_eq!(
                body.contains(marker),
                other == index,
                "{invocation:?} marker found in comment {other}"
            );
        }
    }
}
