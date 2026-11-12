use dx_testing::env_file;
use std::path::Path;

fn check_pair(eco: &str, manifest: &Path, lock: &Path, label: &str) {
    let run = dx_testing::run(
        &env_file("DX_DEPCHECK_BIN"),
        &[
            "consistency",
            "--ecosystem",
            eco,
            "--manifest",
            manifest.to_string_lossy().as_ref(),
            "--lock",
            lock.to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("depcheck consistency must execute");
    assert!(
        run.status.success(),
        "{label} inconsistent (repin the arrival lock)\n{}",
        run.combined()
    );
}

#[test]
fn adopt_locks_consistent() {
    check_pair(
        "js",
        &env_file("DX_PRETTIER_FIXTURE_MANIFEST"),
        &env_file("DX_PRETTIER_FIXTURE_LOCK"),
        "prettier lock fixture package.json plus pnpm-lock.yaml",
    );
    check_pair(
        "js",
        &env_file("DX_COMPONENT_FIXTURE_MANIFEST"),
        &env_file("DX_COMPONENT_FIXTURE_LOCK"),
        "component docs consumer fixture package.json plus pnpm-lock.yaml",
    );
    check_pair(
        "go",
        &env_file("DX_GO_MANIFEST"),
        &env_file("DX_GO_LOCK"),
        "adopt-go go.mod plus go.sum",
    );
    check_pair(
        "ruby",
        &env_file("DX_RUBY_MANIFEST"),
        &env_file("DX_RUBY_LOCK"),
        "adopt-ruby Gemfile plus Gemfile.lock",
    );
    check_pair(
        "rust",
        &env_file("DX_RUST_MANIFEST"),
        &env_file("DX_RUST_LOCK"),
        "adopt-rust Cargo.toml plus Cargo.lock",
    );
    check_pair(
        "js",
        &env_file("DX_JS_MANIFEST"),
        &env_file("DX_JS_LOCK"),
        "adopt-js-ts package.json plus pnpm-lock.yaml",
    );
    check_pair(
        "python",
        &env_file("DX_PY_MANIFEST"),
        &env_file("DX_PY_LOCK"),
        "adopt-python pyproject.toml plus uv.lock",
    );
    check_pair(
        "js",
        &env_file("DX_PG_JS_MANIFEST"),
        &env_file("DX_PG_JS_LOCK"),
        "adopt-polyglot package.json plus pnpm-lock.yaml",
    );
    check_pair(
        "python",
        &env_file("DX_PG_PY_MANIFEST"),
        &env_file("DX_PG_PY_LOCK"),
        "adopt-polyglot pyproject.toml plus uv.lock",
    );
    check_pair(
        "rust",
        &env_file("DX_PG_RUST_MANIFEST"),
        &env_file("DX_PG_RUST_LOCK"),
        "adopt-polyglot Cargo.toml plus Cargo.lock",
    );

    let scratch_tmp = dx_test_scratch::scratch("adopt-locks-");
    let scratch = scratch_tmp.path().to_path_buf();
    let go_mod = env_file("DX_GO_MANIFEST");
    let go_sum = env_file("DX_GO_LOCK");
    let staged_mod = scratch.join("go.mod");
    let staged_sum = scratch.join("go.sum");
    std::fs::copy(&go_mod, &staged_mod).expect("stage go.mod");
    std::fs::copy(&go_sum, &staged_sum).expect("stage go.sum");
    let text = std::fs::read_to_string(&staged_mod).expect("read go.mod");
    let stale = text.replace(
        "github.com/google/go-cmp v0.6.0",
        "github.com/google/go-cmp v0.7.0",
    );
    std::fs::write(&staged_mod, stale.as_bytes()).expect("write stale go.mod");
    let stale_run = dx_testing::run(
        &env_file("DX_DEPCHECK_BIN"),
        &[
            "consistency",
            "--ecosystem",
            "go",
            "--manifest",
            staged_mod.to_string_lossy().as_ref(),
            "--lock",
            staged_sum.to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("depcheck consistency must execute");
    assert!(
        !stale_run.status.success(),
        "negative control broken: stale go.mod passed consistency (want failure)"
    );

    let missing = scratch.join("does-not-exist.sum");
    let missing_run = dx_testing::run(
        &env_file("DX_DEPCHECK_BIN"),
        &[
            "consistency",
            "--ecosystem",
            "go",
            "--manifest",
            go_mod.to_string_lossy().as_ref(),
            "--lock",
            missing.to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("depcheck consistency must execute");
    assert!(
        !missing_run.status.success(),
        "negative control broken: missing lock passed consistency (want failure)"
    );
}
