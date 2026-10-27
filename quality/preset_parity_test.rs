use std::path::PathBuf;

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

#[test]
fn preset_parity() {
    let expected_dir = data("DX_PRESET_EXPECTED")
        .parent()
        .expect("the preset layers live in a directory")
        .to_path_buf();
    let preset_rs = data("DX_PRESET_RS");

    let mut expected_text = String::new();
    for layer in dx_preset::Layer::ALL {
        let path = expected_dir.join(layer.file_name());
        let text = std::fs::read_to_string(&path).expect("read checked-in preset layer");
        assert_eq!(
            dx_preset::render_layer(layer),
            text,
            "//tools/bazelrc:preset_update would rewrite tools/bazelrc/{}; \
             run `bazel run //tools/bazelrc:preset_update`",
            layer.file_name()
        );
        expected_text.push_str(&text);
    }
    let expected = std::env::temp_dir().join(format!("dx-preset-all-{}", std::process::id()));
    std::fs::write(&expected, &expected_text).expect("write joined preset");

    dx_testing::expect_contains(
        &expected,
        &[
            "GENERATED, do not edit",
            "Regenerate: `tools/dx init --apply`, or `bazel run //tools/bazelrc:preset_update` in rules_dx.",
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
            "build --host_compilation_mode=fastbuild",
            "build:dx_debug --compilation_mode=dbg",
            "build:dx_dev --compilation_mode=fastbuild",
            "build:dx_release --compilation_mode=opt",
            "build:dx_release --host_compilation_mode=opt",
            "build:dx_dev_remote --compilation_mode=fastbuild",
            "build:dx_toolchain --compilation_mode=fastbuild",
            "build:windows --enable_runfiles",
            "startup --host_jvm_args=-XX:MaxHeapFreeRatio=30",
            "startup --host_jvm_args=-XX:MinHeapFreeRatio=10",
            "startup --host_jvm_args=-XX:MaxRAMPercentage=25",
            "common --local_resources=memory=HOST_RAM*.5",
            "common --local_resources=cpu=HOST_CPUS*.75",
            "startup --max_idle_secs=900",
            "startup --shutdown_on_low_sys_mem",
            "common --repo_contents_cache_gc_max_age=60d",
            "build --experimental_shrink_worker_pool",
            "build --experimental_total_worker_memory_limit_mb=4096",
        ],
    )
    .expect("the preset layers miss contract lines");
    let gc_age = format!(
        "common --experimental_disk_cache_gc_max_age={}",
        dx_process::DISK_CACHE_GC_MAX_AGE
    );
    let gc_size = format!(
        "common --experimental_disk_cache_gc_max_size={}",
        dx_process::DISK_CACHE_GC_MAX_SIZE
    );
    let preset_lines = std::fs::read_to_string(&expected).expect("read checked-in preset");
    for line in [gc_age, gc_size] {
        assert!(
            preset_lines.lines().any(|preset| preset == line),
            "the preset and the default dx disk cache must share one gc bound: {line}"
        );
    }
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
    std::fs::create_dir_all(scratch.join("tools/bazelrc")).expect("create preset dir");
    std::fs::write(scratch.join("MODULE.bazel"), b"").expect("write module");
    std::fs::write(
        scratch.join(".bazelrc"),
        b"import %workspace%/tools/bazelrc/correctness.bazelrc\nimport %workspace%/tools/bazelrc/performance.bazelrc\nimport %workspace%/tools/bazelrc/resources.bazelrc\ntry-import %workspace%/user.bazelrc\n",
    )
    .expect("write bazelrc");

    assert!(
        matches!(
            dx_preset::check_preset(&scratch),
            Err(dx_preset::PresetError::Stale { .. })
        ),
        "a workspace without the layers must be stale"
    );
    dx_preset::update_preset(&scratch).expect("update writes the layers");

    for layer in dx_preset::Layer::ALL {
        let rel = format!("tools/bazelrc/{}", layer.file_name());
        let actual = scratch.join(&rel);
        assert!(actual.is_file(), "update_preset did not create {rel}");
        dx_testing::snapshot_diff(&expected_dir.join(layer.file_name()), &actual, Some(&rel))
            .expect("preset parity snapshot");
    }
    let actual = scratch.join("tools/bazelrc/resources.bazelrc");
    assert_eq!(dx_preset::check_preset(&scratch), Ok(()));

    std::fs::write(&actual, b"# dirty\n").expect("dirty the fragment");
    assert!(
        matches!(
            dx_preset::check_preset(&scratch),
            Err(dx_preset::PresetError::Stale { .. })
        ),
        "check passed on a dirty fragment"
    );
    let dirty_text = std::fs::read_to_string(&actual).expect("read dirty fragment");
    assert!(
        dirty_text.contains("# dirty"),
        "check mutated the dirty fragment"
    );

    dx_preset::update_preset(&scratch).expect("update fixes the dirty fragment");
    assert_eq!(dx_preset::check_preset(&scratch), Ok(()));
}
