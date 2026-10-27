use serde_json::Value;

use ci_workflow::steps;
use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const DOCS: &str = "docs/github-ci.md";

const UPLOAD_PIN: &str = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";

const SUFFIX: &str = "${{ needs.validate.outputs.artifact_suffix }}";

/// Each report-capable job and the report kinds its dx step requests.
const REPORTING: &[(&str, &[&str])] = &[
    ("lint", &["sarif"]),
    ("typecheck", &["sarif"]),
    ("test", &["junit", "failures", "results"]),
    ("build", &["failures"]),
    ("coverage", &["failures", "results"]),
];

/// Jobs that upload no reports. Flip these when the follow-ups land.
const UNREPORTED: &[&str] = &["format", "generate", "security-audit", "license-audit"];

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

#[test]
fn each_reporting_job_passes_exactly_its_report_kinds() {
    for (job, kinds) in REPORTING {
        let dx_steps: Vec<String> = steps(WORKFLOW, job)
            .iter()
            .filter_map(|step| step["run"].as_str())
            .filter(|run| run.contains("@rules_dx//:dx -- "))
            .filter(|run| run.contains("--report"))
            .map(str::to_owned)
            .collect();
        assert_eq!(dx_steps.len(), 1, "{job} needs one dx step with reports");
        let run = &dx_steps[0];
        assert!(run.contains(&format!("dx-reports/{job}\"")), "{job}: {run}");
        let mut found: Vec<String> = run
            .split("--report \"")
            .skip(1)
            .map(|rest| rest.split('=').next().unwrap_or("").to_owned())
            // The changed-line lcov report stays conditional on its threshold.
            .filter(|kind| kind != "lcov")
            .collect();
        found.sort();
        let mut expected: Vec<String> = kinds.iter().map(|kind| (*kind).to_owned()).collect();
        expected.sort();
        assert_eq!(found, expected, "{job} requests the wrong report kinds");
        assert!(!run.contains("${{"), "{job}: no expressions in run blocks");
    }
}

#[test]
fn each_reporting_job_uploads_once_after_dx_and_before_the_cache_steps() {
    for (job, _) in REPORTING {
        let found = uploads(job);
        assert_eq!(found.len(), 1, "{job} needs exactly one upload step");
        let (at, step) = &found[0];
        assert_eq!(step["uses"].as_str(), Some(UPLOAD_PIN), "{job} pin");
        assert_eq!(step["if"].as_str(), Some("${{ always() }}"), "{job} if");
        assert_eq!(
            step["with"]["name"].as_str(),
            Some(artifact_name(job).as_str())
        );
        assert_eq!(
            step["with"]["path"].as_str(),
            Some(format!("${{{{ runner.temp }}}}/dx-reports/{job}").as_str())
        );
        assert_eq!(step["with"]["if-no-files-found"].as_str(), Some("warn"));
        assert_eq!(step["with"]["retention-days"].as_u64(), Some(14));
        assert!(
            step["with"]["overwrite"].is_null(),
            "{job} must not overwrite"
        );
        let all = steps(WORKFLOW, job);
        let dx_at = all
            .iter()
            .position(|s| {
                s["run"]
                    .as_str()
                    .is_some_and(|r| r.contains("@rules_dx//:dx -- ") && r.contains("--report"))
            })
            .expect("dx step");
        assert!(dx_at < *at, "{job} uploads before dx runs");
        let first_cache = all
            .iter()
            .position(|s| {
                s["name"].as_str().is_some_and(|n| {
                    n.starts_with("Prune the Bazel repository cache")
                        || n.starts_with("Save the Bazel repository cache")
                })
            })
            .unwrap_or(all.len());
        assert!(*at < first_cache, "{job} uploads after the cache steps");
    }
}

#[test]
fn upload_names_are_unique_and_never_the_advisory_name() {
    let mut names = Vec::new();
    for (job, _) in REPORTING {
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
    for (job, _) in REPORTING {
        assert!(
            docs.contains(&format!("dx-reports-{job}-<platform>")),
            "{DOCS} omits the {job} artifact"
        );
    }
    assert!(docs.contains("gh run download"));
}
