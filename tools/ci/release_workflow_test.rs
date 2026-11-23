use std::path::Path;

use ci_workflow::{document, job, scripts, steps, strings, workflow_call};
use dx_testing::resolve_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-release.yml";

fn gate_names(field: &str) -> Vec<String> {
    let run = resolve_runfiles("deploy/private/src/run.rs");
    let dir = run.parent().expect("run.rs has a directory");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read deploy/private/src") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            names.extend(declared(&path, field));
        }
    }
    names.sort();
    names.dedup();
    names
}

fn declared(path: &Path, field: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut names = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix(&format!("{field}: ")) else {
            continue;
        };
        let Some(list) = rest.trim_end_matches(',').trim().strip_prefix('&') else {
            let name = rest.trim_matches(|c| c == '"' || c == ',');
            if !name.starts_with("TEST_") {
                names.push(name.to_owned());
            }
            continue;
        };
        for name in list.trim_matches(|c| c == '[' || c == ']').split(',') {
            let name = name.trim().trim_matches('"');
            if !name.is_empty() && !name.starts_with("TEST_") {
                names.push(name.to_owned());
            }
        }
    }
    names
}

fn publish_script() -> String {
    scripts(WORKFLOW, "release").unwrap()
}

#[test]
fn gate_names_are_found() {
    let names = gate_names("approved");
    assert!(names.len() >= 15, "found too few gates: {names:?}");
    assert!(names.contains(&"PUB_PUBLISH_APPROVED".to_owned()));
}

#[test]
fn publish_step_exports_only_the_approval_gates() {
    let script = publish_script();
    let words: Vec<&str> = script
        .split(|c: char| !(c.is_ascii_uppercase() || c == '_'))
        .collect();
    for name in gate_names("approved") {
        assert!(
            words.contains(&name.as_str()),
            "the release job does not export {name}"
        );
    }
    assert!(
        !words.contains(&"DX_DEPLOY_LIVE"),
        "the release job exports DX_DEPLOY_LIVE; dx deploy --apply sets it"
    );
}

#[test]
fn publish_step_maps_every_gate_credential_from_secrets() {
    let credentials = gate_names("credentials");
    assert!(
        credentials.contains(&"NPM_API_TOKEN".to_owned()),
        "{credentials:?}"
    );
    let step = steps(WORKFLOW, "release")
        .unwrap()
        .into_iter()
        .find(|step| step["name"] == "Publish")
        .expect("release job has a Publish step");
    for name in credentials {
        assert_eq!(
            step["env"][&name],
            format!("${{{{ secrets.{name} }}}}"),
            "the Publish step does not map {name} from secrets"
        );
    }
}

#[test]
fn query_matches_the_release_tag_exactly() {
    let script = scripts(WORKFLOW, "plan").unwrap();
    let regex = script
        .split_once(r#"attr(tags, ""#)
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(regex, _)| regex)
        .unwrap_or_else(|| panic!("the plan step has no attr(tags, ...) query:\n{script}"));
    assert!(
        !regex.contains(r"\\"),
        "the release tag regex must use single backslashes: {regex}"
    );
    assert!(
        regex.contains("release"),
        "the regex must name the release tag: {regex}"
    );
    let build = std::fs::read_to_string(resolve_runfiles("deploy/private/BUILD.bazel")).unwrap();
    assert!(
        build.contains(&format!(r#"attr(tags, "{regex}""#)),
        "//deploy/private:release_tag_query must test the workflow regex {regex}"
    );
}

#[test]
fn release_waits_for_dry_run_and_plan_and_plan_runs_on_tags_or_without_publish() {
    let release = job(WORKFLOW, "release").unwrap();
    let needs: Vec<&str> = release["needs"]
        .as_array()
        .expect("release needs a list")
        .iter()
        .filter_map(|n| n.as_str())
        .collect();
    assert!(
        needs.contains(&"dry-run") && needs.contains(&"plan"),
        "{needs:?}"
    );
    let dry = job(WORKFLOW, "dry-run").unwrap();
    assert_eq!(dry["needs"], serde_json::json!(["plan"]));
    let condition = job(WORKFLOW, "plan").unwrap()["if"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        condition.contains("startsWith(github.ref, 'refs/tags/')")
            && condition.contains("!inputs.publish"),
        "{condition}"
    );
}

#[test]
fn release_job_needs_publish_and_a_tag() {
    let condition = job(WORKFLOW, "release").unwrap()["if"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(condition.contains("inputs.publish"), "{condition}");
    assert!(
        condition.contains("startsWith(github.ref, 'refs/tags/')"),
        "{condition}"
    );
}

#[test]
fn publish_input_defaults_true() {
    let publish = &workflow_call(WORKFLOW).unwrap()["inputs"]["publish"];
    assert_eq!(publish["type"], "boolean");
    assert_eq!(publish["default"], true);
}

#[test]
fn only_the_release_job_can_write() {
    let doc = document(WORKFLOW).unwrap();
    assert_eq!(doc["permissions"], serde_json::json!({"contents": "read"}));
    for name in ["plan", "dry-run"] {
        assert_eq!(
            job(WORKFLOW, name).unwrap()["permissions"],
            serde_json::json!({"contents": "read"}),
            "{name}"
        );
    }
    let perms = job(WORKFLOW, "release").unwrap()["permissions"].clone();
    for scope in ["contents", "id-token", "attestations", "packages"] {
        assert_eq!(perms[scope], "write", "release {scope}");
    }
}

#[test]
fn only_the_publish_step_applies() {
    assert!(!scripts(WORKFLOW, "dry-run").unwrap().contains("--apply"));
    assert!(!scripts(WORKFLOW, "plan").unwrap().contains("--apply"));
    assert!(publish_script().contains("deploy --apply"));
}

#[test]
fn deploys_run_dx_through_cmd_on_windows() {
    for name in ["dry-run", "release"] {
        let script = scripts(WORKFLOW, name).unwrap();
        assert!(
            script.contains("cmd.exe //c tools\\\\dx.cmd deploy"),
            "{name}"
        );
        assert!(steps(WORKFLOW, name).unwrap().len() >= 3);
    }
}

#[test]
fn summary_trap_runs_on_exit() {
    assert!(publish_script().contains("trap summary EXIT"));
}

#[test]
fn resume_command_names_the_calling_workflow() {
    let script = publish_script();
    assert!(script.contains(r#"caller="${GITHUB_WORKFLOW_REF%@*}""#), "{script}");
    assert!(script.contains("gh workflow run ${caller##*/} --ref"), "{script}");
    assert!(!script.contains("gh workflow run release.yml"), "{script}");
}

const E2E: &str = ".github/workflows/release-e2e.yml";

#[test]
fn release_e2e_runs_only_on_e2e_tags() {
    assert_eq!(
        ci_workflow::triggers(E2E).unwrap(),
        serde_json::json!({"push": {"tags": ["v0.0.0-e2e.*"]}})
    );
    assert_eq!(
        document(E2E).unwrap()["permissions"],
        serde_json::json!({"contents": "read"})
    );
}

#[test]
fn release_e2e_calls_the_local_reusable_release_without_secrets() {
    let calls = ci_workflow::reusable_calls(E2E).unwrap();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let (_, called, job) = &calls[0];
    assert_eq!(called, "reusable-release.yml");
    assert_eq!(job["uses"], "./.github/workflows/reusable-release.yml");
    assert!(job.get("secrets").is_none(), "{job}");
    let deployables = job["with"]["deployables"].as_str().unwrap_or_default();
    assert!(deployables.contains("//tools/testing/release_e2e:a_github"), "{deployables}");
    assert!(deployables.contains("//tools/testing/release_e2e:b_refuses"), "{deployables}");
    for scope in ["contents", "id-token", "attestations", "packages"] {
        assert_eq!(job["permissions"][scope], "write", "{scope}");
    }
}

#[test]
fn workflow_text_is_readable() {
    assert!(
        strings(WORKFLOW)
            .unwrap()
            .iter()
            .any(|text| text.contains("rules-dx-release"))
    );
}
