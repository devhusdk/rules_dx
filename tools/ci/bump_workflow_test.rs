use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/bump.yml";

const SETS: &str = "cli/bump/src/sets.rs";

const GHA: &str = "cli/bump/src/gha.rs";

const APP_ACTION: &str = "actions/create-github-app-token";

const APP_ID_SECRET: &str = "secrets.BUMP_APP_ID";

const APP_KEY_SECRET: &str = "secrets.BUMP_APP_PRIVATE_KEY";

/// Returns the workflow text, which must exist.
fn text() -> String {
    read_runfiles(WORKFLOW)
}

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> serde_json::Value {
    let body = text();
    yaml_serde::from_str(&body)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
}

/// Returns the steps of the widen-one job, which must declare some.
fn steps() -> Vec<serde_json::Value> {
    document()["jobs"]["widen-one"]["steps"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("{WORKFLOW} job widen-one declares no steps"))
}

/// Returns the index of the step whose name contains `fragment`.
fn step_index(fragment: &str) -> usize {
    steps()
        .iter()
        .position(|step| {
            step["name"]
                .as_str()
                .is_some_and(|name| name.contains(fragment))
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} names no step containing {fragment:?}"))
}

/// Returns the `run` scripts of the widen-one job, joined.
fn scripts() -> String {
    steps()
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the owned manifest and lock paths every bump set declares.
fn owned_paths() -> Vec<String> {
    let source = read_runfiles(SETS);
    let manifests = section(
        &source,
        "pub fn manifests",
        "pub fn spans_manifest_directory",
    );
    let locks = section(&source, "pub fn locks", "pub fn needs_update_refresh");
    let mut paths: Vec<String> = [manifests, locks]
        .iter()
        .flat_map(|body| quoted(body))
        .collect();
    paths.push(workflow_dir());
    paths.sort();
    paths.dedup();
    assert!(
        !paths.is_empty(),
        "{SETS} declares no owned manifest or lock path"
    );
    paths
}

/// Returns the source between the lines starting `start` and `end`.
fn section(source: &str, start: &str, end: &str) -> String {
    let (_, rest) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("{SETS} declares no {start}"));
    let (body, _) = rest
        .split_once(end)
        .unwrap_or_else(|| panic!("{SETS} leaves {start} open before {end}"));
    body.to_owned()
}

/// Returns every double-quoted literal of a source section.
fn quoted(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some((_, tail)) = rest.split_once('"') {
        if let Some((literal, head)) = tail.split_once('"') {
            out.push(literal.to_owned());
            rest = head;
        } else {
            break;
        }
    }
    out
}

/// Returns the workflow directory the GithubActions set owns.
fn workflow_dir() -> String {
    let source = read_runfiles(GHA);
    let line = source
        .lines()
        .find(|line| line.contains("WORKFLOW_DIR: &str"))
        .unwrap_or_else(|| panic!("{GHA} declares no WORKFLOW_DIR constant"));
    let (_, value) = line
        .rsplit_once('"')
        .and_then(|(head, _)| head.rsplit_once('"'))
        .unwrap_or_else(|| panic!("{GHA} gives WORKFLOW_DIR no string value"));
    value.to_owned()
}

#[test]
fn checkout_keeps_credentials_disabled() {
    let all = steps();
    let found = all
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/checkout@"))
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} checks out with no pinned checkout action"));
    assert_eq!(
        found["with"]["persist-credentials"],
        serde_json::Value::Bool(false),
        "{WORKFLOW} must keep persist-credentials false so only the App token can push"
    );
}

#[test]
fn push_token_comes_from_a_pinned_app_action() {
    let mint = step_index("Mint bump push token");
    let push = step_index("Open one PR");
    assert!(
        mint < push,
        "{WORKFLOW} mints the App token after the push step that needs it"
    );
    let step = &steps()[mint];
    let uses = step["uses"].as_str().unwrap_or_default();
    assert!(
        uses.starts_with(&format!("{APP_ACTION}@")),
        "{WORKFLOW} mint step uses {uses:?} instead of {APP_ACTION}"
    );
    assert_eq!(
        step["with"]["app-id"].as_str().unwrap_or_default(),
        format!("${{{{ {APP_ID_SECRET} }}}}"),
        "{WORKFLOW} mint step must read the App id from {APP_ID_SECRET}"
    );
    assert_eq!(
        step["with"]["private-key"].as_str().unwrap_or_default(),
        format!("${{{{ {APP_KEY_SECRET} }}}}"),
        "{WORKFLOW} mint step must read the App key from {APP_KEY_SECRET}"
    );
    let token = format!(
        "${{{{ steps.{}.outputs.token }}}}",
        step["id"].as_str().unwrap_or_default()
    );
    assert!(
        !step["id"].as_str().unwrap_or_default().is_empty(),
        "{WORKFLOW} mint step names no id, so the push step cannot read its token"
    );
    assert_eq!(
        steps()[push]["env"]["GH_TOKEN"]
            .as_str()
            .unwrap_or_default(),
        token,
        "{WORKFLOW} push step must run gh and git with the App token output"
    );
}

#[test]
fn credentials_are_validated_before_mutation() {
    let proof = step_index("Validate bump App credentials");
    let widen = step_index("Widen one requirement");
    let push = step_index("Open one PR");
    assert!(
        proof < widen && proof < push,
        "{WORKFLOW} validates credentials after the mutation it must guard"
    );
    let step = &steps()[proof];
    let script = step["run"].as_str().unwrap_or_default();
    assert_eq!(
        step["env"]["BUMP_APP_ID"].as_str().unwrap_or_default(),
        format!("${{{{ {APP_ID_SECRET} }}}}"),
        "{WORKFLOW} credential proof must read the App id from {APP_ID_SECRET}"
    );
    assert_eq!(
        step["env"]["BUMP_APP_PRIVATE_KEY"]
            .as_str()
            .unwrap_or_default(),
        format!("${{{{ {APP_KEY_SECRET} }}}}"),
        "{WORKFLOW} credential proof must read the App key from {APP_KEY_SECRET}"
    );
    assert!(
        script.contains("BUMP_APP_ID") && script.contains("BUMP_APP_PRIVATE_KEY"),
        "{WORKFLOW} credential proof never reads both App secrets"
    );
    assert!(
        script.contains("exit 1"),
        "{WORKFLOW} credential proof never fails closed on missing secrets"
    );
}

#[test]
fn push_and_pr_creation_use_the_app_token_only() {
    let body = text();
    assert!(
        !body.contains("github.token"),
        "{WORKFLOW} still pushes or opens the PR with github.token, whose events skip required CI"
    );
    let script = scripts();
    assert!(
        script.contains("gh auth setup-git"),
        "{WORKFLOW} push step never hands the App token to git"
    );
    assert!(
        script.contains("git push --set-upstream origin"),
        "{WORKFLOW} push step never pushes the bump branch"
    );
    assert!(
        script.contains("gh pr create"),
        "{WORKFLOW} never opens the bump PR with the App identity"
    );
}

#[test]
fn staging_is_scoped_to_set_owned_files() {
    let body = text();
    assert!(
        !body.contains("git add -A"),
        "{WORKFLOW} still stages every change instead of the bumped set"
    );
    let script = scripts();
    for path in owned_paths() {
        assert!(
            script.contains(&path),
            "{WORKFLOW} stages no path for the set owning {path:?}"
        );
    }
    assert!(
        script.contains("\"??\""),
        "{WORKFLOW} never rejects unexpected untracked files before staging"
    );
    assert!(
        script.contains("Unknown set"),
        "{WORKFLOW} never fails closed on a selector outside the known sets"
    );
}

#[test]
fn fork_safety_and_automerge_toggle_survive() {
    let script = scripts();
    assert!(
        script.contains("Fork PR detected"),
        "{WORKFLOW} lost the fork guard that blocks pushes from fork runs"
    );
    assert!(
        script.contains("gh pr merge --auto"),
        "{WORKFLOW} lost the automerge path that waits on the required checks"
    );
    assert!(
        script.contains("Automerge off"),
        "{WORKFLOW} lost the human merge path when automerge stays off"
    );
}
