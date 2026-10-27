use serde_json::{json, Value};

use ci_workflow::{document, job, text_at};
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const VALIDATE: &str = "validate";

const PREP: &str = "advisory-snapshots";

const AGGREGATE: &str = "dx-ci";

const CHECKS: [&str; 9] = [
    "lint",
    "typecheck",
    "format",
    "generate",
    "security-audit",
    "license-audit",
    "test",
    "build",
    "coverage",
];

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
        .chain(CHECKS.iter().map(|name| (*name).to_owned()))
        .chain(std::iter::once(PREP.to_owned()))
        .chain(std::iter::once(AGGREGATE.to_owned()))
        .collect();
    wanted.sort();
    assert_eq!(
        job_names(),
        wanted,
        "{WORKFLOW} must name the validate job, the nine checks, the shared advisory \
preparation and the aggregate"
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
    for name in CHECKS {
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
    for name in CHECKS {
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
    }
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
        changed_run.contains("echo skipped > \"${RUNNER_TEMP}/dx_changed_lines.rc\""),
        "{WORKFLOW} changed-line step must record a skipped run: {changed_run}"
    );

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
    let mut wanted: Vec<&str> = vec![VALIDATE];
    for name in CHECKS {
        wanted.push(name);
        if name == "security-audit" {
            wanted.push(PREP);
        }
    }
    assert_eq!(aggregate["needs"], json!(wanted));
    let step = aggregate["steps"][0].clone();
    assert_eq!(step["env"]["DISABLED"], json!(DISABLED));
    let script = text_at(&step["run"], "aggregate steps.0.run");
    for needle in [
        "os.environ[\"DISABLED\"]",
        "if \"security-audit\" in disabled:",
        "disabled.add(\"advisory-snapshots\")",
        "result == \"skipped\" and name not in disabled",
        "skipped without disabled_checks",
        "results.get(\"validate\") != \"success\"",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {AGGREGATE} must state {needle:?}: {script}"
        );
    }
}

#[test]
fn no_matrix_expands_from_a_raw_workflow_input() {
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
