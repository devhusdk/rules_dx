//! Keeps the remote cache key out of command lines, rc files and uploads.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use dx_testing::read_runfiles;

const SETUP_ACTION: &str = ".github/actions/remote-cache-helper/action.yml";
const SCAN_ACTION: &str = ".github/actions/assert-no-secret/action.yml";
const LOCAL_HELPER: &str = "tools/buildbuddy-credential-helper.sh";
const HELPER_STEP: &str = "Configure the remote cache credential helper";
const REMOTE_SCAN: &str = "devhusdk/rules_dx/.github/actions/assert-no-secret";
const REMOTE_HELPER: &str = "devhusdk/rules_dx/.github/actions/remote-cache-helper";
const SCAN_STEP: &str = "Check the upload holds no secret";

fn rc_files() -> Vec<(String, String)> {
    let mut files = vec![(".bazelrc".to_owned(), read_runfiles(".bazelrc"))];
    for layer in dx_preset::Layer::ALL {
        files.push((
            format!(".dx/bazelrc/{}", layer.file_name()),
            dx_preset::render_layer(layer, dx_preset::DEFAULT_REPO),
        ));
    }
    files
}

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
    ci_workflow::document(path)
}

fn named_step(path: &str, name: &str) -> Value {
    parse(path)["runs"]["steps"]
        .as_array()
        .and_then(|steps| steps.iter().find(|step| step["name"] == name))
        .cloned()
        .unwrap_or_else(|| panic!("{path} has no {name:?} step"))
}

fn jobs(document: &Value) -> Vec<(String, Vec<Value>)> {
    ci_workflow::jobs_of(document)
        .into_iter()
        .map(|(id, job)| (id.to_owned(), ci_workflow::steps_of(job).to_vec()))
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
        for line in ci_workflow::strings(&path).iter().flat_map(|text| text.lines()) {
            let at = path.as_str();
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
    for (path, text) in rc_files() {
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
fn workflow_helper_steps_use_the_pinned_action() {
    let remote = format!("{REMOTE_HELPER}@");
    let mut helpers = 0;
    for path in dx_testing::workflow_files() {
        for (id, steps) in jobs(&parse(&path)) {
            for step in steps {
                if step["name"] == HELPER_STEP {
                    helpers += 1;
                    assert!(
                        step["uses"].as_str().is_some_and(|uses| uses.starts_with(&remote)),
                        "{path} job {id} helper must use {remote}<sha>"
                    );
                    assert!(step["run"].is_null(), "{path} job {id} helper runs a script");
                }
            }
        }
    }
    assert!(helpers > 0, "no workflow configures the helper");
}

#[test]
fn scan_action_runs_the_script_on_node() {
    let runs = &parse(SCAN_ACTION)["runs"];
    assert_eq!(runs["using"], "node24");
    assert_eq!(runs["main"], "scan.cjs");
}

#[test]
fn helper_action_runs_the_script_on_node() {
    let runs = &parse(SETUP_ACTION)["runs"];
    assert_eq!(runs["using"], "node24");
    assert_eq!(runs["main"], "helper.cjs");
}

#[test]
fn ci_writes_the_same_helper_local_builds_use() {
    let dir = dx_test_scratch::scratch("helper-action-content");
    let (code, output, written) = run_helper_node(dir.path(), &[("BUILDBUDDY_API_KEY", "k3y")]);
    assert_eq!(code, 0, "{output}");
    let helper = helper_path(&written);
    assert_eq!(
        std::fs::read_to_string(&helper).expect("read helper"),
        read_runfiles(LOCAL_HELPER)
    );
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
        let workflow_key = !document["env"]["BUILDBUDDY_API_KEY"].is_null();
        for (id, steps) in jobs(&document) {
            if !workflow_key && document["jobs"][&id]["env"]["BUILDBUDDY_API_KEY"].is_null() {
                continue;
            }
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
                    let uses = scan["uses"].as_str().expect("scan uses");
                    let remote = format!("{REMOTE_SCAN}@");
                    assert!(
                        uses == "./.github/actions/assert-no-secret" || uses.starts_with(&remote),
                        "{at} scan uses {uses}"
                    );
                    for earlier in &steps[..index] {
                        let sparse = &earlier["with"]["sparse-checkout"];
                        if earlier["uses"]
                            .as_str()
                            .is_some_and(|uses| uses.starts_with("actions/checkout@"))
                            && !sparse.is_null()
                        {
                            let listed = sparse.as_str().expect("sparse-checkout text");
                            assert!(
                                listed
                                    .lines()
                                    .any(|line| line.trim() == ".github/actions/assert-no-secret"),
                                "{at} sparse checkout omits the scan action"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(uploads > 0, "no workflow with the key uploads anything");
}

fn keep_system_vars(command: &mut Command) {
    for (key, value) in std::env::vars_os() {
        if key.to_string_lossy().to_ascii_lowercase().starts_with("system") {
            command.env(key, value);
        }
    }
}

fn run_node(dir: &Path, env: &[(&str, &str)]) -> (i32, String) {
    let mut command = Command::new(dx_testing::env_file("DX_NODE"));
    command
        .arg(dx_testing::env_file("DX_SCAN_CJS"))
        .current_dir(dir)
        .env_clear();
    keep_system_vars(&mut command);
    for (key, value) in env {
        let key = if *key == "DX_UPLOAD_PATH" { "INPUT_PATH" } else { key };
        command.env(key, value);
    }
    let done = command.output().expect("run node");
    let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&done.stderr));
    (done.status.code().unwrap_or(-1), output)
}

fn run_scans(dir: &Path, env: &[(&str, &str)]) -> Vec<(&'static str, i32, String)> {
    let (code, output) = run_node(dir, env);
    vec![("node", code, output)]
}

fn remote_action_shas(action: &str) -> std::collections::BTreeSet<String> {
    let remote = format!("{action}@");
    let mut shas = std::collections::BTreeSet::new();
    for path in dx_testing::workflow_files() {
        for (_, steps) in jobs(&parse(&path)) {
            for step in steps {
                if let Some(sha) = step["uses"].as_str().and_then(|u| u.strip_prefix(&remote)) {
                    assert!(
                        sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
                        "{path} pins {sha}"
                    );
                    shas.insert(sha.to_owned());
                }
            }
        }
    }
    shas
}

#[test]
fn remote_scan_references_name_one_sha() {
    let shas = remote_action_shas(REMOTE_SCAN);
    assert_eq!(shas.len(), 1, "scan action refs name {shas:?}");
}

#[test]
fn remote_helper_references_name_one_sha() {
    let shas = remote_action_shas(REMOTE_HELPER);
    assert_eq!(shas.len(), 1, "helper action refs name {shas:?}");
}

fn assert_no_secret_printed(name: &str, output: &str) {
    for secret in ["k3y", "s3cr3t"] {
        assert!(
            !output.contains(secret),
            "{name} printed {secret}: {output}"
        );
    }
}

#[test]
fn scan_finds_every_cache_secret() {
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
        for (runner, code, output) in run_scans(dir.path(), &env) {
            assert_eq!(code, *status, "{runner} {name}: {output}");
            if let Some(expected) = expected {
                assert!(output.contains(expected), "{runner} {name}: {output}");
            }
            assert_no_secret_printed(name, &output);
        }
    }
}

#[cfg(unix)]
#[test]
fn scan_follows_symlinks() {
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
        for (runner, code, output) in run_scans(dir.path(), &env) {
            assert_eq!(code, 1, "{runner} {name}: {output}");
            if let Some(expected) = expected {
                assert!(output.contains(expected), "{runner} {name}: {output}");
            }
            assert_no_secret_printed(name, &output);
        }
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

fn helper_path(written: &str) -> String {
    written
        .trim()
        .rsplit('=')
        .next()
        .expect("helper path")
        .to_owned()
}

fn run_helper_node(dir: &Path, env: &[(&str, &str)]) -> (i32, String, String) {
    let temp = dir.join("temp");
    std::fs::create_dir_all(&temp).expect("create temp");
    let github_env = dir.join("github_env");
    let mut command = Command::new(dx_testing::env_file("DX_NODE"));
    command
        .arg(dx_testing::env_file("DX_HELPER_JS"))
        .current_dir(dir)
        .env_clear()
        .env("RUNNER_OS", "Linux")
        .env("RUNNER_TEMP", &temp)
        .env("GITHUB_ENV", &github_env);
    keep_system_vars(&mut command);
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run node");
    let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&done.stderr));
    let written = std::fs::read_to_string(&github_env).unwrap_or_default();
    (done.status.code().unwrap_or(-1), output, written)
}

#[test]
fn helper_action_writes_a_working_helper() {
    let key = ("BUILDBUDDY_API_KEY", "k3y");
    let cache = (
        "DX_REMOTE_CACHE_ARG",
        "--remote_cache=grpcs://remote.example.com",
    );
    let header = ("REMOTE_CACHE_HEADER", "x-t=s3cr3t");
    let own_host = (
        "DX_REMOTE_CACHE_ARG",
        "--remote_cache=grpcs://u@cache.example.com:443",
    );
    let cases: &[(&str, &[(&str, &str)], i32, Option<&str>)] = &[
        ("key only", &[key], 0, Some("remote.buildbuddy.io=")),
        (
            "key with a custom cache",
            &[key, cache],
            0,
            Some(
                "--remote_cache=grpcs://remote.example.com --credential_helper=remote.buildbuddy.io=",
            ),
        ),
        (
            "header",
            &[header, own_host],
            0,
            Some("--credential_helper=cache.example.com="),
        ),
        (
            "header wins over key",
            &[key, header, own_host],
            0,
            Some("--credential_helper=cache.example.com="),
        ),
        ("no equals", &[("REMOTE_CACHE_HEADER", "novalue")], 1, None),
        ("quote", &[("REMOTE_CACHE_HEADER", "x=a\"b")], 1, None),
        ("percent", &[("REMOTE_CACHE_HEADER", "x=a%b")], 1, None),
        ("nothing set", &[], 0, None),
    ];
    for (name, env, status, args) in cases {
        let dir = dx_test_scratch::scratch("helper-action-node");
        let (code, output, written) = run_helper_node(dir.path(), env);
        assert_eq!(code, *status, "{name}: {output}");
        assert_no_secret_printed(name, &output);
        if *status != 0 {
            assert!(output.contains("::error::"), "{name}: {output}");
        }
        let Some(args) = args else {
            assert!(written.is_empty(), "{name} wrote {written:?}");
            continue;
        };
        assert!(
            written.starts_with("BB_ARGS=") && written.contains(args),
            "{name}: {written}"
        );
        let helper = std::fs::read_to_string(helper_path(&written)).expect("read helper");
        assert_eq!(helper, read_runfiles(LOCAL_HELPER), "{name}");
    }
}

#[test]
fn hermetic_verify_runs_only_while_the_job_is_succeeding() {
    let action = ".github/actions/hermetic-evidence/action.yml";
    let verify = named_step(action, "Verify the evidence");
    let condition = verify["if"].as_str().expect("Verify has an if");
    assert_eq!(
        condition.replace(' ', ""),
        "${{job.status=='success'}}",
        "Verify must be skipped once an earlier step of the job failed"
    );
    for step in parse(action)["runs"]["steps"].as_array().unwrap() {
        let run = step["run"].as_str().unwrap_or_default();
        assert!(!run.contains("|| true"), "{action} swallows a failure: {run}");
    }
    let compiler = parse(".github/actions/compiler-evidence/action.yml");
    for step in compiler["runs"]["steps"].as_array().unwrap() {
        let run = step["run"].as_str().unwrap_or_default();
        assert!(!run.contains("|| true"), "compiler-evidence swallows a failure: {run}");
    }
}
