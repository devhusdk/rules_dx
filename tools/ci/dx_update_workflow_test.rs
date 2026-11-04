use ci_workflow::steps;
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/dx-update.yml";

const APP_ACTION: &str = "actions/create-github-app-token@";

const GH_SUBCOMMANDS: [&str; 8] = [
    "api", "auth", "issue", "pr", "release", "repo", "run", "workflow",
];

/// Returns the raw workflow text.
fn text() -> String {
    read_runfiles(WORKFLOW)
}

/// Returns the run script of a step, or an empty string.
fn run_of(step: &serde_json::Value) -> &str {
    step["run"].as_str().unwrap_or_default()
}

/// Returns the step name, or an empty string.
fn name_of(step: &serde_json::Value) -> &str {
    step["name"].as_str().unwrap_or_default()
}

/// Returns the step-scoped GH_TOKEN, or an empty string.
fn step_token(step: &serde_json::Value) -> &str {
    step["env"]["GH_TOKEN"].as_str().unwrap_or_default()
}

/// Reports whether a run script calls a gh subcommand.
fn invokes_gh(run: &str) -> bool {
    run.lines().any(|line| {
        let spaced = line.replace("$(", " ");
        let words: Vec<&str> = spaced.split_whitespace().collect();
        words
            .windows(2)
            .any(|pair| pair[0] == "gh" && GH_SUBCOMMANDS.contains(&pair[1]))
    })
}

#[test]
fn invokes_gh_matches_only_gh_subcommands() {
    assert!(invokes_gh("gh api repos/a/b"));
    assert!(invokes_gh("sha=\"$(gh api repos/a/b --jq .sha || true)\""));
    assert!(invokes_gh("if ! gh api x >y; then"));
    assert!(invokes_gh("gh pr create --title t"));
    assert!(!invokes_gh("if command -v gh >/dev/null 2>&1; then"));
    assert!(!invokes_gh("echo \"--- GitHub releases (gh) ---\""));
    assert!(!invokes_gh("echo \"client (gh api); dx update\""));
    assert!(!invokes_gh("GH_TOKEN=x"));
}

#[test]
fn app_action_is_pinned_to_a_sha_with_a_tag() {
    let body = text();
    let app_uses: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- uses:") && line.contains(APP_ACTION))
        .collect();
    assert_eq!(app_uses.len(), 1, "{WORKFLOW} needs one {APP_ACTION} step");
    let pinned = app_uses[0]
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
        "{WORKFLOW} must carry a tag comment on the {APP_ACTION} pin"
    );
}

#[test]
fn app_token_comes_before_the_dx_step() {
    let steps = steps(WORKFLOW, "update");
    let app = steps
        .iter()
        .position(|step| {
            step["uses"]
                .as_str()
                .unwrap_or_default()
                .starts_with(APP_ACTION)
        })
        .unwrap_or_else(|| panic!("{WORKFLOW} must carry the app token step"));
    let dx = steps
        .iter()
        .position(|step| run_of(step).contains("tools/dx update --pr"))
        .unwrap_or_else(|| panic!("{WORKFLOW} must run tools/dx update --pr"));
    assert!(app < dx, "the app token must be minted before the dx step");
    assert_eq!(steps[app]["id"], serde_json::json!("app"));
    assert_eq!(steps[app]["if"], serde_json::json!("env.HAS_APP == 'true'"));
}

#[test]
fn git_pushes_through_gh_without_leaking_the_token() {
    let body = text();
    assert!(
        body.contains("gh auth setup-git"),
        "{WORKFLOW} must configure git through gh"
    );
    assert!(
        !body.contains("x-access-token:${"),
        "{WORKFLOW} must never embed the token in a remote URL"
    );
    assert!(
        !body.contains("echo $GH_TOKEN") && !body.contains("echo \"$GH_TOKEN"),
        "{WORKFLOW} must never print the token"
    );
}

#[test]
fn schedule_and_dispatch_only() {
    let body = text();
    assert!(body.contains("schedule:"), "{WORKFLOW} needs a schedule");
    assert!(
        body.contains("workflow_dispatch"),
        "{WORKFLOW} needs workflow_dispatch"
    );
    assert!(
        !body.contains("pull_request:") && !body.contains("pull_request_target"),
        "{WORKFLOW} must not run on pull requests"
    );
}

#[test]
fn every_gh_step_sets_gh_token() {
    let mut found = 0;
    for step in steps(WORKFLOW, "update") {
        if invokes_gh(run_of(&step)) {
            found += 1;
            assert!(
                !step_token(&step).is_empty(),
                "{WORKFLOW} step {:?} runs gh without GH_TOKEN in its env",
                name_of(&step)
            );
        }
    }
    assert!(found >= 1, "{WORKFLOW} must carry a step that runs gh");
}
