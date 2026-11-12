use dx_testing::env_file;

#[test]
fn repo_locks_consistent() {
    let bin = env_file("DX_DEPCHECK_BIN");
    let run = dx_testing::run(
        &bin,
        &[
            "locks",
            "--cargo-manifest",
            env_file("DX_CARGO_MANIFEST").to_string_lossy().as_ref(),
            "--cargo-lock",
            env_file("DX_CARGO_LOCK").to_string_lossy().as_ref(),
            "--uv-manifest",
            env_file("DX_UV_MANIFEST").to_string_lossy().as_ref(),
            "--uv-lock",
            env_file("DX_UV_LOCK").to_string_lossy().as_ref(),
            "--pnpm-manifest",
            env_file("DX_PNPM_MANIFEST").to_string_lossy().as_ref(),
            "--pnpm-lock",
            env_file("DX_PNPM_LOCK").to_string_lossy().as_ref(),
            "--go-manifest",
            env_file("DX_GO_MANIFEST").to_string_lossy().as_ref(),
            "--go-lock",
            env_file("DX_GO_LOCK").to_string_lossy().as_ref(),
            "--maven-artifacts",
            env_file("DX_MAVEN_MANIFEST").to_string_lossy().as_ref(),
            "--maven-lock",
            env_file("DX_MAVEN_LOCK").to_string_lossy().as_ref(),
            "--paket-manifest",
            env_file("DX_PAKET_MANIFEST").to_string_lossy().as_ref(),
            "--paket-lock",
            env_file("DX_PAKET_LOCK").to_string_lossy().as_ref(),
            "--ruby-manifest",
            env_file("DX_RUBY_MANIFEST").to_string_lossy().as_ref(),
            "--ruby-lock",
            env_file("DX_RUBY_LOCK").to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("depcheck locks must execute");
    assert!(
        run.status.success(),
        "workspace locks inconsistent (repin with bazel run //cli/cli:dx -- update)\n{}",
        run.combined()
    );
}
