use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/bump.yml";

const ALLOWED: &str = "tools/ci/bump_auth_fixtures/allowed.txt";

const UNEXPECTED: &str = "tools/ci/bump_auth_fixtures/unexpected.txt";

const APP_ACTION: &str = "actions/create-github-app-token@";

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> serde_json::Value {
    let text = read_runfiles(WORKFLOW);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
}

/// Returns the steps of the widen-one job as values.
fn steps() -> Vec<serde_json::Value> {
    let document = document();
    document["jobs"]["widen-one"]["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job widen-one declares no steps"))
        .clone()
}

/// Returns the raw workflow text.
fn text() -> String {
    read_runfiles(WORKFLOW)
}

/// Returns fixture paths, one per non-empty line.
fn fixture(path: &str) -> Vec<String> {
    read_runfiles(path)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn bump_authenticates_through_a_pinned_app_token() {
    let body = text();
    let app_uses: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- uses:"))
        .filter(|line| line.contains(APP_ACTION))
        .collect();
    assert_eq!(
        app_uses.len(),
        1,
        "{WORKFLOW} must carry exactly one {APP_ACTION} step, found {}",
        app_uses.len()
    );
    let uses = app_uses[0];
    let pinned = uses
        .split_once(APP_ACTION)
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let (sha, comment) = pinned
        .split_once(char::is_whitespace)
        .unwrap_or((pinned, ""));
    assert!(
        sha.len() == 40
            && sha
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{WORKFLOW} must pin {APP_ACTION} at a 40-char lowercase commit SHA, found {sha:?}"
    );
    assert!(
        !comment.trim().trim_start_matches('#').trim().is_empty(),
        "{WORKFLOW} must carry a trailing tag comment on the {APP_ACTION} pin"
    );
    for step in steps() {
        let uses = step["uses"].as_str().unwrap_or_default();
        if uses.starts_with(APP_ACTION) {
            assert_eq!(
                step["id"],
                serde_json::json!("bump-app"),
                "{WORKFLOW} app token step must carry id bump-app"
            );
            let with = &step["with"];
            assert!(
                with["app-id"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("BUMP_APP_ID"),
                "{WORKFLOW} app token step must read secrets.BUMP_APP_ID"
            );
            assert!(
                with["private-key"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("BUMP_APP_PRIVATE_KEY"),
                "{WORKFLOW} app token step must read secrets.BUMP_APP_PRIVATE_KEY"
            );
            assert!(
                step["if"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("inputs.selector"),
                "{WORKFLOW} app token step must run only for dispatched selector runs"
            );
        }
    }
}

#[test]
fn missing_app_secrets_fail_before_any_mutation() {
    let body = text();
    let marker = "Validate bump credentials";
    assert!(
        body.contains(marker),
        "{WORKFLOW} must carry a {marker:?} step before any mutation"
    );
    for needle in [
        "BUMP_APP_ID",
        "BUMP_APP_PRIVATE_KEY",
        "bump_auth_missing",
        "exit 1",
    ] {
        assert!(
            body.contains(needle),
            "{WORKFLOW} credential validation must state {needle:?}"
        );
    }
    let validate = body.find(marker).unwrap_or_default();
    let widen = body
        .find("Widen one requirement plus resolver-owned refresh")
        .unwrap_or(body.len());
    let push = body.find("git push --set-upstream").unwrap_or(body.len());
    assert!(
        validate < widen && validate < push,
        "{WORKFLOW} credential validation must precede the widen and push steps"
    );
}

#[test]
fn push_and_pr_creation_use_the_app_identity() {
    let body = text();
    assert!(
        !body.contains("GH_TOKEN: ${{ github.token }}"),
        "{WORKFLOW} must not create PRs with github.token, which skips required CI"
    );
    assert!(
        body.contains("GH_TOKEN: ${{ steps.bump-app.outputs.token }}"),
        "{WORKFLOW} PR creation must run as the App identity"
    );
    assert!(
        body.contains("gh auth setup-git"),
        "{WORKFLOW} must configure git through gh so the push uses the App identity"
    );
    assert!(
        !body.contains("x-access-token:${"),
        "{WORKFLOW} must never embed the token in a remote URL"
    );
    for leak in [
        "echo $BUMP_TOKEN",
        "echo ${BUMP_TOKEN",
        "echo \"$BUMP_TOKEN",
    ] {
        assert!(
            !body.contains(leak),
            "{WORKFLOW} must never print the token to logs: {leak:?}"
        );
    }
}

#[test]
fn only_selector_scoped_manifests_stage() {
    let body = text();
    assert!(
        !body.contains("git add -A"),
        "{WORKFLOW} must not stage the whole tree with git add -A"
    );
    for arm in [
        "bazel)",
        "cargo)",
        "github-actions|gha)",
        "go|gomod)",
        "maven)",
        "npm)",
        "nuget)",
    ] {
        assert!(
            body.contains(arm),
            "{WORKFLOW} staging scope must name the {arm:?} selector set"
        );
    }
    for path in fixture(ALLOWED) {
        assert!(
            body.contains(&path),
            "{WORKFLOW} staging scope must list allowed path {path:?}"
        );
    }
    for path in fixture(UNEXPECTED) {
        let staged = body
            .lines()
            .filter(|line| line.trim_start().starts_with("allowed=("))
            .any(|line| line.contains(&path));
        assert!(
            !staged,
            "{WORKFLOW} must never stage unexpected path {path:?}"
        );
    }
    for needle in [
        "bump_scope_unknown",
        "bump_scope_rejected",
        "git status --porcelain",
        "git add --",
        "git diff --cached --quiet",
    ] {
        assert!(
            body.contains(needle),
            "{WORKFLOW} scoped staging must state {needle:?}"
        );
    }
}

#[test]
fn one_dependency_per_pr_with_fork_safety_and_automerge() {
    let body = text();
    for needle in [
        "inputs.selector != '' && inputs.version != ''",
        "Fork PR detected",
        "plan only, never push",
        "gh pr create",
        "gh pr merge --auto --squash",
        "concurrency + App token + fork-safety",
    ] {
        assert!(
            body.contains(needle),
            "{WORKFLOW} must keep {needle:?}: one dep per PR, fork safety, automerge gate"
        );
    }
}
