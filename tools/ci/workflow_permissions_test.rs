use serde_json::Value;

use dx_testing::read_runfiles;

/// Text that shows a step uses a write scope, one entry per scope GitHub grants.
/// A scope missing here fails the check, so a new grant needs a reviewed marker.
const MARKERS: &[(&str, &[&str])] = &[
    ("actions", &["/cancel", "/force-cancel"]),
    ("attestations", &["actions/attest@"]),
    ("artifact-metadata", &["actions/attest@"]),
    ("checks", &["check-runs"]),
    ("contents", &["gh release ", "git push"]),
    (
        "id-token",
        &["cosign", "actions/deploy-pages@", "actions/attest@"],
    ),
    ("packages", &["docker push"]),
    ("pages", &["actions/deploy-pages@"]),
    ("pull-requests", &["gh pr ", "/issues/", "/pulls"]),
];

/// Scopes that act through OIDC, so a step needs no GITHUB_TOKEN to use them.
const TOKENLESS: &[&str] = &["id-token"];

const TOKEN_NEEDLES: &[&str] = &["github.token", "secrets.GITHUB_TOKEN"];

/// Returns every workflow under `.github/workflows`, so new workflows join the check.
fn workflows() -> Vec<String> {
    dx_testing::workflow_files()
}

fn parse(name: &str, text: &str) -> Value {
    yaml_serde::from_str(text)
        .unwrap_or_else(|error| panic!("{name} is not valid workflow YAML: {error}"))
}

fn document(name: &str) -> Value {
    parse(name, &read_runfiles(name))
}

/// Returns the scopes a job holds: its own `permissions`, else the workflow's.
fn effective_scopes<'a>(workflow: &'a Value, job: &'a Value) -> &'a Value {
    match job.get("permissions") {
        Some(own) => own,
        None => &workflow["permissions"],
    }
}

/// Returns true when one step shows it uses `scope`.
fn step_uses_scope(step: &Value, scope: &str, markers: &[&str]) -> bool {
    let text = serde_json::to_string(step).expect("serialize step");
    if !markers.iter().any(|marker| text.contains(marker)) {
        return false;
    }
    step.get("uses").is_some()
        || TOKENLESS.contains(&scope)
        || TOKEN_NEEDLES.iter().any(|needle| text.contains(needle))
}

/// Returns one message per write scope that no step of its job uses.
fn unused_write_scopes(name: &str, workflow: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(jobs) = workflow["jobs"].as_object() else {
        return vec![format!("{name} declares no jobs")];
    };
    for (id, job) in jobs {
        if job.get("uses").is_some() {
            // A calling job's grants are checked against the called workflow elsewhere.
            continue;
        }
        let scopes = effective_scopes(workflow, job);
        let Some(scopes) = scopes.as_object() else {
            problems.push(format!(
                "{name} job {id} must list its permissions as a map, got {scopes}"
            ));
            continue;
        };
        let steps = job["steps"].as_array().cloned().unwrap_or_default();
        for (scope, level) in scopes {
            if level != "write" {
                continue;
            }
            let Some((_, markers)) = MARKERS.iter().find(|(known, _)| known == scope) else {
                problems.push(format!(
                    "{name} job {id} grants {scope}: write, which has no usage marker"
                ));
                continue;
            };
            if !steps
                .iter()
                .any(|step| step_uses_scope(step, scope, markers))
            {
                problems.push(format!(
                    "{name} job {id} grants {scope}: write, but no step uses it"
                ));
            }
        }
    }
    problems
}

#[test]
fn every_workflow_sets_top_level_permissions() {
    let names = workflows();
    assert!(!names.is_empty(), "DX_WORKFLOWS names no workflows");
    for name in names {
        let workflow = document(&name);
        assert!(
            workflow["permissions"].is_object(),
            "{name} must set top-level permissions as a map, got {}",
            workflow["permissions"]
        );
    }
}

#[test]
fn every_write_scope_has_a_step_that_uses_it() {
    let problems: Vec<String> = workflows()
        .iter()
        .flat_map(|name| unused_write_scopes(name, &document(name)))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn an_unused_checks_scope_fails() {
    let fixture = parse(
        "fixture",
        "permissions:\n  contents: read\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n      checks: write\n    steps:\n      - run: echo lint\n        env:\n          GH_TOKEN: ${{ github.token }}\n",
    );
    assert_eq!(
        unused_write_scopes("fixture", &fixture),
        vec!["fixture job lint grants checks: write, but no step uses it".to_owned()]
    );
}

#[test]
fn an_inherited_unused_write_scope_fails() {
    let fixture = parse(
        "fixture",
        "permissions:\n  contents: read\n  pull-requests: write\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo gh pr view\n",
    );
    assert_eq!(
        unused_write_scopes("fixture", &fixture),
        vec!["fixture job build grants pull-requests: write, but no step uses it".to_owned()]
    );
}

#[test]
fn a_used_write_scope_passes_and_an_unknown_one_fails() {
    let fixture = parse(
        "fixture",
        "permissions:\n  contents: read\njobs:\n  upload:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: write\n      discussions: write\n    steps:\n      - run: gh release upload v1 out/*\n        env:\n          GH_TOKEN: ${{ github.token }}\n",
    );
    assert_eq!(
        unused_write_scopes("fixture", &fixture),
        vec!["fixture job upload grants discussions: write, which has no usage marker".to_owned()]
    );
}
