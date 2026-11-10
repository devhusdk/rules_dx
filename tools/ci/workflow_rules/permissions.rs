use serde_json::Value;

/// Text that shows a step uses a write scope, one entry per scope GitHub grants.
/// A scope missing here fails the check, so a new grant needs a reviewed marker.
const MARKERS: &[(&str, &[&str])] = &[
    (
        "actions",
        &[
            "/cancel",
            "/force-cancel",
            "gh cache delete",
            "gh workflow run",
        ],
    ),
    ("attestations", &["actions/attest@", "deploy --apply"]),
    ("artifact-metadata", &["actions/attest@"]),
    ("checks", &["check-runs"]),
    (
        "contents",
        &[
            "gh release ",
            "git push",
            "dx update --pr",
            "update advisory --pr",
            "deploy --apply",
        ],
    ),
    (
        "id-token",
        &[
            "cosign",
            "actions/deploy-pages@",
            "actions/attest@",
            "deploy --apply",
        ],
    ),
    ("issues", &["gh issue ", "gh pr comment"]),
    ("packages", &["docker push", "deploy --apply"]),
    ("pages", &["actions/deploy-pages@"]),
    ("statuses", &["/statuses/"]),
    (
        "pull-requests",
        &[
            "gh pr ",
            "/issues/",
            "/pulls",
            "dx update --pr",
            "update advisory --pr",
        ],
    ),
];

/// Scopes that act through OIDC, so a step needs no GITHUB_TOKEN to use them.
const TOKENLESS: &[&str] = &["id-token"];

const TOKEN_NEEDLES: &[&str] = &["github.token", "secrets.GITHUB_TOKEN"];

fn parse(name: &str, text: &str) -> Value {
    ci_workflow::parse(name, text)
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
    let jobs = ci_workflow::jobs_of(workflow);
    if jobs.is_empty() {
        return vec![format!("{name} declares no jobs")];
    }
    for (id, job) in jobs {
        if job.get("uses").is_some() {
            // A calling job's grants are checked against the called workflow elsewhere.
            continue;
        }
        let scopes = ci_workflow::effective_permissions(workflow, job);
        let Some(scopes) = scopes.as_object() else {
            problems.push(format!(
                "{name} job {id} must list its permissions as a map, got {scopes}"
            ));
            continue;
        };
        let steps = ci_workflow::steps_of(job);
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
    for (name, workflow) in ci_workflow::all() {
        assert!(
            workflow["permissions"].is_object(),
            "{name} must set top-level permissions as a map, got {}",
            workflow["permissions"]
        );
    }
}

#[test]
fn every_write_scope_has_a_step_that_uses_it() {
    let problems: Vec<String> = ci_workflow::all()
        .iter()
        .flat_map(|(name, workflow)| unused_write_scopes(name, workflow))
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

/// Callers whose jobs call a reusable workflow and must grant what it requests.
const CALLERS: [&str; 2] = [
    "examples/docs-ci/caller.yml",
    ".github/workflows/ci.yml",
];

/// Ranks a permission level so a grant can be compared with a request.
fn permission_rank(level: &str) -> u8 {
    match level {
        "write" => 2,
        "read" => 1,
        _ => 0,
    }
}

/// Adds each (scope, level) of one `permissions` map, which must be a map when present.
fn add_scopes(scopes: &mut Vec<(String, String)>, permissions: &Value, what: &str) {
    if permissions.is_null() {
        return;
    }
    let Some(map) = permissions.as_object() else {
        panic!("{what} must list its permissions as a map, got {permissions}");
    };
    for (scope, level) in map {
        let Some(level) = level.as_str() else {
            panic!("{what} grants {scope} a non-string level: {level}");
        };
        let grant = (scope.clone(), level.to_owned());
        if !scopes.contains(&grant) {
            scopes.push(grant);
        }
    }
}

/// Returns every scope a reusable workflow requests: its top level and every job's.
fn requested_scopes(path: &str) -> Vec<(String, String)> {
    let workflow = ci_workflow::document(path);
    let mut scopes = Vec::new();
    add_scopes(&mut scopes, &workflow["permissions"], path);
    for (id, job) in ci_workflow::jobs(path) {
        add_scopes(
            &mut scopes,
            &job["permissions"],
            &format!("{path} job {id}"),
        );
    }
    scopes
}

#[test]
fn calling_jobs_grant_every_scope_the_called_workflow_requests() {
    let mut calls = 0;
    for caller in CALLERS {
        for (id, name, job) in ci_workflow::reusable_calls(caller) {
            calls += 1;
            let requested = requested_scopes(&format!(".github/workflows/{name}"));
            assert!(
                !requested.is_empty(),
                "{name} requests no permissions, so {caller} job {id} grants none"
            );
            let mut granted = Vec::new();
            add_scopes(
                &mut granted,
                &job["permissions"],
                &format!("{caller} job {id}"),
            );
            for (scope, level) in &granted {
                let ceiling = requested
                    .iter()
                    .filter(|(name, _)| name == scope)
                    .map(|(_, level)| permission_rank(level))
                    .max();
                assert!(
                    ceiling.is_some_and(|ceiling| permission_rank(level) <= ceiling),
                    "{caller} job {id} grants {scope}: {level}, but {name} requests it at most at {ceiling:?}; grant exactly what the called jobs request"
                );
            }
            for (scope, level) in &requested {
                let held = granted.iter().find(|(name, _)| name == scope);
                assert!(
                    held.is_some_and(|(_, held)| permission_rank(held) >= permission_rank(level)),
                    "{caller} job {id} calls {name} but grants {scope} at {:?}, needs {level}",
                    held.map(|(_, held)| held.as_str())
                );
            }
        }
    }
    assert!(calls > 0, "no caller calls a reusable workflow");
}

#[test]
fn gate_job_checks_evidence_statuses_with_read_only_statuses_scope() {
    let (_, ci) = ci_workflow::all()
        .into_iter()
        .find(|(name, _)| name.ends_with("/ci.yml"))
        .expect("ci.yml");
    let (_, gate) = ci_workflow::jobs_of(&ci)
        .into_iter()
        .find(|(id, _)| *id == "gate")
        .expect("gate job");
    assert_eq!(gate["permissions"]["statuses"], "read");
    assert_eq!(gate["permissions"]["contents"], "read");
    let steps = ci_workflow::steps_of(gate);
    let first = &steps[0];
    assert_eq!(first["if"], "${{ github.event_name == 'merge_group' }}");
    let run = first["run"].as_str().expect("run");
    assert!(run.contains("set -euo pipefail"), "{run}");
    assert!(run.contains("/commits/$sha/statuses"), "{run}");
    assert!(run.contains("evidence\\/"), "{run}");
    assert!(
        first["env"]["HEAD_REF"]
            .as_str()
            .is_some_and(|v| v.contains("github.event.merge_group.head_ref")),
        "{first}"
    );
}
