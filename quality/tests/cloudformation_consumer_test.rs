use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const PARENT: &str = "AWSTemplateFormatVersion: \"2010-09-09\"\nParameters:\n  Prefix:\n    Type: String\n    Default: dx\nResources:\n  Bucket:\n    Type: AWS::S3::Bucket\n    Properties:\n      BucketName: !Sub \"${Prefix}-${AWS::AccountId}-data\"\n  Child:\n    Type: AWS::CloudFormation::Stack\n    Properties:\n      TemplateURL: https://s3.amazonaws.com/example/child.yaml\n      Parameters:\n        Name: !Ref Bucket\nOutputs:\n  Arn:\n    Value: !GetAtt Bucket.Arn\n";

const PARENT_DIRTY: &str = "AWSTemplateFormatVersion: \"2010-09-09\"\nResources:\n  Bucket:\n    Type: AWS::S3::Bucket\n    Properties:\n      Bogus: true\n";

const TOPIC: &str =
    "{\n  \"Resources\": {\n    \"Topic\": {\n      \"Type\": \"AWS::SNS::Topic\"\n    }\n  }\n}\n";

const PLAIN: &str = "name: not-a-template\nvalue: 1\n";

const BROKEN: &str = "Resources: [unclosed\n";

const RC: &str = "ignore_checks:\n  - E3002\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
include("//third_party/rules_dx:rules_dx.MODULE.bazel")

# rules_dx imports repositories only rules_kotlin 2.4.21 generates.
bazel_dep(name = "rules_kotlin", version = "2.4.21")

bazel_dep(name = "rules_rust", version = "0.75.0")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.6")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "cfn_lint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "cloudformation_family",
    family_id = "cloudformation",
    disabled = [
        "format",
        "typecheck",
    ],
    lint = ["cfn_lint"],
)

workspace_policy(
    name = "policy",
    families = [":cloudformation_family"],
)

cfn_lint_config(
    name = "relaxed",
    src = "relaxed.cfnlintrc",
    visibility = ["//visibility:public"],
)
"#;

const STACK_BUILD: &str = r#"load("@rules_dx//cloudformation/rules:defs.bzl", "cloudformation_template")

cloudformation_template(
    name = "stack",
    aspect_hints = HINTS,
    srcs = ["parent.yaml"],
)
"#;

const TOPIC_BUILD: &str = r#"load("@rules_dx//cloudformation/rules:defs.bzl", "cloudformation_template")

cloudformation_template(
    name = "topic",
    srcs = ["topic.json"],
)
"#;

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("stack/BUILD.bazel"),
        &STACK_BUILD.replace("HINTS", hints),
    );
}

/// Writes the `third_party/rules_dx` include `dx init` gives every consumer: rules_dx is a
/// non-root module, so its patched-dependency overrides only apply through that include.
fn adopt_overrides(root: &Case) {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let home_dir = scratch("init-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let workspace = root.to_string_lossy().into_owned();
    let init = root.run(
        &resolve_runfiles(&rel),
        &[
            "init",
            "--apply",
            "--output=text",
            "--workspace",
            workspace.as_str(),
        ],
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    );
    assert_eq!(init.status.code(), Some(0), "{}", init.combined());
}

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    adopt_overrides(&root);
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("relaxed.cfnlintrc"), RC);
    set_hints(&root, "[]");
    write(&root.join("stack/parent.yaml"), PARENT);
    write(&root.join("topic/BUILD.bazel"), TOPIC_BUILD);
    write(&root.join("topic/topic.json"), TOPIC);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("cloudformation-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn lint(root: &Case) -> Run {
    dx_with(root, "lint", &[], &[])
}

fn assert_unchanged(root: &Case, parent: &str) {
    assert_eq!(read(&root.join("stack/parent.yaml")), parent);
    assert_eq!(read(&root.join("topic/topic.json")), TOPIC);
}

#[test]
fn templates_with_intrinsics_a_nested_stack_and_a_resources_only_json_lint_clean() {
    let root = consumer();
    let clean = lint(&root);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
}

#[test]
fn a_violation_is_located_and_check_leaves_files_unchanged() {
    let root = consumer();
    write(&root.join("stack/parent.yaml"), PARENT_DIRTY);
    let dirty = lint(&root);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    let out = dirty.combined();
    assert!(out.contains("stack/parent.yaml:"), "{out}");
    assert!(out.contains("E3002"), "{out}");
    assert!(!out.contains("topic/topic.json:"), "{out}");
    assert_unchanged(&root, PARENT_DIRTY);
}

#[test]
fn a_bound_non_default_config_changes_the_result() {
    let root = consumer();
    write(&root.join("stack/parent.yaml"), PARENT_DIRTY);
    let before = lint(&root);
    assert_eq!(before.status.code(), Some(1), "{}", before.combined());
    set_hints(&root, "[\"//:relaxed\"]");
    let after = lint(&root);
    assert_eq!(after.status.code(), Some(0), "{}", after.combined());
}

#[test]
fn a_plain_yaml_file_in_a_template_target_is_a_located_error() {
    let root = consumer();
    write(&root.join("stack/parent.yaml"), PLAIN);
    let plain = lint(&root);
    assert_ne!(plain.status.code(), Some(0), "{}", plain.combined());
    assert!(
        plain.combined().contains("stack/parent.yaml"),
        "{}",
        plain.combined()
    );
    assert_unchanged(&root, PLAIN);
}

#[test]
fn a_syntax_error_is_never_clean() {
    let root = consumer();
    write(&root.join("stack/parent.yaml"), BROKEN);
    let broken = lint(&root);
    assert_ne!(broken.status.code(), Some(0), "{}", broken.combined());
    assert!(
        broken.combined().contains("stack/parent.yaml"),
        "{}",
        broken.combined()
    );
    assert_unchanged(&root, BROKEN);
}

#[test]
fn apply_never_rewrites_a_template() {
    let root = consumer();
    write(&root.join("stack/parent.yaml"), PARENT_DIRTY);
    let applied = dx_with(&root, "lint", &["--apply"], &[]);
    assert_eq!(applied.status.code(), Some(1), "{}", applied.combined());
    assert_unchanged(&root, PARENT_DIRTY);
}

#[test]
fn the_declared_cfn_lint_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = lint(&root);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("cloudformation-denied");
    let denied = scratch_dir.path().join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("stack/parent.yaml"), PARENT_DIRTY);
    let built = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("stack/parent.yaml:"),
        "{}",
        built.combined()
    );
}
