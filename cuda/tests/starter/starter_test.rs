use std::path::Path;

use dx_nested as nested;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn bazel(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let run = nested::bazel_in(root, args);
    (run.status.code(), run.stdout, run.stderr)
}

#[test]
fn dx_init_with_a_cuda_fixture_builds_and_tests_without_a_gpu() {
    let case = nested::case("starter");
    let scratch = case.root().to_path_buf();
    let root = nested::scaffold_fixture(&scratch, "cuda", "demo-app");
    for file in ["MODULE.bazel", "BUILD.bazel", "scale.cu", "scale_test.cc"] {
        assert!(root.join(file).is_file(), "{file}");
    }

    let rules = dx_testing::rules_dx_source();
    let module = read(&root.join("MODULE.bazel"));
    assert!(module.contains("bazel_dep(name = \"rules_dx\""), "{module}");
    std::fs::write(
        root.join("MODULE.bazel"),
        format!(
            "{module}local_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    )
    .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let bazelrc = read(&root.join(".bazelrc"));
    std::fs::write(
        root.join(".bazelrc"),
        format!("{bazelrc}{}", dx_testing::preset_rc(&rules)),
    )
    .unwrap_or_else(|error| panic!("write .bazelrc: {error}"));

    let (code, out, err) = bazel(&root, &["build", "--color=no", "--curses=no", "//..."]);
    assert_eq!(code, Some(0), "{out}{err}");

    let (code, out, err) = bazel(&root, &["test", "--color=no", "--curses=no", "//..."]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("//:demo-app_test"), "{out}{err}");
    assert!(out.contains("PASSED"), "{out}{err}");

    // nvcc's host compiler must come from the stanza's cuda_host_llvm, not the ambient C++ toolchain.
    let (code, out, err) = bazel(
        &root,
        &[
            "aquery",
            "--color=no",
            "--curses=no",
            "--output=text",
            "mnemonic(\"CudaCompile\", //:demo-app)",
        ],
    );
    assert_eq!(code, Some(0), "{out}{err}");
    let ccbin = out
        .lines()
        .find(|line| line.contains("-ccbin"))
        .unwrap_or_else(|| panic!("no -ccbin in aquery\n{out}{err}"));
    let after = ccbin.split("-ccbin").nth(1).unwrap_or_default();
    assert!(
        after.trim_start().starts_with("external/cuda_host_llvm")
            || after.contains("cuda_host_llvm"),
        "-ccbin must come from cuda_host_llvm: {ccbin}"
    );
}
