//! Asserts the docs workflow validates one artifact before and after deploying it.

use serde_json::{json, Value};

use ci_workflow::{document, job, scripts, steps};
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-docs.yml";

const CALLER: &str = ".github/workflows/ci.yml";

const CHECK: &str = "docs-check";

const PUBLISH: &str = "docs-publish";

const RENDER: &str = "bazel run \"$@\" @rules_dx//:dx -- docs \"$DX_DOCS_SITE\" -- \"$@\"";

const DECLARED_SITE: &str = "bazel cquery \"$@\" --output=files \"$DX_DOCS_SITE\"";

const STAGED_CHECK: &str = "@rules_dx//tools/docs/site/check:site_check -- --rendered";

const CHECK_ARGS: [&str; 3] = ["\"${check[@]}\"", "DX_DOCS_LANDING", "DX_DOCS_FORBID"];

const FORBIDDEN: [&str; 2] = ["Treat warnings as errors", "docs_fixture"];

const INPUTS: [(&str, &str); 5] = [
    ("DX_DOCS_SITE", "site"),
    ("DX_DOCS_SITE_CHECK", "site_check"),
    ("DX_DOCS_FORBID", "forbid"),
    ("DX_DOCS_LANDING", "landing"),
    ("DX_DOCS_SMOKE_TEST", "smoke_test"),
];

const LABEL_VARS: [&str; 3] = ["DX_DOCS_SITE", "DX_DOCS_SITE_CHECK", "DX_DOCS_SMOKE_TEST"];

const INPUT_CHECK: &str = "Check the docs inputs";

const SOURCE_CHECK: &str = "Check the docs site sources";

const PAGE_ARTIFACT: &str = "actions/upload-pages-artifact";

const PAGE_DEPLOY: &str = "actions/deploy-pages";

const FETCH: &str =
    "curl --fail --silent --show-error --location --retry 10 --retry-delay 3 --retry-all-errors";

const PAGE_URL: &str = "${{ steps.deploy.outputs.page_url }}";

/// Returns one string field of a step, or an empty string when it has none.
fn field<'a>(step: &'a Value, key: &str) -> &'a str {
    step[key].as_str().unwrap_or_default()
}

/// Returns the first step of a job whose name is exactly `name`.
fn named_step(job_name: &str, name: &str) -> Value {
    steps(WORKFLOW, job_name)
        .into_iter()
        .find(|step| field(step, "name") == name)
        .unwrap_or_else(|| panic!("{WORKFLOW} job {job_name} has no step named {name:?}"))
}

/// Returns the index of the first step of a job naming `needle`.
fn step_index(name: &str, needle: &str) -> usize {
    let found = steps(WORKFLOW, name).iter().position(|step| {
        ["name", "run", "uses"]
            .into_iter()
            .any(|key| field(step, key).contains(needle))
    });
    found.unwrap_or_else(|| panic!("{WORKFLOW} job {name} has no step naming {needle:?}"))
}

#[test]
fn the_docs_check_renders_and_validates_the_artifact_without_deploying() {
    let script = scripts(WORKFLOW, CHECK);
    for needle in [
        RENDER,
        DECLARED_SITE,
        STAGED_CHECK,
        "cp -R \"$site/.\" \"$stage/\"",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {CHECK} must contain {needle:?}: {script}"
        );
    }
    for needle in CHECK_ARGS {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {CHECK} must contain {needle:?}: {script}"
        );
    }
    for action in [PAGE_ARTIFACT, PAGE_DEPLOY] {
        assert!(
            !steps(WORKFLOW, CHECK)
                .iter()
                .any(|step| field(step, "uses").contains(action)),
            "{WORKFLOW} job {CHECK} must never run {action}"
        );
    }
    assert!(
        !script.contains("PAGES_STAGE"),
        "{WORKFLOW} job {CHECK} must not stage an artifact for upload: {script}"
    );
    assert_eq!(
        job(WORKFLOW, PUBLISH)["needs"],
        json!([CHECK]),
        "{WORKFLOW} job {PUBLISH} must wait for {CHECK}"
    );
}

#[test]
fn the_publish_job_reads_the_declared_site_instead_of_a_guessed_path() {
    let script = scripts(WORKFLOW, PUBLISH);
    assert!(
        script.contains(DECLARED_SITE),
        "{WORKFLOW} job {PUBLISH} must read the declared site: {script}"
    );
    assert!(
        !script.contains("bazel-bin/tools/docs/site/user_site"),
        "{WORKFLOW} job {PUBLISH} must not guess a bazel-bin path: {script}"
    );
    assert!(
        script.contains("cp -R \"$site/.\" \"$stage/\""),
        "{WORKFLOW} job {PUBLISH} must stage the whole tree: {script}"
    );
    assert!(
        script.contains("echo \"PAGES_STAGE=$stage\" >> \"$GITHUB_ENV\""),
        "{WORKFLOW} job {PUBLISH} must publish the stage to later steps: {script}"
    );
}

#[test]
fn the_publish_job_validates_the_staged_tree_before_it_uploads() {
    let script = scripts(WORKFLOW, PUBLISH);
    for needle in [STAGED_CHECK, "--rendered \"$PAGES_STAGE\""] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {PUBLISH} must contain {needle:?}: {script}"
        );
    }
    for needle in CHECK_ARGS {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {PUBLISH} must contain {needle:?}: {script}"
        );
    }
    let stage = step_index(PUBLISH, "Stage rendered site");
    let validate = step_index(PUBLISH, "Validate staged site");
    let upload = step_index(PUBLISH, PAGE_ARTIFACT);
    assert!(
        stage < validate && validate < upload,
        "{WORKFLOW} job {PUBLISH} must stage at {stage}, validate at {validate} and upload at {upload}"
    );
}

#[test]
fn the_publish_job_smokes_the_page_url_it_deployed_before_the_clean_proof() {
    let publish_steps = steps(WORKFLOW, PUBLISH);
    let smoke_step = publish_steps
        .iter()
        .find(|step| field(step, "run").contains("curl "))
        .unwrap_or_else(|| panic!("{WORKFLOW} job {PUBLISH} never fetches the deployed site"));
    assert_eq!(
        field(&smoke_step["env"], "PAGES_URL"),
        PAGE_URL,
        "{WORKFLOW} job {PUBLISH} must smoke the URL deploy-pages reported"
    );
    let run = field(smoke_step, "run");
    for needle in [
        FETCH,
        "for file in index.html searchindex.js; do",
        "\"${PAGES_URL%/}/$file\"",
        "--test_env=DX_PAGES_URL=\"$PAGES_URL\"",
    ] {
        assert!(
            run.contains(needle),
            "{WORKFLOW} job {PUBLISH} smoke step must contain {needle:?}: {run}"
        );
    }
    let guard = run
        .find("if [ -n \"$DX_DOCS_SMOKE_TEST\" ]; then")
        .unwrap_or_else(|| {
            panic!("{WORKFLOW} must run the smoke test only when one is set: {run}")
        });
    let test = run
        .find("\"$DX_DOCS_SMOKE_TEST\"\n")
        .unwrap_or_else(|| panic!("{WORKFLOW} must run the caller's smoke test: {run}"));
    assert!(
        guard < test,
        "{WORKFLOW} must check DX_DOCS_SMOKE_TEST is set before it runs it: {run}"
    );
    let deploy = publish_steps
        .iter()
        .find(|step| field(step, "uses").contains(PAGE_DEPLOY))
        .unwrap_or_else(|| panic!("{WORKFLOW} job {PUBLISH} never deploys to Pages"));
    assert_eq!(
        field(deploy, "id"),
        "deploy",
        "{WORKFLOW} job {PUBLISH} must name the deploy step so its URL is readable"
    );
    let smoke = step_index(PUBLISH, FETCH);
    let clean = step_index(PUBLISH, "Prove checkout left clean");
    assert!(
        smoke < clean,
        "{WORKFLOW} job {PUBLISH} must smoke the deployment at {smoke} before the clean proof at {clean}"
    );
}

#[test]
fn the_docs_scope_reaches_the_shell_only_through_the_environment() {
    let text = read_runfiles(WORKFLOW);
    assert!(
        !text.contains("${{ inputs.docs_scope }} --"),
        "{WORKFLOW} must not interpolate docs_scope into a script"
    );
    let step = steps(WORKFLOW, CHECK)
        .into_iter()
        .find(|step| field(step, "name") == "Lint the docs scope")
        .unwrap_or_else(|| panic!("{WORKFLOW} job {CHECK} has no scope lint step"));
    assert_eq!(
        step["env"]["DX_DOCS_SCOPE"],
        json!("${{ inputs.docs_scope }}"),
        "{WORKFLOW} must pass docs_scope through DX_DOCS_SCOPE"
    );
    assert!(
        !field(&step, "run").contains("${{"),
        "{WORKFLOW} scope step must not interpolate expressions into its script"
    );
    for (name, job_steps) in [
        (CHECK, steps(WORKFLOW, CHECK)),
        (PUBLISH, steps(WORKFLOW, PUBLISH)),
    ] {
        for other in job_steps {
            assert!(
                !field(&other, "run").contains("inputs.docs_scope"),
                "{WORKFLOW} job {name} must not read docs_scope inside a script"
            );
            assert!(
                !field(&other, "run").contains("${{"),
                "{WORKFLOW} job {name} must not interpolate expressions into a script: {}",
                field(&other, "run")
            );
        }
    }
    let env = &document(WORKFLOW)["env"];
    for (var, input) in INPUTS {
        assert_eq!(
            env[var],
            json!(format!("${{{{ inputs.{input} }}}}")),
            "{WORKFLOW} must map input {input} to {var} in the workflow env"
        );
    }
}

#[test]
fn the_rules_dx_caller_keeps_its_own_site_values() {
    let with = &document(CALLER)["jobs"]["docs"]["with"];
    for (key, value) in [
        ("site", "//tools/docs/site:user_site"),
        ("site_check", "//tools/docs/site:user_site_check"),
        ("smoke_test", "//tools/docs/site:deploy_smoke_test"),
    ] {
        assert_eq!(with[key], json!(value), "{CALLER} docs job must pass {key}");
    }
    assert_eq!(
        with["landing"],
        json!(true),
        "{CALLER} docs job must pass landing: true"
    );
    let forbid = with["forbid"].as_str().unwrap_or_default();
    let lines: Vec<&str> = forbid.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(
        lines, FORBIDDEN,
        "{CALLER} docs job must forbid exactly {FORBIDDEN:?}"
    );
    let text = read_runfiles(WORKFLOW);
    for needle in FORBIDDEN.into_iter().chain(["user_site"]) {
        assert!(
            !text.contains(needle),
            "{WORKFLOW} must take {needle:?} from its caller, not hardcode it"
        );
    }
}

#[test]
fn the_docs_inputs_are_checked_before_any_step_reads_the_site() {
    let check = step_index(CHECK, INPUT_CHECK);
    let run = field(&named_step(CHECK, INPUT_CHECK), "run").to_owned();
    for var in LABEL_VARS {
        assert!(
            run.contains(var),
            "{WORKFLOW} step {INPUT_CHECK:?} must check {var}: {run}"
        );
    }
    for (index, step) in steps(WORKFLOW, CHECK).iter().enumerate() {
        if index != check && field(step, "run").contains("DX_DOCS_SITE") {
            assert!(
                check < index,
                "{WORKFLOW} job {CHECK} reads DX_DOCS_SITE at step {index} before {INPUT_CHECK:?} at {check}"
            );
        }
    }
}

#[test]
fn the_source_check_runs_only_when_the_caller_names_one() {
    let step = named_step(CHECK, SOURCE_CHECK);
    assert_eq!(
        field(&step, "if"),
        "inputs.site_check != ''",
        "{WORKFLOW} step {SOURCE_CHECK:?} must be skipped without site_check"
    );
    assert!(
        field(&step, "run").contains("docs --check \"$DX_DOCS_SITE_CHECK\""),
        "{WORKFLOW} step {SOURCE_CHECK:?} must check the caller's site_check label"
    );
}
