use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested as nested;
use dx_testing::resolve_runfiles;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("cuda/tests/starter/starter_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn dx(workspace: &Path, args: &[&str]) -> (Option<i32>, String) {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let output = Command::new(resolve_runfiles(&rel))
        .current_dir(workspace)
        .args(args)
        .arg("--workspace")
        .arg(workspace)
        .output()
        .unwrap_or_else(|error| panic!("dx runs: {error}"));
    (
        output.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

fn bazel(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let run = nested::bazel_in(root, args);
    (run.status.code(), run.stdout, run.stderr)
}

#[test]
fn dx_new_cuda_scaffolds_a_project_that_builds_and_tests_without_a_gpu() {
    let case = nested::case("starter");
    let scratch = case.root().to_path_buf();
    let (code, text) = dx(&scratch, &["new", "cuda", "demo-app"]);
    assert_eq!(code, Some(1), "{text}");
    assert!(!scratch.join("demo-app").exists(), "check must not write");

    let (code, text) = dx(&scratch, &["new", "--apply", "cuda", "demo-app"]);
    assert_eq!(code, Some(0), "{text}");
    let root = scratch.join("demo-app");
    for file in ["MODULE.bazel", "BUILD.bazel", "scale.cu", "scale_test.cc"] {
        assert!(root.join(file).is_file(), "{file}\n{text}");
    }

    let rules = rules_dx_root();
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
        format!(
            "{bazelrc}{}",
            read(&rules.join("tools/bazelrc/preset.bazelrc"))
        ),
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
