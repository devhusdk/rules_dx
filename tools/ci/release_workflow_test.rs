use std::path::Path;

use ci_workflow::{document, job, scripts, steps};
use dx_testing::{read_runfiles, resolve_runfiles};

const WORKFLOW: &str = ".github/workflows/reusable-release.yml";

/// Returns every gate env var name that an `OwnerGate` in deploy/rules/src declares.
fn gate_names() -> Vec<String> {
    let run = resolve_runfiles("deploy/rules/src/run.rs");
    let dir = run.parent().expect("run.rs has a directory");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read deploy/rules/src") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            names.extend(declared(&path));
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Returns the approved and live names declared in one source file.
fn declared(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        for field in ["approved: \"", "live: \""] {
            let Some(rest) = line.strip_prefix(field) else {
                continue;
            };
            let name = rest.split('"').next().unwrap_or_default();
            if !name.starts_with("TEST_") {
                names.push(name.to_owned());
            }
        }
    }
    names
}

fn publish_script() -> String {
    scripts(WORKFLOW, "release")
}

#[test]
fn gate_names_are_found() {
    let names = gate_names();
    assert!(names.len() >= 20, "found too few gates: {names:?}");
    assert!(names.contains(&"PUB_PUBLISH_LIVE".to_owned()));
}

#[test]
fn publish_step_exports_every_deploy_gate() {
    let script = publish_script();
    let words: Vec<&str> = script
        .split(|c: char| !(c.is_ascii_uppercase() || c == '_'))
        .collect();
    for name in gate_names() {
        assert!(
            words.contains(&name.as_str()),
            "the release job does not export {name}"
        );
    }
}

#[test]
fn query_matches_the_release_tag_exactly() {
    let script = scripts(WORKFLOW, "plan");
    assert!(
        script.contains(r#"attr(tags, "[\\[ ]release[,\\]]", kind(dx_deployment, //...))"#),
        "the plan query must match the exact release tag:\n{script}"
    );
}

#[test]
fn release_waits_for_dry_run_and_plan_and_plan_needs_a_tag() {
    let release = job(WORKFLOW, "release");
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
    let dry = job(WORKFLOW, "dry-run");
    assert_eq!(dry["needs"], serde_json::json!(["plan"]));
    let condition = job(WORKFLOW, "plan")["if"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        condition.contains("startsWith(github.ref, 'refs/tags/')"),
        "{condition}"
    );
}

#[test]
fn only_the_release_job_can_write() {
    let doc = document(WORKFLOW);
    assert_eq!(doc["permissions"], serde_json::json!({"contents": "read"}));
    for name in ["plan", "dry-run"] {
        assert_eq!(
            job(WORKFLOW, name)["permissions"],
            serde_json::json!({"contents": "read"}),
            "{name}"
        );
    }
    let perms = job(WORKFLOW, "release")["permissions"].clone();
    for scope in ["contents", "id-token", "attestations", "packages"] {
        assert_eq!(perms[scope], "write", "release {scope}");
    }
}

#[test]
fn only_the_publish_step_applies() {
    assert!(!scripts(WORKFLOW, "dry-run").contains("--apply"));
    assert!(!scripts(WORKFLOW, "plan").contains("--apply"));
    assert!(publish_script().contains("deploy --apply"));
}

#[test]
fn deploys_run_dx_through_cmd_on_windows() {
    for name in ["dry-run", "release"] {
        let script = scripts(WORKFLOW, name);
        assert!(
            script.contains("cmd.exe //c tools\\\\dx.cmd deploy"),
            "{name}"
        );
        assert!(steps(WORKFLOW, name).len() >= 3);
    }
}

#[test]
fn summary_trap_runs_on_exit() {
    assert!(publish_script().contains("trap summary EXIT"));
}

#[test]
fn workflow_text_is_readable() {
    assert!(read_runfiles(WORKFLOW).contains("rules-dx-release"));
}
