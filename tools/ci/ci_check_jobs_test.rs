use ci_workflow::{job, steps, triggers};
use input_validation::PLATFORMS;

const WORKFLOW: &str = ".github/workflows/ci.yml";
const GATE_ACTION: &str = "./.github/actions/gate";
const JOB: &str = "check";
const LCOV: &str = "${{ runner.temp }}/coverage/lcov.info";
const GATE: &str = "${{ matrix.platform == 'linux_x86_64' && 'check coverage' || 'check' }}";

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
fn the_check_job_runs_on_every_platform() {
    let job = job(WORKFLOW, JOB).unwrap();
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
    assert_eq!(pairs, table(), "{JOB} matrix drifted from PLATFORMS");
    assert_eq!(job["runs-on"].as_str(), Some("${{ matrix.runner }}"));
    assert_eq!(job["strategy"]["fail-fast"].as_bool(), Some(false));
}

#[test]
fn the_check_job_runs_dx_affected_through_the_gate_action() {
    let steps = steps(WORKFLOW, JOB).unwrap();
    let checkout = steps
        .iter()
        .position(|step| step["name"].as_str() == Some("Check out files in parallel"))
        .expect("checkout step");
    let gate = steps
        .iter()
        .position(|step| step["uses"].as_str() == Some(GATE_ACTION))
        .expect("gate step");
    assert!(checkout < gate, "{JOB} runs the gate before the checkout");
    let fetch = steps[checkout]["run"].as_str().unwrap_or_default();
    assert!(fetch.contains("--depth=2"), "{fetch}");
    let with = &steps[gate]["with"];
    assert_eq!(with["command"].as_str(), Some("${{ env.GATE }}"));
    assert_eq!(with["lcov"].as_str(), Some(LCOV));
}

#[test]
fn only_linux_x86_64_measures_coverage() {
    let job = job(WORKFLOW, JOB).unwrap();
    assert_eq!(job["env"]["GATE"].as_str(), Some(GATE));
}

#[test]
fn the_check_job_verifies_every_generated_file_on_one_platform() {
    let step = steps(WORKFLOW, JOB)
        .unwrap()
        .into_iter()
        .find(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains("/dx-bin/dx\" generate -- \"$@\""))
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
fn the_coverage_platform_uploads_the_lcov() {
    let steps = steps(WORKFLOW, JOB).unwrap();
    let scan = steps
        .iter()
        .find(|step| step["id"].as_str() == Some("scan-coverage"))
        .expect("coverage scan step");
    assert_eq!(
        scan["if"].as_str(),
        Some("${{ always() && contains(env.GATE, 'coverage') }}")
    );
    let upload = steps
        .iter()
        .find(|step| step["with"]["name"].as_str() == Some("coverage-${{ matrix.platform }}"))
        .expect("coverage upload step");
    assert_eq!(
        upload["if"].as_str(),
        Some("${{ always() && steps.scan-coverage.outcome == 'success' }}")
    );
    assert_eq!(
        upload["with"]["path"].as_str(),
        Some("${{ runner.temp }}/coverage")
    );
    assert_eq!(upload["with"]["retention-days"].as_u64(), Some(7));
}
