//! A consumer adopts rules_dx with `dx init --apply`, then builds and runs a `python_binary`
//! and a `rust_binary` through `@rules_dx`, and a corrupted override patch is refused (#2283).

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "platforms", version = "1.1.0")
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")
bazel_dep(name = "rules_python", version = "RULES_PYTHON")
bazel_dep(name = "aspect_rules_py", version = "ASPECT_RULES_PY")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")

MAVEN_BLOCK
"#;

const BUILD: &str = r#"load("@rules_dx//python/rules:defs.bzl", "python_binary")
load("@rules_dx//rust/rules:defs.bzl", "rust_binary")

python_binary(
    name = "py_app",
    srcs = ["py_app.py"],
    main = "py_app.py",
)

rust_binary(
    name = "rs_app",
    srcs = ["rs_app.rs"],
)
"#;

const PATCH: &str = "third_party/rules_dx/rules_rust.lld_link.patch";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("python/tests/consumer/overrides_consumer_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing from versions.bzl"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let versions = read(&rules.join("modules/versions.bzl"));
    let case = nested::case("overrides");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("MAVEN_BLOCK", nested::CONSUMER_MAVEN)
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_PYTHON", &pin(&versions, "FLOOR_RULES_PYTHON"))
            .replace("ASPECT_RULES_PY", &pin(&versions, "FLOOR_ASPECT_RULES_PY")),
    );
    nested::seed_rules_dx_lock(&root, &rules);
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&rules));
    nested::stage_consumer_maven_lock(&root);
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("py_app.py"), "print(\"hello from python\")\n");
    write(
        &root.join("rs_app.rs"),
        "fn main() {\n    println!(\"hello from rust\");\n}\n",
    );
    case
}

fn dx(case: &Case, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let bin = resolve_runfiles(&rel);
    let mut command = Command::new(bin);
    command.current_dir(case.root()).args(args);
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("run dx: {error}"));
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn adopt(case: &Case) {
    let applied = dx(case, &["init", "--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(case.root().join(PATCH).is_file(), "{}", applied.combined());
    assert!(
        read(&case.root().join("MODULE.bazel")).contains("third_party/rules_dx"),
        "MODULE.bazel does not include the overrides"
    );
}

#[test]
fn python_and_rust_binaries_run_through_rules_dx_after_init() {
    let case = consumer();
    adopt(&case);
    for (target, expected) in [
        ("//:py_app", "hello from python"),
        ("//:rs_app", "hello from rust"),
    ] {
        let ran = case.bazel(&["run", "--color=no", "--curses=no", target]);
        assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
        assert!(ran.stdout.contains(expected), "{}", ran.combined());
    }
}

#[test]
fn a_corrupted_override_patch_fails_dx_and_init_repairs_it() {
    let case = consumer();
    adopt(&case);
    let patch = case.root().join(PATCH);
    let mut bytes = std::fs::read(&patch).unwrap_or_else(|error| panic!("{error}"));
    bytes[0] ^= 1;
    std::fs::write(&patch, bytes).unwrap_or_else(|error| panic!("{error}"));

    let status = dx(&case, &["status"]);
    assert!(
        status.combined().contains("rules_rust.lld_link.patch"),
        "{}",
        status.combined()
    );

    let repaired = dx(&case, &["init", "--apply"]);
    assert_eq!(repaired.status.code(), Some(0), "{}", repaired.combined());
    let healthy = dx(&case, &["status"]);
    assert!(
        !healthy.combined().contains("rules_rust.lld_link.patch"),
        "{}",
        healthy.combined()
    );
}
