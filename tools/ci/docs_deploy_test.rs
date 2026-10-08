//! Asserts the docs workflow validates one artifact before and after deploying it.

use serde_json::{json, Value};

use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-docs.yml";

const CHECK: &str = "docs-check";

const PUBLISH: &str = "docs-publish";

const RENDER: &str = "bazel run \"$@\" @rules_dx//:dx -- docs -- \"$@\"";

const DECLARED_SITE: &str = "bazel cquery \"$@\" --output=files //docs/site:user_site";

const STAGED_CHECK: &str = "//docs/site/check:site_check -- --rendered";

const FORBIDDEN: [&str; 3] = [
    "--forbid \"Treat warnings as errors\"",
    "--forbid \"AccountService\"",
    "--forbid \"Python demo package\"",
];

const PAGE_ARTIFACT: &str = "actions/upload-pages-artifact";

const PAGE_DEPLOY: &str = "actions/deploy-pages";

const SMOKE: &str = "//docs/site:deploy_smoke_test";

const PAGE_URL: &str = "${{ steps.deploy.outputs.page_url }}";

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> Value {
    let text = read_runfiles(WORKFLOW);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
}

/// Returns one job of the parsed workflow.
fn job(name: &str) -> Value {
    let found = document()["jobs"][name].clone();
    assert!(
        found.is_object(),
        "{WORKFLOW} names no {name:?} job: {}",
        document()["jobs"]
    );
    found
}

/// Returns the steps of one job, which must declare some.
fn steps(name: &str) -> Vec<Value> {
    let found = job(name)["steps"].as_array().cloned();
    found.unwrap_or_else(|| panic!("{WORKFLOW} job {name} declares no steps"))
}

/// Returns the `run` scripts of one job, joined.
fn scripts(name: &str) -> String {
    steps(name)
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns one string field of a step, or an empty string when it has none.
fn field<'a>(step: &'a Value, key: &str) -> &'a str {
    step[key].as_str().unwrap_or_default()
}

/// Returns the index of the first step of a job naming `needle`.
fn step_index(name: &str, needle: &str) -> usize {
    let found = steps(name).iter().position(|step| {
        ["name", "run", "uses"]
            .into_iter()
            .any(|key| field(step, key).contains(needle))
    });
    found.unwrap_or_else(|| panic!("{WORKFLOW} job {name} has no step naming {needle:?}"))
}

#[test]
fn the_docs_check_renders_and_validates_the_artifact_without_deploying() {
    let script = scripts(CHECK);
    for needle in [
        RENDER,
        DECLARED_SITE,
        STAGED_CHECK,
        "--landing",
        "cp -R \"$site/.\" \"$stage/\"",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {CHECK} must contain {needle:?}: {script}"
        );
    }
    for needle in FORBIDDEN {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {CHECK} must contain {needle:?}: {script}"
        );
    }
    for action in [PAGE_ARTIFACT, PAGE_DEPLOY] {
        assert!(
            !steps(CHECK)
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
        job(PUBLISH)["needs"],
        json!([CHECK]),
        "{WORKFLOW} job {PUBLISH} must wait for {CHECK}"
    );
}

#[test]
fn the_publish_job_reads_the_declared_site_instead_of_a_guessed_path() {
    let script = scripts(PUBLISH);
    assert!(
        script.contains(DECLARED_SITE),
        "{WORKFLOW} job {PUBLISH} must read the declared site: {script}"
    );
    assert!(
        !script.contains("bazel-bin/docs/site/user_site"),
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
    let script = scripts(PUBLISH);
    for needle in [STAGED_CHECK, "--rendered \"$PAGES_STAGE\"", "--landing"] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {PUBLISH} must contain {needle:?}: {script}"
        );
    }
    for needle in FORBIDDEN {
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
    let publish_steps = steps(PUBLISH);
    let smoke_step = publish_steps
        .iter()
        .find(|step| field(step, "run").contains(SMOKE))
        .unwrap_or_else(|| panic!("{WORKFLOW} job {PUBLISH} runs no deploy smoke test"));
    assert_eq!(
        field(&smoke_step["env"], "PAGES_URL"),
        PAGE_URL,
        "{WORKFLOW} job {PUBLISH} must smoke the URL deploy-pages reported"
    );
    assert!(
        field(smoke_step, "run").contains("--test_env=DX_PAGES_URL=\"$PAGES_URL\""),
        "{WORKFLOW} job {PUBLISH} must pass the deployed URL to {SMOKE}"
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
    let smoke = step_index(PUBLISH, SMOKE);
    let clean = step_index(PUBLISH, "Prove checkout left clean");
    assert!(
        smoke < clean,
        "{WORKFLOW} job {PUBLISH} must smoke the deployment at {smoke} before the clean proof at {clean}"
    );
}
