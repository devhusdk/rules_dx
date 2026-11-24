use ci_workflow::{job, steps, triggers};
use input_validation::PLATFORMS;

const WORKFLOW: &str = ".github/workflows/ci.yml";
const GATE_ACTION: &str = "./.github/actions/gate";
const LCOV: &str = "${{ runner.temp }}/coverage/lcov.info";
const JOBS: [(&str, Option<&str>); 2] = [("check", None), ("coverage", Some(LCOV))];

fn table() -> Vec<(String, String)> {
    PLATFORMS
        .iter()
        .map(|(name, runner)| (name.to_string(), runner.to_string()))
        .collect()
}

#[test]
fn workflow_runs_on_pull_requests_and_in_the_merge_queue() {
    let on = triggers(WORKFLOW).unwrap();
    assert!(
        on.get("pull_request").is_some(),
        "{WORKFLOW} needs a pull_request trigger"
    );
    assert!(
        on.get("pull_request_target").is_none(),
        "{WORKFLOW} must not use pull_request_target"
    );
    assert!(
        on.get("merge_group").is_some(),
        "{WORKFLOW} needs a merge_group trigger"
    );
}

#[test]
fn every_job_runs_on_every_platform() {
    for (name, _) in JOBS {
        let job = job(WORKFLOW, name).unwrap();
        let pairs: Vec<(String, String)> = job["strategy"]["matrix"]["include"]
            .as_array()
            .expect("matrix include")
            .iter()
            .map(|cell| {
                (
                    cell["platform"].as_str().unwrap_or_default().to_owned(),
                    cell["runner"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        assert_eq!(pairs, table(), "{name} matrix drifted from PLATFORMS");
        assert_eq!(job["runs-on"].as_str(), Some("${{ matrix.runner }}"));
        assert_eq!(job["strategy"]["fail-fast"].as_bool(), Some(false));
    }
}

#[test]
fn every_job_runs_dx_affected_through_the_gate_action() {
    for (name, lcov) in JOBS {
        let steps = steps(WORKFLOW, name).unwrap();
        let checkout = steps
            .iter()
            .position(|step| step["name"].as_str() == Some("Check out files in parallel"))
            .unwrap_or_else(|| panic!("{name} has no checkout step"));
        let gate = steps
            .iter()
            .position(|step| step["uses"].as_str() == Some(GATE_ACTION))
            .unwrap_or_else(|| panic!("{name} does not use {GATE_ACTION}"));
        assert!(checkout < gate, "{name} runs the gate before the checkout");
        let fetch = steps
            .get(checkout)
            .and_then(|step| step["run"].as_str())
            .unwrap_or_default();
        assert!(fetch.contains("--depth=2"), "{fetch}");
        let with = &steps.get(gate).expect("gate step")["with"];
        assert_eq!(with["command"].as_str(), Some(name));
        assert_eq!(with["lcov"].as_str(), lcov);
    }
}

#[test]
fn check_verifies_every_generated_file_on_one_platform() {
    let step = steps(WORKFLOW, "check")
        .unwrap()
        .into_iter()
        .find(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("@rules_dx//:dx -- generate -- \"$@\""))
        })
        .expect("whole-repo generate step");
    assert_eq!(
        step["if"].as_str(),
        Some(
            "${{ !cancelled() && matrix.platform == 'linux_x86_64' && github.event_name != 'pull_request' }}"
        )
    );
}

#[test]
fn coverage_uploads_the_lcov_per_platform() {
    let upload = steps(WORKFLOW, "coverage")
        .unwrap()
        .into_iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/upload-artifact@"))
        })
        .expect("upload step");
    assert_eq!(
        upload["with"]["name"].as_str(),
        Some("coverage-${{ matrix.platform }}")
    );
    assert_eq!(
        upload["with"]["path"].as_str(),
        Some("${{ runner.temp }}/coverage")
    );
    assert_eq!(upload["with"]["retention-days"].as_u64(), Some(7));
}
