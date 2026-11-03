use dx_testing::{read_runfiles, runfiles_root};
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_DIR: &str = "tools/ci/module_pin_fixtures";

struct Expect {
    pin: String,
    status: Option<i32>,
    says: Vec<String>,
    absent: Vec<String>,
}

fn expectation(case: &str) -> Expect {
    let text = read_runfiles(&format!("{FIXTURE_DIR}/{case}/expect.txt"));
    let mut expect = Expect {
        pin: String::new(),
        status: None,
        says: Vec::new(),
        absent: Vec::new(),
    };
    for line in text.lines() {
        if let Some(pin) = line.strip_prefix("pin ") {
            expect.pin = pin.to_string();
        } else if let Some(status) = line.strip_prefix("exit ") {
            expect.status = status.parse().ok();
        } else if let Some(says) = line.strip_prefix("says ") {
            expect.says.push(says.to_string());
        } else if let Some(absent) = line.strip_prefix("absent ") {
            expect.absent.push(absent.to_string());
        }
    }
    expect
}

fn cases() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(runfiles_root().join(FIXTURE_DIR))
        .expect("fixture directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn copy_case(source: &Path, target: &Path) {
    for entry in std::fs::read_dir(source).expect("fixture files").flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            let inner = target.join(&name);
            std::fs::create_dir_all(&inner).expect("fixture directory");
            copy_case(&path, &inner);
        } else if name != "expect.txt" {
            let name = name.strip_suffix(".fixture").unwrap_or(&name);
            std::fs::copy(&path, target.join(name)).expect("fixture copy");
        }
    }
}

fn scratch_case(scratch: &Path, case: &str) -> PathBuf {
    let home = scratch.join(case);
    std::fs::create_dir_all(&home).expect("case directory");
    copy_case(&runfiles_root().join(FIXTURE_DIR).join(case), &home);
    home
}

fn validator() -> PathBuf {
    runfiles_root().join(std::env::var("DX_MODULE_PIN").expect("DX_MODULE_PIN"))
}

fn run(home: &Path, pin: &str) -> (i32, String) {
    let done = Command::new(validator())
        .arg("MODULE.bazel")
        .current_dir(home)
        .env("RULES_DX_PIN", pin)
        .output()
        .expect("run module_pin_bin");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    );
    (done.status.code().unwrap_or(-1), text)
}

#[test]
fn every_fixture_states_one_expected_outcome() {
    let names = cases();
    assert!(!names.is_empty(), "{FIXTURE_DIR} holds no fixture");
    for case in names {
        let expect = expectation(&case);
        assert!(
            matches!(expect.status, Some(0 | 1)),
            "{case} declares no exit status"
        );
        assert!(!expect.says.is_empty(), "{case} expects no validator text");
    }
}

#[test]
fn fixtures_report_the_expected_diagnostic() {
    let scratch = dx_test_scratch::scratch("module-pin");
    for case in cases() {
        let expect = expectation(&case);
        let home = scratch_case(scratch.path(), &case);
        let (code, output) = run(&home, &expect.pin);
        assert_eq!(Some(code), expect.status, "{case} exited {code}: {output}");
        for text in &expect.says {
            assert!(
                output.contains(text.as_str()),
                "{case} never reported [{text}]: {output}"
            );
        }
        for name in &expect.absent {
            assert!(!home.join(name).exists(), "{case} created {name}");
        }
    }
}

fn repository_pin() -> String {
    read_runfiles(".github/workflows/ci.yml")
        .lines()
        .find_map(|line| line.trim().strip_prefix("rules_dx_version:"))
        .map(|value| value.trim().trim_matches('"').to_string())
        .expect("ci.yml passes no rules_dx_version")
}

#[test]
fn the_repository_module_declares_the_version_its_workflow_passes() {
    let scratch = dx_test_scratch::scratch("module-pin-repo");
    let pin = repository_pin();
    dx_test_scratch::write(
        scratch.path(),
        "MODULE.bazel",
        &read_runfiles("MODULE.bazel"),
    );
    let (code, output) = run(scratch.path(), &pin);
    assert_eq!(
        code, 0,
        "MODULE.bazel must declare rules_dx at {pin}: {output}"
    );
    let declared =
        format!("module(name = \"rules_dx\", version = \"{pin}\") matches the workflow pin");
    assert!(output.contains(&declared), "{output}");
    let (other, message) = run(scratch.path(), "0.0.0-wrong");
    assert_eq!(other, 1, "{message}");
    assert!(
        message.contains("the workflow expects \"0.0.0-wrong\""),
        "{message}"
    );
}

#[test]
fn bazel_run_reads_the_module_in_the_calling_directory() {
    let scratch = dx_test_scratch::scratch("module-pin-cwd");
    let case = "pass_external_bazel_dep";
    let home = scratch_case(scratch.path(), case);
    let elsewhere = scratch.path().join("runfiles");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");
    let done = Command::new(validator())
        .current_dir(&elsewhere)
        .env("RULES_DX_PIN", expectation(case).pin)
        .env("BUILD_WORKING_DIRECTORY", &home)
        .output()
        .expect("run module_pin_bin");
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
}

const BAD_LABELS: [&str; 10] = [
    "@//:x.MODULE.bazel",
    "@other//:x.MODULE.bazel",
    "//../a:x.MODULE.bazel",
    "//:../x.MODULE.bazel",
    "//a\\\\b:x.MODULE.bazel",
    "//a//b:x.MODULE.bazel",
    "//pkg",
    ":x.MODULE.bazel",
    "//:x.bzl",
    "//:.h.MODULE.bazel",
];

#[test]
fn bad_include_labels_are_rejected_and_never_read() {
    let scratch = dx_test_scratch::scratch("module-pin-labels");
    dx_test_scratch::write(
        scratch.path(),
        "x.MODULE.bazel",
        "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n",
    );
    for (index, label) in BAD_LABELS.iter().enumerate() {
        let home = scratch.path().join(format!("home{index}"));
        dx_test_scratch::write(&home, "MODULE.bazel", &format!("include(\"{label}\")\n"));
        let (code, output) = run(&home, "1.2.3");
        assert_eq!(code, 1, "{label} was accepted: {output}");
        assert!(!output.contains("matches"), "{label} was read: {output}");
        assert!(
            output.contains(&format!("include {label}")),
            "{label}: {output}"
        );
    }
}
