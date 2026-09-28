use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn dx_update(dx: &Path, scratch: &Path, args: &[&str]) -> dx_testing::Run {
    let workspace = scratch.to_string_lossy().into_owned();
    let mut full = vec!["--workspace", workspace.as_str()];
    full.extend(args);
    dx_testing::run(dx, &full, &[]).expect("dx update must execute")
}

#[test]
fn preset_parity() {
    let expected = data("DX_PRESET_EXPECTED");
    let dx = data("DX_DX_BIN");
    let preset_rs = data("DX_PRESET_RS");

    dx_testing::expect_contains(
        &expected,
        &[
            "GENERATED, do not edit",
            "Regenerate: `bazel run //tools/bazelrc:preset_update`",
            "common --enable_bzlmod",
            "build --verbose_failures",
            "test --test_output=errors",
            "common --enable_platform_specific_config",
            "coverage --test_env=GENERATE_LLVM_LCOV=1",
            "coverage --combined_report=lcov",
            "coverage --test_tag_filters=-no-coverage",
            "coverage --enable_runfiles",
            "coverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
            "coverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
            "coverage --instrumentation_filter=^//",
            "build:dx_debug --compilation_mode=dbg",
            "build:dx_dev --compilation_mode=fastbuild",
            "build:dx_release --compilation_mode=opt",
            "build:dx_dev_remote --compilation_mode=fastbuild",
            "build:dx_toolchain --compilation_mode=fastbuild",
            "build:windows --enable_runfiles",
        ],
    )
    .expect("preset.bazelrc missing contract lines");
    dx_testing::expect_contains(
        &preset_rs,
        &[
            "PRESET_BAZEL_VERSION",
            "PRESET_DX_VERSION",
            "\"9.2.0\"",
            "\"0.0.0\"",
        ],
    )
    .expect("preset src lost its Bazel/dx version pins");

    let scratch = dx_testing::mkscratch("preset-parity-").expect("scratch");
    std::fs::write(scratch.join("MODULE.bazel"), b"").expect("write module");
    std::fs::write(
        scratch.join(".bazelrc"),
        b"import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    )
    .expect("write bazelrc");

    let update = dx_update(&dx, &scratch, &["update", "go", "--quiet"]);
    assert!(
        update.status.success(),
        "dx update go failed in scratch workspace\n{}",
        update.combined()
    );

    let actual = scratch.join("tools/bazelrc/preset.bazelrc");
    assert!(
        actual.is_file(),
        "dx update did not create tools/bazelrc/preset.bazelrc"
    );
    dx_testing::snapshot_diff(&expected, &actual, Some("tools/bazelrc/preset.bazelrc"))
        .expect("preset parity snapshot");

    let check = dx_update(&dx, &scratch, &["update", "--check", "--quiet"]);
    assert!(
        check.status.success(),
        "dx update --check failed on the fresh fragment\n{}",
        check.combined()
    );

    std::fs::write(&actual, b"# dirty\n").expect("dirty the fragment");
    let dirty_check = dx_update(&dx, &scratch, &["update", "--check", "--quiet"]);
    assert!(
        !dirty_check.status.success(),
        "dx update --check passed on a dirty fragment"
    );
    let dirty_text = std::fs::read_to_string(&actual).expect("read dirty fragment");
    assert!(
        dirty_text.contains("# dirty"),
        "check mode mutated the dirty fragment (check must never write)"
    );

    let fix = dx_update(&dx, &scratch, &["update", "go", "--quiet"]);
    assert!(
        fix.status.success(),
        "dx update failed to fix the dirty fragment\n{}",
        fix.combined()
    );
    let fixed_check = dx_update(&dx, &scratch, &["update", "--check", "--quiet"]);
    assert!(
        fixed_check.status.success(),
        "dx update --check failed after the fix\n{}",
        fixed_check.combined()
    );
}
