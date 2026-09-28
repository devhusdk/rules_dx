use std::path::PathBuf;

fn depcheck() -> PathBuf {
    let rel =
        std::env::var("DX_DEPCHECK_BIN").expect("DX_DEPCHECK_BIN must name the depcheck binary");
    dx_testing::resolve_runfiles(&rel)
}

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a lock input"));
    dx_testing::resolve_runfiles(&rel)
}

#[test]
fn repo_locks_consistent() {
    let bin = depcheck();
    let run = dx_testing::run(
        &bin,
        &[
            "locks",
            "--cargo-manifest",
            data("DX_CARGO_MANIFEST").to_string_lossy().as_ref(),
            "--cargo-lock",
            data("DX_CARGO_LOCK").to_string_lossy().as_ref(),
            "--uv-manifest",
            data("DX_UV_MANIFEST").to_string_lossy().as_ref(),
            "--uv-lock",
            data("DX_UV_LOCK").to_string_lossy().as_ref(),
            "--pnpm-manifest",
            data("DX_PNPM_MANIFEST").to_string_lossy().as_ref(),
            "--pnpm-lock",
            data("DX_PNPM_LOCK").to_string_lossy().as_ref(),
            "--go-manifest",
            data("DX_GO_MANIFEST").to_string_lossy().as_ref(),
            "--go-lock",
            data("DX_GO_LOCK").to_string_lossy().as_ref(),
            "--maven-artifacts",
            data("DX_MAVEN_MANIFEST").to_string_lossy().as_ref(),
            "--maven-lock",
            data("DX_MAVEN_LOCK").to_string_lossy().as_ref(),
            "--paket-manifest",
            data("DX_PAKET_MANIFEST").to_string_lossy().as_ref(),
            "--paket-lock",
            data("DX_PAKET_LOCK").to_string_lossy().as_ref(),
            "--ruby-manifest",
            data("DX_RUBY_MANIFEST").to_string_lossy().as_ref(),
            "--ruby-lock",
            data("DX_RUBY_LOCK").to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("depcheck locks must execute");
    assert!(
        run.status.success(),
        "workspace locks inconsistent (repin with bazel run //tools:repin-all)\n{}",
        run.combined()
    );
}
