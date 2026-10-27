use dx_nested::{self as nested, Case};
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const CORE: &str = "package core\n\nfinal class Box(val value: Int)\n\nobject Core {\n  def make(): Box = new Box(1)\n}\n";

const APP: &str = "package app\n\nimport com.google.common.collect.ImmutableList\nimport core.Core\n\nobject App {\n  def box = Core.make()\n  def names = ImmutableList.of(\"a\", \"b\")\n}\n";

const APP_COMPLIANT: &str = "package app\n\nimport com.google.common.collect.ImmutableList\nimport core.Box\nimport core.Core\n\nobject App {\n  def box: Box = Core.make()\n  def names: ImmutableList[String] = ImmutableList.of(\"a\", \"b\")\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_scala", version = "7.3.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")

scala_config = use_extension("@rules_scala//scala/extensions:config.bzl", "scala_config")
scala_config.settings(scala_version = "2.13.18")
use_repo(scala_config, "rules_scala_config")

scala_deps = use_extension("@rules_scala//scala/extensions:deps.bzl", "scala_deps")
scala_deps.scala()

register_toolchains("@rules_dx//scala/toolchain:semanticdb")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:32.0.1-jre"],
    known_contributing_modules = ["protobuf", "rules_dx"],
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven")
"#;

const MODULE_WITHOUT_SEMANTICDB: &str =
    "register_toolchains(\"@rules_dx//scala/toolchain:semanticdb\")\n";

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "scalafix_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "scala_family",
    family_id = "scala",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["scalafix"],
)

workspace_policy(
    name = "policy",
    families = [":scala_family"],
)

scalafix_config(
    name = "types",
    src = "types.scalafix.conf",
    visibility = ["//visibility:public"],
)

scalafix_config(
    name = "vars",
    src = "vars.scalafix.conf",
    visibility = ["//visibility:public"],
)
"#;

const CORE_BUILD: &str = r#"load("@rules_dx//scala/rules:defs.bzl", "scala_library")

scala_library(
    name = "core",
    aspect_hints = ["//:types"],
    srcs = ["Core.scala"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//scala/rules:defs.bzl", "scala_library")

scala_library(
    name = "app",
    aspect_hints = ["//:CONFIG"],
    srcs = ["App.scala"],
    deps = [
        "//core",
        "@maven//:com_google_guava_guava",
    ],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\nbuild --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(
        &root.join("types.scalafix.conf"),
        "rules = [ExplicitResultTypes]\n",
    );
    write(
        &root.join("vars.scalafix.conf"),
        "rules = [DisableSyntax]\nDisableSyntax.noVars = true\n",
    );
    write(&root.join("core/BUILD.bazel"), CORE_BUILD);
    write(&root.join("core/Core.scala"), CORE);
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("CONFIG", "types"),
    );
    write(&root.join("app/App.scala"), APP);
    root
}

fn dx(root: &Case, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["lint"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

#[test]
fn check_reports_apply_fixes_and_recheck_passes_across_two_modules() {
    let root = consumer();
    let source = root.join("app/App.scala");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/App.scala:"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&source), APP);
    assert_eq!(read(&root.join("core/Core.scala")), CORE);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let fixed = read(&source);
    assert!(fixed.contains("def box: Box = "), "{fixed}");
    assert!(
        fixed.contains("def names: ImmutableList[String] = "),
        "{fixed}"
    );
    assert_eq!(read(&root.join("core/Core.scala")), CORE);

    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&source), fixed);
}

#[test]
fn a_compliant_source_is_clean() {
    let root = consumer();
    write(&root.join("app/App.scala"), APP_COMPLIANT);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_bound_config_changes_the_rules() {
    let root = consumer();
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("CONFIG", "vars"),
    );
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());

    write(
        &root.join("app/App.scala"),
        &APP.replace("object App {", "object App {\n  var count = 0"),
    );
    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("DisableSyntax.var"),
        "{}",
        dirty.combined()
    );
}

#[test]
fn a_target_without_a_config_fails_and_names_the_config() {
    let root = consumer();
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("    aspect_hints = [\"//:CONFIG\"],\n", ""),
    );
    let out = dx(&root, &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.combined().contains("Scalafix"), "{}", out.combined());
    assert_eq!(read(&root.join("app/App.scala")), APP);
}

#[test]
fn missing_semanticdb_toolchain_fails_and_names_the_fix() {
    let root = consumer();
    let module = read(&root.join("MODULE.bazel")).replace(MODULE_WITHOUT_SEMANTICDB, "");
    write(&root.join("MODULE.bazel"), &module);
    let out = dx(&root, &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("register_toolchains(\"@rules_dx//scala/toolchain:semanticdb\")"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("app/App.scala")), APP);
}
