use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const MODULE: &str = "module(name = \"acme_renamed\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" build "*|*" test "*) ;; *) exit 0 ;; esac
for arg in "$@"; do
  case "$arg" in --build_event_json_file=*) bep="${arg#--build_event_json_file=}" ;; esac
done
printf '%s\n' "$FAKE_BEP_LINE" > "$bep"
exit 1
"#;

const DIAGNOSTIC: &str = r#"{"$message_type":"diagnostic","message":"cannot find value `y` in this scope","code":{"code":"E0425","explanation":null},"level":"error","spans":[{"file_name":"ünï/src/main.rs","byte_start":30,"byte_end":31,"line_start":3,"line_end":3,"column_start":5,"column_end":6,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}"#;

fn run(verb: &str) -> (Option<i32>, String, String) {
    let scratch = dx_test_scratch::scratch("dx-failure-launcher-");
    let base = scratch.path().to_path_buf();
    let workspace = base.join("ws");
    let bin = base.join("bin");
    let tmp = base.join("tmp");
    for dir in [&workspace, &bin, &tmp, &workspace.join(".dx")] {
        fs::create_dir_all(dir).expect("dir");
    }
    fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
    let stderr_file = base.join("action-stderr");
    fs::write(&stderr_file, format!("{DIAGNOSTIC}\n")).expect("stderr");
    let bep = serde_json::json!({
        "id": {"actionCompleted": {
            "primaryOutput": "bazel-out/k8-fastbuild/bin/ünï/bin",
            "label": "//ünï:bin",
            "configuration": {"id": "cfg1"},
        }},
        "action": {
            "success": false,
            "type": "Rustc",
            "exitCode": 1,
            "stderr": {"uri": format!("file://{}", stderr_file.display())},
            "label": "//ünï:bin",
            "configuration": {"id": "cfg1"},
        },
    })
    .to_string();
    let bazel = bin.join("bazel");
    fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
    fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
    let output = Command::new(dx_testing::runfiles_root().join("cli/cli/dx"))
        .args([verb, "--output=json", "//..."])
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", &base)
        .env("TMPDIR", &tmp)
        .env("FAKE_BEP_LINE", bep)
        .stdin(Stdio::null())
        .output()
        .expect("dx runs");
    let _: PathBuf = base;
    (
        output.status.code(),
        String::from_utf8(output.stdout).expect("stdout"),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_launcher_reports_a_failed_rust_action_in_a_renamed_consumer() {
    for verb in ["build", "test"] {
        let (code, out, err) = run(verb);
        assert_eq!(code, Some(1), "{verb}\n{out}\n{err}");
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("NDJSON"))
            .collect();
        let failures: Vec<_> = events
            .iter()
            .filter(|event| event["event"] == "build_failure")
            .collect();
        assert_eq!(failures.len(), 1, "{verb}\n{out}\n{err}");
        assert_eq!(failures[0]["target"], "//ünï:bin");
        assert_eq!(failures[0]["stage"], "compilation");
        assert_eq!(failures[0]["diagnostics"]["items"][0]["code"], "E0425");
        assert_eq!(
            events.last().expect("events")["event"],
            "command_finished",
            "{out}"
        );
        assert!(
            !events.iter().any(|event| event["event"] == "test_outcome"),
            "{out}"
        );
    }
}
