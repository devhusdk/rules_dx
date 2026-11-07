//! Keeps the remote cache key out of command lines, rc files and uploads.

use std::path::Path;
use std::process::Command;

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

fn gated(scan: &Value, upload: &Value) -> Result<(), String> {
    let id = scan["id"]
        .as_str()
        .ok_or("scan step has no id for the upload to check")?;
    if !scan["continue-on-error"].is_null() {
        return Err("scan step must not continue on error".into());
    }
    let expected = format!("${{{{ always() && steps.{id}.outcome == 'success' }}}}");
    if upload["if"] != expected.as_str() {
        return Err(format!(
            "upload must use if: {expected}, not {}",
            upload["if"]
        ));
    }
    Ok(())
}

#[test]
fn an_upload_runs_only_after_its_scan_passed() {
    let pair = |yaml: &str| -> Vec<Value> { yaml_serde::from_str(yaml).expect("valid YAML") };
    let shared = pair("- {name: scan, if: always()}\n- {uses: upload, if: always()}\n");
    assert!(gated(&shared[0], &shared[1]).is_err());
    let ignored = pair(
        "- {name: scan, id: scan-x, continue-on-error: true}\n\
         - {uses: upload, if: \"${{ always() && steps.scan-x.outcome == 'success' }}\"}\n",
    );
    assert!(gated(&ignored[0], &ignored[1]).is_err());
    let gated_pair = pair(
        "- {name: scan, id: scan-x, if: always()}\n\
         - {uses: upload, if: \"${{ always() && steps.scan-x.outcome == 'success' }}\"}\n",
    );
    assert_eq!(gated(&gated_pair[0], &gated_pair[1]), Ok(()));
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
                if let Err(error) = gated(scan, step) {
                    panic!("{at} {error}");
                }
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

fn run_script(dir: &Path, script: &str, env: &[(&str, &str)]) -> (i32, String) {
    let mut command = Command::new("bash");
    command.arg("-e").arg("-c").arg(script).current_dir(dir);
    command
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run bash");
    let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&done.stderr));
    (done.status.code().unwrap_or(-1), output)
}

fn assert_no_secret_printed(name: &str, output: &str) {
    for secret in ["k3y", "s3cr3t"] {
        assert!(!output.contains(secret), "{name} printed {secret}: {output}");
    }
}

#[test]
fn scan_finds_every_cache_secret() {
    let step = named_step(SCAN_ACTION, SCAN_STEP);
    let script = step["run"].as_str().expect("scan run");
    let key = ("BUILDBUDDY_API_KEY", "k3y");
    let header = ("REMOTE_CACHE_HEADER", "x-t=s3cr3t");
    let empty = ("REMOTE_CACHE_HEADER", "x-t=");
    let no_equals = ("REMOTE_CACHE_HEADER", "novalue");
    let no_value = Some("has no value");
    let cases: &[(&str, &[(&str, &str)], Option<&str>, i32, Option<&str>)] = &[
        ("nothing set", &[], Some("k3y s3cr3t"), 0, None),
        ("key found", &[key], Some("has k3y"), 1, None),
        ("key clean", &[key], Some("clean"), 0, None),
        ("header value found", &[header], Some("has s3cr3t"), 1, None),
        ("header name only", &[header], Some("x-t"), 0, None),
        ("header empty value", &[empty], Some("x"), 1, no_value),
        ("header without =", &[no_equals], Some("x"), 1, no_value),
        ("key with header", &[key, header], Some("has k3y"), 1, None),
        ("path missing", &[key, header], None, 0, None),
    ];
    for (name, env, content, status, expected) in cases {
        let dir = dx_test_scratch::scratch("secret-scan");
        let upload = dir.path().join("upload");
        if let Some(content) = content {
            std::fs::create_dir(&upload).expect("create upload dir");
            std::fs::write(upload.join("report.txt"), content).expect("write upload");
        }
        let upload = upload.to_str().expect("utf-8 path");
        let mut env = env.to_vec();
        env.push(("DX_UPLOAD_PATH", upload));
        let (code, output) = run_script(dir.path(), script, &env);
        assert_eq!(code, *status, "{name}: {output}");
        if let Some(expected) = expected {
            assert!(output.contains(expected), "{name}: {output}");
        }
        assert_no_secret_printed(name, &output);
    }
}

#[test]
fn scan_follows_symlinks() {
    let step = named_step(SCAN_ACTION, SCAN_STEP);
    let script = step["run"].as_str().expect("scan run");
    let cases = [
        ("link to a secret", "../outside.txt", None),
        ("dangling link", "../missing", Some("failed")),
    ];
    for (name, target, expected) in cases {
        let dir = dx_test_scratch::scratch("secret-scan-link");
        let upload = dir.path().join("upload");
        std::fs::create_dir(&upload).expect("create upload dir");
        std::fs::write(dir.path().join("outside.txt"), "has k3y").expect("write outside");
        std::os::unix::fs::symlink(target, upload.join("link")).expect("symlink");
        let env = [
            ("BUILDBUDDY_API_KEY", "k3y"),
            ("DX_UPLOAD_PATH", upload.to_str().expect("utf-8 path")),
        ];
        let (code, output) = run_script(dir.path(), script, &env);
        assert_eq!(code, 1, "{name}: {output}");
        if let Some(expected) = expected {
            assert!(output.contains(expected), "{name}: {output}");
        }
        assert_no_secret_printed(name, &output);
    }
}

#[test]
fn blanked_key_keeps_header_visible() {
    let mut matched = 0;
    for path in dx_testing::workflow_files() {
        let document = parse(&path);
        let key = document["env"]["BUILDBUDDY_API_KEY"].as_str().unwrap_or("");
        if !key.contains("secrets.REMOTE_CACHE_HEADER") {
            continue;
        }
        matched += 1;
        assert_eq!(
            document["env"]["REMOTE_CACHE_HEADER"], "${{ secrets.REMOTE_CACHE_HEADER }}",
            "{path} blanks the key but does not pass the header to the scan"
        );
    }
    assert!(matched > 0, "no workflow blanks the key for a header");
}
