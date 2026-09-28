use std::path::PathBuf;

fn runner() -> PathBuf {
    let rel = std::env::var("DX_RUNNER_BIN").expect("DX_RUNNER_BIN must name the red runner");
    dx_testing::resolve_runfiles(&rel)
}

fn run_red() -> String {
    let out = std::process::Command::new(runner())
        .output()
        .expect("red runner must execute");
    assert!(
        !out.status.success(),
        "red runner unexpectedly passed (proof is void)"
    );
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    combined
}

fn expect_all(output: &str, wants: &[&str]) {
    for want in wants {
        assert!(
            output.contains(want),
            "missing documented diagnostic: {want}\n{output}"
        );
    }
}

#[test]
fn negative_proof() {
    let case = std::env::var("DX_NEGATIVE_CASE").expect("DX_NEGATIVE_CASE must select the proof");
    let output = run_red();
    match case.as_str() {
        "failing_check" => expect_all(
            &output,
            &[
                "FAIL: deliberately wrong sum",
                "  expected: 3",
                "  actual:   2",
                "FAIL: deliberately wrong product",
                "starlark_test: 1 passed, 2 failed",
                "PASS: control that still passes",
            ],
        ),
        "missing_observation" => expect_all(
            &output,
            &[
                "FAIL: observations",
                "field sum=43",
                "field sum=0",
                "starlark_test: 0 passed, 1 failed",
            ],
        ),
        "missing_fragment" => expect_all(
            &output,
            &[
                "FAIL: file //libs/starlark/tests/negative:present_fixture.txt is missing substring 1/1",
                "substring: this substring is absent",
                "starlark_test: 0 passed, 1 failed",
            ],
        ),
        other => panic!("unknown DX_NEGATIVE_CASE: {other}"),
    }
}
