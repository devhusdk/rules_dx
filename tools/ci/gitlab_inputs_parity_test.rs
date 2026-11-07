use serde_json::Value;

use ci_workflow::document;
use dx_testing::read_runfiles;

const GITHUB: &str = ".github/workflows/reusable-consumer.yml";

const GITLAB: &str = "ci/gitlab/dx.gitlab-ci.yml";

/// GitHub inputs the GitLab template has no use for: it runs on one Linux runner,
/// reads the remote cache from `DX_REMOTE_CACHE`, and opens no pull requests.
const GITHUB_ONLY: [&str; 4] = ["advisory_pr", "invocation_id", "platforms", "remote_cache"];

fn names(inputs: &Value, path: &str) -> Vec<String> {
    let mut names: Vec<String> = inputs
        .as_object()
        .unwrap_or_else(|| panic!("{path} declares no inputs"))
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

#[test]
fn gitlab_takes_the_github_inputs_under_the_same_names() {
    let github: Vec<String> = names(&document(GITHUB)["on"]["workflow_call"]["inputs"], GITHUB)
        .into_iter()
        .filter(|name| !GITHUB_ONLY.contains(&name.as_str()))
        .collect();
    let text = read_runfiles(GITLAB);
    let (header, _) = text
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("{GITLAB} has no spec header"));
    let spec: Value = yaml_serde::from_str(header)
        .unwrap_or_else(|error| panic!("{GITLAB} spec is not valid YAML: {error}"));
    let gitlab = names(&spec["spec"]["inputs"], GITLAB);
    assert_eq!(gitlab, github, "{GITLAB} and {GITHUB} inputs differ");
}

#[test]
fn every_github_only_input_still_exists() {
    let github = names(&document(GITHUB)["on"]["workflow_call"]["inputs"], GITHUB);
    for name in GITHUB_ONLY {
        assert!(
            github.iter().any(|seen| seen == name),
            "{GITHUB} lost {name}"
        );
    }
}
