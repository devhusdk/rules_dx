use std::fs;
use std::process::{Command, Stdio};

const MODULE: &str = "module(name = \"acme\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" build "*) printf '%s\n' "$*" >> "$ARGV_LOG" ;; esac
exit 0
"#;

/// Runs `dx build` against a fake Bazel; returns every recorded build argv line and stderr.
fn build_lines(args: &[&str], in_slot: bool) -> (Vec<String>, String) {
    build_lines_with_env(args, in_slot, &[])
}

/// Like `build_lines`; with `in_slot` the workspace is a free `dx worktree` pool slot.
fn build_lines_with_env(
    args: &[&str],
    in_slot: bool,
    env: &[(&str, &str)],
) -> (Vec<String>, String) {
    let scratch = dx_test_scratch::scratch("dx-build-slots-launcher-");
    let base = scratch.path().to_path_buf();
    let workspace = if in_slot {
        let meta = base.join("pool/.dx");
        fs::create_dir_all(&meta).expect("meta");
        let record = dx_worktree::Record {
            slot: 1,
            held: false,
            branch: None,
            last_branch: None,
            session: None,
            agent: None,
            pid: None,
            pid_start: None,
            boot_id: None,
            gone: false,
            stopped_at: None,
            unclaimed: false,
            broken: false,
            leased_at: 0,
            last_used: 0,
            lock_digest: None,
        };
        fs::write(
            meta.join("1.json"),
            serde_json::to_vec(&record).expect("json"),
        )
        .expect("record");
        base.join("pool/1")
    } else {
        base.join("ws")
    };
    let bin = base.join("bin");
    let tmp = base.join("tmp");
    for dir in [&workspace, &bin, &tmp, &workspace.join(".dx")] {
        fs::create_dir_all(dir).expect("dir");
    }
    fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
    fs::write(workspace.join(".dx/pins"), "0.0.0\n").expect("pin");
    fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
    let bazel = bin.join("bazel");
    dx_testing::executable_script(&bazel, FAKE_BAZEL);
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
        .envs(env.iter().copied())
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
fn a_plain_build_outside_a_slot_takes_no_slot() {
    let (lines, err) = build_lines(&["//..."], false);
    assert!(!lines.is_empty(), "bazel build never ran: {err}");
    assert!(
        !lines.iter().any(|line| line.contains("--local_resources")),
        "{lines:?}"
    );
    assert!(!err.contains("waiting for a build slot"), "{err}");
}

#[test]
fn a_plain_build_in_a_slot_gets_a_local_resources_share() {
    let (lines, err) = build_lines(&["//..."], true);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("--local_resources=cpu=")),
        "positive control: {lines:?}\n{err}"
    );
}

#[test]
fn the_env_can_turn_build_slots_on_outside_a_slot() {
    let (lines, err) = build_lines_with_env(&["//..."], false, &[("DX_BUILD_SLOTS", "auto")]);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("--local_resources=cpu=")),
        "{lines:?}\n{err}"
    );
}

#[test]
fn build_slots_off_adds_no_local_resources_and_never_waits() {
    let (lines, err) = build_lines(&["--build-slots", "off", "//..."], false);
    assert!(!lines.is_empty(), "bazel build never ran: {err}");
    assert!(
        !lines.iter().any(|line| line.contains("--local_resources")),
        "{lines:?}"
    );
    assert!(!err.contains("waiting for a build slot"), "{err}");
}

#[test]
fn a_count_keeps_a_cpu_resource_passed_after_the_separator() {
    let (lines, err) = build_lines(
        &[
            "--build-slots",
            "2",
            "//...",
            "--",
            "--local_resources=cpu=4",
        ],
        false,
    );
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
