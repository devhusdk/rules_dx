use dx_testing::env_file;

#[test]
fn devcontainer_parity() {
    let expected = env_file("DX_DEVCONTAINER_EXPECTED");
    let dx = env_file("DX_DX_BIN");

    let scratch_guard = dx_test_scratch::scratch("devcontainer-parity-");

    let scratch = scratch_guard.path();
    std::fs::write(scratch.join("MODULE.bazel"), b"").expect("write module");
    let init = dx_testing::run(
        &dx,
        &[
            "init",
            "--apply",
            "--no-generate",
            "--workspace",
            scratch.to_string_lossy().as_ref(),
            "--quiet",
        ],
        &[],
    )
    .expect("dx init must execute");
    assert!(init.status.success(), "dx init failed\n{}", init.combined());

    let actual = scratch.join(".devcontainer/devcontainer.json");
    dx_testing::assert_valid_json(&expected).expect("checked-in devcontainer.json");
    dx_testing::assert_valid_json(&actual).expect("scaffolded devcontainer.json");
    dx_testing::snapshot_diff(&expected, &actual, Some(".devcontainer/devcontainer.json"))
        .expect("devcontainer parity snapshot");
}
