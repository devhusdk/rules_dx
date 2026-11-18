use evidence_inputs::{PLATFORMS, validate};

#[path = "evidence_rules/status.rs"]
mod status;

const WORKFLOW: &str = ".github/workflows/evidence.yml";

fn lookup<'a>(inputs: &[(&str, &'a str)], name: &str) -> &'a str {
    inputs
        .iter()
        .find(|(key, _)| *key == name)
        .map_or("", |(_, value)| value)
}

fn check(inputs: &[(&str, &str)]) -> Result<Vec<(String, String)>, String> {
    validate(|name| lookup(inputs, name).to_string()).map(|done| done.outputs)
}

fn run(scope: &str, platforms: &str, timeout: &str) -> Result<Vec<(String, String)>, String> {
    check(&[
        ("DX_SCOPE", scope),
        ("DX_TASK", "test"),
        ("DX_PLATFORMS", platforms),
        ("DX_TEST_TIMEOUT", timeout),
    ])
}

const OK: &str = r#"["linux_x86_64"]"#;

#[test]
fn accepts_whitespace_separated_patterns() {
    let cases = [
        ("//cli/cli/...", "//cli/cli/..."),
        (
            "//cli/process/... //tools/testing/process_probe/...",
            "//cli/process/... //tools/testing/process_probe/...",
        ),
        ("//a/...\t//b:t\n//c:*", "//a/... //b:t //c:*"),
        (
            "//apple/private/ios/... //a:t",
            "//apple/private/ios/... //a:t",
        ),
    ];
    for (scope, expected) in cases {
        let outputs = run(scope, OK, "600").unwrap_or_else(|e| panic!("{scope:?}: {e}"));
        let keys: Vec<&str> = outputs.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["scope", "task", "test_timeout", "flags", "relocate"]);
        assert_eq!(outputs[0].1, expected);
    }
}

#[test]
fn rejects_bad_inputs_naming_the_value() {
    let mut cases: Vec<(&str, &str, &str, &str)> = vec![
        ("", OK, "600", "at least one target pattern"),
        (" \t\n", OK, "600", "at least one target pattern"),
        ("//a/... b/...", OK, "600", "'b/...'"),
        ("cli/...", OK, "600", "'cli/...'"),
        ("//$(id)", OK, "600", "'//$(id)'"),
        ("//a`id`", OK, "600", "'//a`id`'"),
        ("//a;id", OK, "600", "'//a;id'"),
        ("@other//a", OK, "600", "'@other//a'"),
        ("//a|b", OK, "600", "'//a|b'"),
        ("//\"a\"", OK, "600", "'//\"a\"'"),
        ("-//a", OK, "600", "'-//a'"),
        ("//a/... //$(id)", OK, "600", "'//$(id)'"),
        ("//a:", OK, "600", "'//a:'"),
        ("//a:b:c", OK, "600", "'//a:b:c'"),
    ];
    for (platforms, says) in [
        (r#"["linux_x86_64","linux_x86_64"]"#, "twice"),
        ("[1]", "JSON number"),
        (r#"[["linux_x86_64"]]"#, "JSON array"),
        ("{}", "JSON object"),
        (r#""x""#, "JSON string"),
        ("[]", "non-empty"),
        (r#"["plan9_x86_64"]"#, "unknown platforms ['plan9_x86_64']"),
        ("[linux", "must be a JSON array"),
        ("", "must be a JSON array"),
    ] {
        cases.push(("//a/...", platforms, "600", says));
    }
    for timeout in ["", "1.5", "10s", "$(id)"] {
        cases.push(("//a/...", OK, timeout, "whole seconds"));
    }
    for (scope, platforms, timeout, says) in cases {
        let error = run(scope, platforms, timeout).expect_err(scope);
        assert!(
            error.contains(says),
            "{scope:?} {platforms} {timeout:?}: {error}"
        );
    }
}

#[test]
fn rejects_unknown_task_and_displays() {
    let task = check(&[
        ("DX_SCOPE", "//a/..."),
        ("DX_TASK", "deploy"),
        ("DX_PLATFORMS", OK),
        ("DX_TEST_TIMEOUT", "600"),
    ]);
    assert!(
        task.err()
            .is_some_and(|e| e.contains("task must be one of"))
    );
    let displays = check(&[
        ("DX_SCOPE", "//a/..."),
        ("DX_TASK", "test"),
        ("DX_PLATFORMS", OK),
        ("DX_DISPLAYS", "mir"),
        ("DX_TEST_TIMEOUT", "600"),
    ]);
    assert!(
        displays
            .err()
            .is_some_and(|e| e.contains("displays must be one of"))
    );
}

#[test]
fn accepts_the_coverage_task() {
    let done = check(&[
        ("DX_SCOPE", "//..."),
        ("DX_TASK", "coverage"),
        ("DX_PLATFORMS", OK),
        ("DX_TEST_TIMEOUT", "600"),
    ]);
    assert!(done.is_ok());
}

#[test]
fn accepts_the_build_scripts_task() {
    let done = check(&[
        ("DX_SCOPE", "//..."),
        ("DX_TASK", "build_scripts"),
        ("DX_PLATFORMS", OK),
        ("DX_TEST_TIMEOUT", "600"),
    ]);
    assert!(done.is_ok());
}

fn with_flags(flags: &str) -> Result<Vec<(String, String)>, String> {
    check(&[
        ("DX_SCOPE", "//a:t"),
        ("DX_TASK", "test"),
        ("DX_PLATFORMS", OK),
        ("DX_TEST_TIMEOUT", "600"),
        ("DX_FLAGS", flags),
    ])
}

#[test]
fn accepts_label_and_config_flags() {
    let outputs = with_flags("--//cc:compiler=msvc  --config=ci").unwrap();
    let flags = outputs.iter().find(|(key, _)| key == "flags").unwrap();
    assert_eq!(flags.1, "--//cc:compiler=msvc --config=ci");
}

#[test]
fn rejects_other_flags() {
    for flag in [
        "--output_base=x",
        "--//a:b",
        "--//a:b=",
        "--config=",
        "--//a:b=$(id)",
        "-c",
    ] {
        let error = with_flags(flag).unwrap_err();
        assert!(error.contains("flags holds"), "{flag}: {error}");
    }
}

fn relocate_result(task: &str, relocate: &str) -> Result<Vec<(String, String)>, String> {
    check(&[
        ("DX_SCOPE", "//a:t"),
        ("DX_TASK", task),
        ("DX_PLATFORMS", OK),
        ("DX_TEST_TIMEOUT", "600"),
        ("DX_RELOCATE", relocate),
    ])
}

#[test]
fn accepts_relocate_for_test_and_build() {
    for (task, value, expected) in [
        ("test", "true", "true"),
        ("build", "true", "true"),
        ("test", "false", "false"),
        ("test", "", "false"),
    ] {
        let outputs = relocate_result(task, value).unwrap();
        let (key, shown) = outputs.last().unwrap();
        assert_eq!((key.as_str(), shown.as_str()), ("relocate", expected));
    }
}

#[test]
fn rejects_bad_relocate() {
    let bad = relocate_result("test", "yes").unwrap_err();
    assert!(bad.contains("'yes'"), "{bad}");
    let coverage = relocate_result("coverage", "true").unwrap_err();
    assert!(
        coverage.contains("relocate supports test and build"),
        "{coverage}"
    );
    let build_scripts = relocate_result("build_scripts", "true").unwrap_err();
    assert!(
        build_scripts.contains("relocate supports test and build"),
        "{build_scripts}"
    );
}

#[test]
fn the_workflow_runner_map_matches_platforms() {
    let document = ci_workflow::document(WORKFLOW).unwrap();
    let runs_on =
        ci_workflow::text_at(&document["jobs"]["evidence"]["runs-on"], "evidence runs-on").unwrap();
    let map = runs_on
        .strip_prefix("${{ fromJSON('")
        .and_then(|rest| rest.split_once("')"))
        .map(|(map, _)| map)
        .expect("evidence.yml runs-on runner map");
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(map).expect("runner map is JSON");
    let mut map: Vec<(&str, &str)> = map
        .iter()
        .map(|(name, runner)| (name.as_str(), runner.as_str().expect("runner label")))
        .collect();
    let mut expected = PLATFORMS.to_vec();
    map.sort_unstable();
    expected.sort_unstable();
    assert_eq!(map, expected);
}

#[test]
fn accepts_spaced_platform_lists() {
    for platforms in [
        r#"["linux_x86_64", "windows_arm64"]"#,
        r#"[ "windows_x86_64" ]"#,
        r#"["windows_x86_64","windows_arm64","linux_x86_64"]"#,
    ] {
        run("//cli/process/...", platforms, "600").unwrap();
    }
}

#[test]
fn the_workflow_validates_with_producers_bin_after_setting_up_bazel() {
    let steps = ci_workflow::steps(WORKFLOW, "evidence").unwrap();
    let setup = steps
        .iter()
        .position(|step| step["uses"].as_str() == Some("./.github/actions/setup-bazel"))
        .expect("evidence.yml sets up Bazel");
    let validate = steps
        .iter()
        .position(|step| step["id"].as_str() == Some("inputs"))
        .expect("evidence.yml validates its inputs");
    let run = steps[validate]["run"].as_str().unwrap_or_default();
    assert!(
        run.contains(
            "bazel run --noshow_progress //tools/ci/producers:producers_bin -- --evidence-inputs"
        ),
        "{run}"
    );
    assert_eq!(validate, setup + 1);
}
