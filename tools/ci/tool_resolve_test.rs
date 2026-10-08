use serde_json::{json, Value};

use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const AUDIT: &str = "security-audit";

const RESOLVE_TARGET: &str = "@rules_dx//cli/tool_resolve";

const TOOL_LABEL: &str = "@dx_tools//:gitleaks";

const TOOL_VAR: &str = "DX_GITLEAKS_BIN";

/// Returns the parsed workflow, which must be valid Actions YAML.
fn document() -> Value {
    let text = read_runfiles(WORKFLOW);
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{WORKFLOW} is not valid workflow YAML: {error}"))
}

/// Returns the steps of one job, which must declare some.
fn steps(name: &str) -> Vec<Value> {
    document()["jobs"][name]["steps"]
        .as_array()
        .unwrap_or_else(|| panic!("{WORKFLOW} job {name} declares no steps"))
        .to_vec()
}

/// Returns the `run` scripts of one job, joined.
fn scripts(name: &str) -> String {
    steps(name)
        .iter()
        .filter_map(|step| step["run"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn one_resolve_step_runs_the_pinned_native_command() {
    let audit = steps(AUDIT);
    let resolve: Vec<&Value> = audit
        .iter()
        .filter(|step| {
            step["run"]
                .as_str()
                .is_some_and(|script| script.contains(RESOLVE_TARGET))
        })
        .collect();
    assert_eq!(
        resolve.len(),
        1,
        "{WORKFLOW} job {AUDIT} must run {RESOLVE_TARGET} exactly once"
    );
    assert_eq!(
        resolve[0]["name"],
        json!("Resolve hermetic Gitleaks"),
        "{WORKFLOW} job {AUDIT} must name its tool resolution step"
    );
    let script = resolve[0]["run"].as_str().unwrap_or_default();
    for needle in [
        "--label @dx_tools//:gitleaks",
        "--var DX_GITLEAKS_BIN",
        "bazel run",
    ] {
        assert!(
            script.contains(needle),
            "{WORKFLOW} job {AUDIT} resolution must pass {needle}: {script}"
        );
    }
}

#[test]
fn no_audit_step_parses_tool_paths_with_host_utilities() {
    let all = scripts(AUDIT);
    for absent in ["cquery", "tail -1", "output_base", "2>/dev/null"] {
        assert!(
            !all.contains(absent),
            "{WORKFLOW} job {AUDIT} must resolve tools without {absent}: {all}"
        );
    }
    assert!(
        !all.contains("GITHUB_ENV"),
        "{WORKFLOW} job {AUDIT} must let the native command write the environment: {all}"
    );
}

#[test]
fn the_audit_still_receives_the_resolved_tool() {
    assert!(
        scripts(AUDIT).contains(TOOL_LABEL),
        "{WORKFLOW} job {AUDIT} must still resolve {TOOL_LABEL}"
    );
    assert!(
        scripts(AUDIT).contains(TOOL_VAR),
        "{WORKFLOW} job {AUDIT} must still export {TOOL_VAR}"
    );
}
