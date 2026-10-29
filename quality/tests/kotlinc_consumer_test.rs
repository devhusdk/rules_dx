use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "package lib\n\nclass Count {\n  fun count(): Int {\n    return 1\n  }\n}\n";
const DIRTY: &str =
    "package lib\n\nclass Count {\n  fun count(): Int {\n    return \"not an int\"\n  }\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "kotlin_family",
    family_id = "kotlin",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["kotlinc"],
)

workspace_policy(
    name = "policy",
    families = [":kotlin_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//kotlin/rules:defs.bzl", "kotlin_library")

kotlin_library(
    name = "lib",
    srcs = ["Count.kt"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("kotlin/tests/fixtures/type_error/Bad.kt");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

const PROCESSOR_BUILD: &str = r#"load("@rules_dx//kotlin/rules:defs.bzl", "kotlin_library")
load("@rules_java//java:defs.bzl", "java_library", "java_plugin")

java_plugin(
    name = "processor",
    srcs = ["Processor.java"],
    processor_class = "gen.Processor",
)

java_library(
    name = "dagger_like",
    exported_plugins = [":processor"],
)

# Refers to a class only an annotation processor would generate: kapt targets are skipped, not reported.
kotlin_library(
    name = "lib",
    srcs = ["Count.kt"],
    deps = [":dagger_like"],
)
"#;

const PROCESSOR: &str = "package gen;\n\nimport java.util.Set;\nimport javax.annotation.processing.AbstractProcessor;\nimport javax.annotation.processing.RoundEnvironment;\nimport javax.lang.model.element.TypeElement;\n\npublic class Processor extends AbstractProcessor {\n  @Override\n  public boolean process(Set<? extends TypeElement> annotations, RoundEnvironment env) {\n    return false;\n  }\n}\n";

const GENERATED_REFERENCE: &str =
    "package lib\n\nclass Count {\n  fun count(): Int = GeneratedByProcessor().value\n}\n";

fn consumer(source: &str) -> Case {
    consumer_with(source, LIB_BUILD)
}

fn consumer_with(source: &str, lib_build: &str) -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n# rules_dx imports repositories only rules_kotlin 2.4.21 generates.\nbazel_dep(name = \"rules_kotlin\", version = \"2.4.21\")\nbazel_dep(name = \"rules_java\", version = \"9.9.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), lib_build);
    write(&root.join("lib/Count.kt"), source);
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
fn target_with_a_dependency_exporting_a_processor_is_skipped() {
    let root = consumer_with(GENERATED_REFERENCE, PROCESSOR_BUILD);
    write(&root.join("lib/Processor.java"), PROCESSOR);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn type_error_is_reported_at_its_source_location() {
    let root = consumer(DIRTY);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/Count.kt:5:12:")
            && out.stdout.contains("[kotlinc/RETURN_TYPE_MISMATCH]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/Count.kt")), DIRTY);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}
