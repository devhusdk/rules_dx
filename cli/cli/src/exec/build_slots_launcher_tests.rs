use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

const MODULE: &str = "module(name = \"acme\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" build "*) printf '%s\n' "$*" >> "$ARGV_LOG" ;; esac
exit 0
"#;

/// Runs `dx build` against a fake Bazel; returns every recorded build argv line and stderr.
fn build_lines(args: &[&str]) -> (Vec<String>, String) {
    let scratch = dx_test_scratch::scratch("dx-build-slots-launcher-");
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
    let bazel = bin.join("bazel");
    fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
    fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
    let log = base.join("argv.log");
    let output = Command::new(dx_testing::runfiles_root().join("cli/cli/dx"))
        .arg("build")
        .args(args)
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", &base)
        .env("XDG_CACHE_HOME", base.join("cache"))
        .env("TMPDIR", &tmp)
        .env("ARGV_LOG", &log)
        .stdin(Stdio::null())
        .output()
        .expect("dx runs");
    let lines = fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    (lines, String::from_utf8_lossy(&output.stderr).into_owned())
}

#[test]
fn a_plain_build_gets_a_local_resources_share() {
    let (lines, err) = build_lines(&["//..."]);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("--local_resources=cpu=")),
        "positive control: {lines:?}\n{err}"
    );
}

#[test]
fn build_slots_off_adds_no_local_resources_and_never_waits() {
    let (lines, err) = build_lines(&["--build-slots", "off", "//..."]);
    assert!(!lines.is_empty(), "bazel build never ran: {err}");
    assert!(
        !lines.iter().any(|line| line.contains("--local_resources")),
        "{lines:?}"
    );
    assert!(!err.contains("waiting for a build slot"), "{err}");
}

#[test]
fn a_count_keeps_a_cpu_resource_passed_after_the_separator() {
    let (lines, err) = build_lines(&[
        "--build-slots",
        "2",
        "//...",
        "--",
        "--local_resources=cpu=4",
    ]);
    let builds: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("--local_resources=cpu=4"))
        .collect();
    assert!(!builds.is_empty(), "{lines:?}\n{err}");
    for line in builds {
        assert_eq!(line.matches("--local_resources=cpu=").count(), 1, "{line}");
        assert!(line.contains("--local_resources=memory="), "{line}");
    }
}
