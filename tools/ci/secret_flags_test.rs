//! Keeps the remote cache key out of command lines, rc files and uploads.

use serde_json::Value;

use dx_testing::read_runfiles;

const SETUP_ACTION: &str = ".github/actions/setup-bazel/action.yml";
const SCAN_ACTION: &str = ".github/actions/assert-no-secret/action.yml";
const LOCAL_HELPER: &str = "tools/buildbuddy-credential-helper.sh";
const HELPER_STEP: &str = "Configure the remote cache credential helper";
const SCAN_STEP: &str = "Check the upload holds no secret";
const RC_FILES: &[&str] = &[
    ".bazelrc",
    "tools/bazelrc/correctness.bazelrc",
    "tools/bazelrc/performance.bazelrc",
    "tools/bazelrc/resources.bazelrc",
];

fn action_files() -> Vec<String> {
    let list = std::env::var("DX_ACTIONS").expect("DX_ACTIONS is unset");
    list.split_whitespace().map(str::to_owned).collect()
}

fn ci_files() -> Vec<String> {
    let mut files = dx_testing::workflow_files();
    files.extend(action_files());
    files
}

fn parse(path: &str) -> Value {
    yaml_serde::from_str(&read_runfiles(path))
        .unwrap_or_else(|error| panic!("{path} is not valid YAML: {error}"))
}

fn named_step(path: &str, name: &str) -> Value {
    parse(path)["runs"]["steps"]
        .as_array()
        .and_then(|steps| steps.iter().find(|step| step["name"] == name))
        .cloned()
        .unwrap_or_else(|| panic!("{path} has no {name:?} step"))
}

fn jobs(document: &Value) -> Vec<(String, Vec<Value>)> {
    let Some(jobs) = document["jobs"].as_object() else {
        return Vec::new();
    };
    jobs.iter()
        .map(|(id, job)| {
            let steps = job["steps"].as_array().cloned().unwrap_or_default();
            (id.clone(), steps)
        })
        .collect()
}

fn is_upload(step: &Value) -> bool {
    step["uses"].as_str().is_some_and(|uses| {
        uses.starts_with("actions/upload-artifact@")
            || uses.starts_with("actions/upload-pages-artifact@")
    })
}

#[test]
fn no_remote_header_carries_a_secret() {
    for path in ci_files() {
        let text = read_runfiles(&path);
        for (index, line) in text.lines().enumerate() {
            let at = format!("{path}:{}", index + 1);
            if line.contains("remote_header") {
                assert!(
                    !line.contains("secrets.") && !line.contains("BUILDBUDDY_API_KEY"),
                    "{at} passes a secret through --remote_header"
                );
            }
            assert!(
                !line.contains("x-buildbuddy-api-key=") && !line.contains("announce_rc"),
                "{at} can print the cache key"
            );
            assert!(
                !line.contains("--remote_header="),
                "{at} sets a --remote_header; use the credential helper"
            );
        }
    }
}

#[test]
fn no_rc_file_announces_its_lines() {
    for path in RC_FILES {
        let text = read_runfiles(path);
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            assert!(
                line.starts_with('#') || !line.contains("announce_rc"),
                "{path}:{} sets --announce_rc, which prints the rc lines into logs",
                index + 1
            );
        }
    }
}

#[test]
fn no_workflow_sets_bb_args_in_env() {
    for path in dx_testing::workflow_files() {
        let document = parse(&path);
        assert!(
            document["env"]["BB_ARGS"].is_null(),
            "{path} sets BB_ARGS; the credential helper step writes it"
        );
        for (id, _) in jobs(&document) {
            assert!(
                document["jobs"][&id]["env"]["BB_ARGS"].is_null(),
                "{path} job {id} sets BB_ARGS; the credential helper step writes it"
            );
        }
    }
}

#[test]
fn inline_helper_and_scan_steps_match_the_actions() {
    let helper = named_step(SETUP_ACTION, HELPER_STEP);
    let scan = named_step(SCAN_ACTION, SCAN_STEP);
    let mut helpers = 0;
    for path in dx_testing::workflow_files() {
        for (id, steps) in jobs(&parse(&path)) {
            for step in steps {
                if step["name"] == HELPER_STEP {
                    helpers += 1;
                    assert_eq!(
                        step, helper,
                        "{path} job {id} helper differs from {SETUP_ACTION}"
                    );
                }
                if step["name"] == SCAN_STEP && step["uses"].is_null() {
                    assert_eq!(step["run"], scan["run"], "{path} job {id} scan differs");
                    assert_eq!(step["shell"], "bash", "{path} job {id} scan shell");
                    assert!(step["env"]["DX_UPLOAD_PATH"].is_string(), "{path} job {id}");
                }
            }
        }
    }
    assert!(helpers > 0, "no workflow copies the helper step");
}

#[test]
fn ci_writes_the_same_helper_local_builds_use() {
    let step = named_step(SETUP_ACTION, HELPER_STEP);
    let run = step["run"].as_str().expect("helper run");
    let (_, rest) = run.split_once("<<'EOF'\n").expect("helper heredoc");
    let (body, _) = rest.split_once("\nEOF\n").expect("helper heredoc end");
    assert_eq!(format!("{body}\n"), read_runfiles(LOCAL_HELPER));
    assert!(
        read_runfiles(".bazelrc").contains(&format!(
            "common:bb --credential_helper=remote.buildbuddy.io=%workspace%/{LOCAL_HELPER}\n"
        )),
        ".bazelrc --config=bb must use {LOCAL_HELPER}"
    );
}

#[test]
fn every_job_using_bb_args_configures_the_helper_first() {
    for path in dx_testing::workflow_files() {
        for (id, steps) in jobs(&parse(&path)) {
            let Some(first) = steps.iter().position(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run.contains("$BB_ARGS") || run.contains("${BB_ARGS"))
            }) else {
                continue;
            };
            let configured = steps[..first].iter().any(|step| {
                step["name"] == HELPER_STEP || step["uses"] == "./.github/actions/setup-bazel"
            });
            assert!(
                configured,
                "{path} job {id} uses $BB_ARGS before the credential helper step"
            );
        }
    }
}

#[test]
fn every_upload_follows_a_matching_scan() {
    let mut uploads = 0;
    for path in dx_testing::workflow_files() {
        let document = parse(&path);
        if document["env"]["BUILDBUDDY_API_KEY"].is_null() {
            continue;
        }
        for (id, steps) in jobs(&document) {
            for (index, step) in steps.iter().enumerate() {
                if !is_upload(step) {
                    continue;
                }
                uploads += 1;
                let at = format!("{path} job {id} step {index}");
                let scan = index
                    .checked_sub(1)
                    .map(|before| &steps[before])
                    .filter(|before| before["name"] == SCAN_STEP)
                    .unwrap_or_else(|| panic!("{at} uploads without a {SCAN_STEP:?} step first"));
                assert_eq!(
                    scan["if"], step["if"],
                    "{at} scan and upload conditions differ"
                );
                let upload_path = step["with"]["path"].as_str().expect("upload path");
                let scanned = scan["with"]["path"]
                    .as_str()
                    .or_else(|| scan["env"]["DX_UPLOAD_PATH"].as_str())
                    .expect("scan path");
                let expected = match upload_path.find('*') {
                    Some(star) => upload_path[..star].trim_end_matches('/'),
                    None => upload_path,
                };
                assert_eq!(scanned, expected, "{at} scans another path than it uploads");
                if scan["uses"].is_string() {
                    assert_eq!(scan["uses"], "./.github/actions/assert-no-secret", "{at}");
                }
            }
        }
    }
    assert!(uploads > 0, "no workflow with the key uploads anything");
}
