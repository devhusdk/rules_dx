use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str = "package lib\n\nobject Count {\n  def count: Int = 1\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_scala", version = "7.3.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")

scala_config = use_extension("@rules_scala//scala/extensions:config.bzl", "scala_config")
scala_config.settings(scala_version = "2.13.18")
use_repo(scala_config, "rules_scala_config")

scala_deps = use_extension("@rules_scala//scala/extensions:deps.bzl", "scala_deps")
scala_deps.scala()

register_toolchains("@rules_dx//scala/toolchain:semanticdb")

MAVEN_BLOCK
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "scala_family",
    family_id = "scala",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["scalac"],
)

workspace_policy(
    name = "policy",
    families = [":scala_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//scala/rules:defs.bzl", "scala_library")

scala_library(
    name = "lib",
    srcs = ["Count.scala"],
)
"#;

fn consumer(source: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("MAVEN_BLOCK", nested::CONSUMER_MAVEN)
            .replace("RULES_DX", &rules.display().to_string()),
    );
    nested::stage_consumer_maven_lock(&root);
    write(&root.join(".bazelrc"), "common --enable_bzlmod\nbuild --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/Count.scala"), source);
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
    let dirty = read(&resolve_runfiles(
        "scala/tests/fixtures/type_error/Bad.scala",
    ));
    let root = consumer(&dirty);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/Count.scala:5:18:") && out.stdout.contains("[scalac/scalac]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/Count.scala")), dirty);
}
