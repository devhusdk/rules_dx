use serde_json::{json, Value};

use ci_workflow::{document, job, text_at};
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const VALIDATE: &str = "validate";

const PREP: &str = "advisory-snapshots";

const AGGREGATE: &str = "dx-ci";

const MATRIX_JOB: &str = "checks";

const PLATFORM_JOBS: [&str; 3] = [MATRIX_JOB, "security-audit", "coverage"];

const CHECKS_LIST: &str = "${{ needs.validate.outputs.checks }}";

const CHECKS_MATRIX: &str = "${{ fromJSON(needs.validate.outputs.checks) }}";

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
        .collect();
    wanted.sort();
    assert_eq!(
        job_names(),
        wanted,
        "{WORKFLOW} must name the validate job, the checks matrix, security-audit, coverage, the \
shared advisory preparation and the aggregate"
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
                gate, "${{ needs.validate.outputs.checks != '[]' }}",
                "{WORKFLOW} job {name} must skip only an empty checks list"
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
fn the_checks_job_runs_one_cell_per_validated_check_and_platform() {
    let checks = job(WORKFLOW, MATRIX_JOB);
    assert_eq!(
        checks["name"],
        json!("${{ matrix.check.id }} (${{ matrix.platform }})"),
        "{WORKFLOW} job {MATRIX_JOB} must keep the per-check names branch protection reads"
    );
    assert_eq!(
        checks["timeout-minutes"],
        json!("${{ matrix.check.timeout }}")
    );
    assert_eq!(checks["strategy"]["matrix"]["check"], json!(CHECKS_MATRIX));
    let run = matrix_step("Run ");
    for (key, value) in [
        ("DX_CHECK", "${{ matrix.check.id }}"),
        ("DX_ARGS", "${{ join(matrix.check.args, ' ') }}"),
        ("DX_REPORTS", "${{ join(matrix.check.reports, ' ') }}"),
        ("DX_PASSTHROUGH", "${{ matrix.check.passthrough }}"),
    ] {
        assert_eq!(
            run["env"][key],
            json!(value),
            "{WORKFLOW} run step env {key}"
        );
    }
    let script = text_at(&run["run"], "checks run step");
    assert!(
        !script.contains("${{"),
        "{WORKFLOW} job {MATRIX_JOB} must read the row through env: {script}"
    );
    let upload = matrix_step("Upload the ");
    assert_eq!(
        upload["with"]["name"],
        json!(
            "dx-reports-${{ matrix.check.id }}-${{ matrix.platform }}${{ needs.validate.outputs.artifact_suffix }}"
        ),
        "{WORKFLOW} job {MATRIX_JOB} must keep the report artifact names"
    );
    assert_eq!(
        matrix_step("Prove dx run works noninteractively in CI")["if"],
        json!("${{ matrix.check.id == 'build' }}"),
        "{WORKFLOW} job {MATRIX_JOB} must prove dx run on the build cell only"
    );
}

#[test]
fn the_coverage_job_takes_its_threshold_from_the_validated_inputs() {
    let coverage = job(WORKFLOW, "coverage");
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job coverage has no steps"));
    let thresholds = steps
        .iter()
        .filter(|step| step["env"]["DX_MIN_COVERAGE"] == json!(THRESHOLD))
        .count();
    assert_eq!(
        thresholds, 1,
        "{WORKFLOW} job coverage must pass the validated threshold exactly once"
    );
}

/// Returns the coverage job step whose name starts with the given prefix.
fn coverage_step(prefix: &str) -> Value {
    let coverage = job(WORKFLOW, "coverage");
    let steps = coverage["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job coverage has no steps"))
        .clone();
    steps
        .into_iter()
        .find(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} job coverage has no step named {prefix:?}"))
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
        changed_run.contains("/dx_coverage.log"),
        "{WORKFLOW} changed-line step must read the dx coverage log: {changed_run}"
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
    ] {
        assert!(
            coverage_run.contains(needle),
            "{WORKFLOW} coverage step must state {needle:?}: {coverage_run}"
        );
    }

    let render = coverage_step("Render first-party coverage summary");
    assert_eq!(render["if"], json!("always()"));
    let render_run = text_at(&render["run"], "render step run");
    for needle in [
        "dx_changed_lines.rc",
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
        json!("always() && github.event_name == 'pull_request'"),
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
        json!([VALIDATE, MATRIX_JOB, "security-audit", PREP, "coverage"])
    );
    let step = aggregate["steps"][0].clone();
    assert_eq!(step["env"]["DISABLED"], json!(DISABLED));
    assert_eq!(step["env"]["CHECKS"], json!(CHECKS_LIST));
    let script = text_at(&step["run"], "aggregate steps.0.run");
    for needle in [
        "os.environ[\"DISABLED\"]",
        "if \"security-audit\" in disabled:",
        "disabled.add(\"advisory-snapshots\")",
        "result == \"skipped\" and name not in disabled",
        "skipped without disabled_checks",
        "results.get(\"validate\") != \"success\"",
        "os.environ[\"CHECKS\"]",
        "disabled.add(\"checks\")",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

#[test]
fn no_matrix_expands_from_a_raw_workflow_input() {
    assert_eq!(
        job(WORKFLOW, MATRIX_JOB)["strategy"]["matrix"]["check"],
        json!(CHECKS_MATRIX),
        "{WORKFLOW} must expand matrix.check only from the validated checks list"
    );
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
fn the_checks_scope_is_affected_only_for_scoped_cells_on_pull_request() {
    let scope = scope_text(&job(WORKFLOW, MATRIX_JOB)["env"]["DX_SCOPE"]);
    for needle in [
        "'pull_request'",
        "inputs.affected_on_pr",
        "matrix.check.id",
        "'--affected'",
        "'//...'",
    ] {
        assert!(
            scope.contains(needle),
            "{WORKFLOW} {MATRIX_JOB} DX_SCOPE misses {needle}: {scope}"
        );
    }
    for id in SCOPED {
        assert!(
            scope.contains(&format!("\"{id}\"")),
            "{WORKFLOW} {MATRIX_JOB} DX_SCOPE must scope {id}: {scope}"
        );
    }
    for id in ["generate", "license-audit"] {
        assert!(
            !scope.contains(id),
            "{WORKFLOW} {MATRIX_JOB} DX_SCOPE must keep {id} on //...: {scope}"
        );
    }
    assert!(
        !scope.contains("pull_request_target"),
        "{WORKFLOW} DX_SCOPE: {scope}"
    );
}

#[test]
fn scoped_jobs_pin_the_base_and_run_dx_on_the_scope() {
    for name in [MATRIX_JOB, "coverage"] {
        let steps = job(WORKFLOW, name)["steps"]
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
            .unwrap_or_else(|| panic!("{name} has no checkout"));
        let depth = scope_text(&steps[checkout]["with"]["fetch-depth"]);
        assert!(depth.contains("DX_SCOPE"), "{name} fetch-depth: {depth}");
        let pin = steps
            .iter()
            .position(|s| {
                s["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with("Pin the pull request base"))
            })
            .unwrap_or_else(|| panic!("{name} has no pin step"));
        assert!(pin > checkout, "{name} pins before checkout");
        assert!(
            scope_text(&steps[pin]["if"]).contains("--affected"),
            "{name} pin guard"
        );
        assert!(
            scope_text(&steps[pin]["env"]["GITHUB_BASE_REF"]).contains("github.base_ref"),
            "{name} pin must read the base ref through env"
        );
        assert!(
            !scope_text(&steps[pin]["run"]).contains("${{"),
            "{name} pin run has an expression"
        );
        let dx = steps
            .iter()
            .position(|s| scope_text(&s["run"]).contains("@rules_dx//:dx"))
            .unwrap_or_else(|| panic!("{name} has no dx step"));
        assert!(pin < dx, "{name} pins after dx");
        let run = scope_text(&steps[dx]["run"]);
        assert!(run.contains("\"$DX_SCOPE\""), "{name} dx line: {run}");
        assert!(
            !run.contains("//..."),
            "{name} dx line still names //...: {run}"
        );
    }
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
    let coverage = job(WORKFLOW, "coverage");
    let scope = scope_text(&coverage["env"]["DX_SCOPE"]);
    for needle in [
        "'pull_request'",
        "inputs.affected_on_pr",
        "needs.validate.outputs.min_coverage == ''",
        "needs.validate.outputs.min_changed_line_coverage == ''",
        "'--affected'",
        "'//...'",
    ] {
        assert!(
            scope.contains(needle),
            "coverage DX_SCOPE misses {needle}: {scope}"
        );
    }
    let steps = coverage["steps"].as_array().cloned().unwrap_or_default();
    let depth = scope_text(&steps[0]["with"]["fetch-depth"]);
    assert!(
        depth.starts_with("${{ needs.validate.outputs.min_changed_line_coverage != '' && '0'"),
        "coverage fetch-depth: {depth}"
    );
}
