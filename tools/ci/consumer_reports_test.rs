use serde_json::Value;

use ci_workflow::steps;
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const DOCS: &str = "docs/github-ci.md";

const UPLOAD_PIN: &str = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";

const SUFFIX: &str = "${{ needs.validate.outputs.artifact_suffix }}";

/// The checks that upload reports from the per-platform job.
const REPORTING: &[&str] = &["lint", "typecheck", "test", "build", "coverage"];

const JOB: &str = "checks";

/// Jobs that upload no reports. Flip these when the follow-ups land.
const UNREPORTED: &[&str] = &["security-audit"];

fn uploads(name: &str) -> Vec<(usize, Value)> {
    steps(WORKFLOW, name)
        .into_iter()
        .enumerate()
        .filter(|(_, step)| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/upload-artifact@"))
        })
        .collect()
}

fn artifact_name(job: &str) -> String {
    format!("dx-reports-{job}-${{{{ matrix.platform }}}}{SUFFIX}")
}

fn upload_of(id: &str) -> (usize, Value) {
    let name = artifact_name(id);
    let found: Vec<(usize, Value)> = uploads(JOB)
        .into_iter()
        .filter(|(_, step)| step["with"]["name"].as_str() == Some(name.as_str()))
        .collect();
    assert_eq!(found.len(), 1, "{id} needs exactly one upload step");
    found[0].clone()
}

#[test]
fn the_run_step_writes_every_report_under_its_check_directory() {
    let run = steps(WORKFLOW, JOB)
        .iter()
        .filter_map(|step| step["run"].as_str())
        .find(|run| run.contains("@rules_dx//:dx -- \"${args[@]}\""))
        .expect("run step")
        .to_owned();
    assert!(
        run.contains("reports=\"${RUNNER_TEMP:-/tmp}/dx-reports/$id\""),
        "{run}"
    );
    assert!(
        run.contains("--report \"${token%%=*}=$reports/${token#*=}\""),
        "{run}"
    );
    assert!(!run.contains("${{"), "no expressions in run blocks");
}

#[test]
fn each_reporting_check_uploads_once_after_dx_and_before_the_cache_steps() {
    let all = steps(WORKFLOW, JOB);
    let dx_at = all
        .iter()
        .position(|s| {
            s["run"]
                .as_str()
                .is_some_and(|r| r.contains("@rules_dx//:dx -- \"${args[@]}\""))
        })
        .expect("dx step");
    let first_cache = all
        .iter()
        .position(|s| {
            s["name"].as_str().is_some_and(|n| {
                n.starts_with("Prune the Bazel repository cache")
                    || n.starts_with("Save the Bazel repository cache")
            })
        })
        .unwrap_or(all.len());
    for id in REPORTING {
        let (at, step) = upload_of(id);
        assert_eq!(step["uses"].as_str(), Some(UPLOAD_PIN), "{id} pin");
        let guard = step["if"].as_str().unwrap_or("");
        assert!(guard.starts_with("${{ always() && "), "{id} if: {guard}");
        assert_eq!(
            step["with"]["path"].as_str(),
            Some(format!("${{{{ runner.temp }}}}/dx-reports/{id}").as_str())
        );
        assert_eq!(step["with"]["if-no-files-found"].as_str(), Some("warn"));
        assert_eq!(step["with"]["retention-days"].as_u64(), Some(14));
        assert!(
            step["with"]["overwrite"].is_null(),
            "{id} must not overwrite"
        );
        assert!(dx_at < at, "{id} uploads before dx runs");
        assert!(at < first_cache, "{id} uploads after the cache steps");
    }
}

#[test]
fn upload_names_are_unique_and_never_the_advisory_name() {
    let mut names = Vec::new();
    for job in REPORTING {
        let name = artifact_name(job);
        assert!(name.contains("matrix.platform") && name.ends_with(SUFFIX));
        assert!(!name.starts_with("advisory-snapshots"));
        assert!(!names.contains(&name), "{name} repeats");
        names.push(name);
    }
}

#[test]
fn unreported_jobs_upload_no_reports() {
    for job in UNREPORTED {
        for (_, step) in uploads(job) {
            let name = step["with"]["name"].as_str().unwrap_or("");
            assert!(!name.starts_with("dx-reports-"), "{job} uploads {name}");
        }
    }
}

#[test]
fn docs_name_every_artifact_and_the_download_command() {
    let docs = read_runfiles(DOCS);
    for job in REPORTING {
        assert!(
            docs.contains(&format!("dx-reports-{job}-<platform>")),
            "{DOCS} omits the {job} artifact"
        );
    }
    assert!(docs.contains("gh run download"));
}

#[test]
fn the_profile_report_is_kept_and_summarised_in_the_pr_comment() {
    let all = steps(WORKFLOW, JOB);
    let run_of = |needle: &str| -> String {
        all.iter()
            .filter_map(|step| step["run"].as_str())
            .find(|run| run.contains(needle))
            .unwrap_or_else(|| panic!("no step runs {needle}"))
            .to_owned()
    };
    let checks = run_of("@rules_dx//:dx -- \"${args[@]}\"");
    assert!(
        checks.contains("cp .dx/profile/report.json \"$reports/profile-$id.json\""),
        "{checks}"
    );
    let summary = run_of("dx-coverage-summary");
    assert!(summary.contains("% cache hits (remote "), "{summary}");
    assert!(summary.contains("critical path "), "{summary}");
    assert!(summary.contains("\"$profile_lines\""), "{summary}");
}
