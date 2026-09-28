use std::path::PathBuf;

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

#[test]
fn devcontainer_parity() {
    let expected = data("DX_DEVCONTAINER_EXPECTED");
    let dx = data("DX_DX_BIN");

    let scratch = dx_testing::mkscratch("devcontainer-parity-").expect("scratch");
    std::fs::write(scratch.join("MODULE.bazel"), b"").expect("write module");
    let init = dx_testing::run(
        &dx,
        &[
            "--workspace",
            scratch.to_string_lossy().as_ref(),
            "init",
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
