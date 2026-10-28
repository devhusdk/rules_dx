use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "module count\n  implicit none\ncontains\n  integer function count_up()\n    count_up = 1\n  end function count_up\nend module count\n";
const DIRTY: &str = "module count\n  implicit none\ncontains\n  integer function count_up()\n    count_up = missing\n  end function count_up\nend module count\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "fortran_family",
    family_id = "fortran",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["flang"],
)

workspace_policy(
    name = "policy",
    families = [":fortran_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//fortran/rules:defs.bzl", "fortran_library")

fortran_library(
    name = "lib",
    srcs = ["count.f90"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("fortran/tests/fixtures/type_error/bad.f90");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(source: &str) -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/count.f90"), source);
    root
}

fn dx(root: &Case) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let argv: Vec<&str> = vec![
        "typecheck",
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ];
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

#[test]
fn clean_project_passes() {
    let root = consumer(CLEAN);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn type_error_is_reported_at_its_source_location() {
    let root = consumer(DIRTY);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/count.f90:5:") && out.stdout.contains("[flang"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/count.f90")), DIRTY);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}
