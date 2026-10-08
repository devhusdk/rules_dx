use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn dx_update(dx: &Path, scratch: &Path, args: &[&str]) -> dx_testing::Run {
    let (command, rest) = args.split_first().expect("dx_update needs a command");
    let workspace = scratch.to_string_lossy().into_owned();
    let mut full = vec![*command, "--workspace", workspace.as_str()];
    full.extend_from_slice(rest);
    dx_testing::run(dx, &full, &[]).expect("dx update must execute")
}

fn preset_update(bin: &Path, scratch: &Path, args: &[&str]) -> dx_testing::Run {
    let workspace = scratch.to_string_lossy().into_owned();
    dx_testing::run(
        bin,
        args,
        &[("BUILD_WORKSPACE_DIRECTORY", workspace.as_str())],
    )
    .expect("preset_update must execute")
}

fn write_scratch(scratch: &Path) {
    std::fs::write(scratch.join("MODULE.bazel"), b"").expect("write module");
    std::fs::write(
        scratch.join(".bazelrc"),
        b"import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    )
    .expect("write bazelrc");
    std::fs::create_dir_all(scratch.join("tools/bazelrc")).expect("write source dir");
}

#[test]
fn preset_parity() {
    let expected = data("DX_PRESET_EXPECTED");
    let dx = data("DX_DX_BIN");
    let updater = data("DX_PRESET_UPDATE_BIN");
    let preset_rs = data("DX_PRESET_RS");

    let expected_text = std::fs::read_to_string(&expected).expect("read checked-in preset");
    assert_eq!(
        dx_preset::render_fragment(),
        expected_text,
        "//tools/bazelrc:preset_update would rewrite tools/bazelrc/preset.bazelrc; \
         run `bazel run //tools/bazelrc:preset_update`"
    );

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
    write_scratch(&scratch);

    let update = dx_update(&dx, &scratch, &["update", "go", "--quiet"]);
    assert!(
        update.status.success(),
        "dx update go failed in scratch workspace\n{}",
        update.combined()
    );
    let actual = scratch.join("tools/bazelrc/preset.bazelrc");
    assert!(
        !actual.is_file(),
        "dx update must not own tools/bazelrc/preset.bazelrc"
    );

    let create = preset_update(&updater, &scratch, &[]);
    assert!(
        create.status.success(),
        "preset_update failed in scratch workspace\n{}",
        create.combined()
    );
    assert!(
        actual.is_file(),
        "preset_update did not create tools/bazelrc/preset.bazelrc"
    );
    dx_testing::snapshot_diff(&expected, &actual, Some("tools/bazelrc/preset.bazelrc"))
        .expect("preset parity snapshot");

    let check = preset_update(&updater, &scratch, &["--verify-only"]);
    assert!(
        check.status.success(),
        "preset_update --verify-only failed on the fresh fragment\n{}",
        check.combined()
    );

    std::fs::write(&actual, b"# dirty\n").expect("dirty the fragment");
    let dirty_check = preset_update(&updater, &scratch, &["--verify-only"]);
    assert!(
        !dirty_check.status.success(),
        "preset_update --verify-only passed on a dirty fragment"
    );
    let dirty_text = std::fs::read_to_string(&actual).expect("read dirty fragment");
    assert!(
        dirty_text.contains("# dirty"),
        "verify mode mutated the dirty fragment (verify must never write)"
    );

    let fix = preset_update(&updater, &scratch, &[]);
    assert!(
        fix.status.success(),
        "preset_update failed to fix the dirty fragment\n{}",
        fix.combined()
    );
    let fixed_check = preset_update(&updater, &scratch, &["--verify-only"]);
    assert!(
        fixed_check.status.success(),
        "preset_update --verify-only failed after the fix\n{}",
        fixed_check.combined()
    );
}
