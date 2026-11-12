fn run_red(runner_var: &str, manifest_var: &str) -> String {
    let manifest = std::env::var(manifest_var).expect("red runner manifest must be set");
    let out = std::process::Command::new(dx_testing::env_file(runner_var))
        .env("STARLARK_HARNESS_MANIFEST", manifest)
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
fn failing_check() {
    expect_all(
        &run_red("DX_FAILING_CHECK_BIN", "DX_FAILING_CHECK_MANIFEST"),
        &[
            "FAIL: deliberately wrong sum",
            "  expected: 3",
            "  actual:   2",
            "FAIL: deliberately wrong product",
            "starlark_test: 1 passed, 2 failed",
            "PASS: control that still passes",
        ],
    );
}

#[test]
fn missing_observation() {
    expect_all(
        &run_red("DX_MISSING_OBSERVATION_BIN", "DX_MISSING_OBSERVATION_MANIFEST"),
        &[
            "FAIL: observations",
            "field sum=43",
            "field sum=0",
            "starlark_test: 0 passed, 1 failed",
        ],
    );
}

#[test]
fn missing_fragment() {
    expect_all(
        &run_red("DX_MISSING_FRAGMENT_BIN", "DX_MISSING_FRAGMENT_MANIFEST"),
        &[
            "FAIL: file //core/tests/negative:present_fixture.txt is missing substring 1/1",
            "substring: this substring is absent",
            "starlark_test: 0 passed, 1 failed",
        ],
    );
}
