use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const MAIN: &str = "variable \"name\" {\n  type    = string\n  default = \"example\"\n}\n\nmodule \"child\" {\n  source = \"./modules/child\"\n  name   = var.name\n}\n";

const MAIN_DIRTY: &str = "variable \"name\" {\n  type = string\n  default=\"example\"\n}\n\nmodule \"child\" {\n  source = \"./modules/child\"\n  name   = var.name\n}\n";

const OUTPUTS: &str = "output \"name\" {\n  value = module.child.name\n}\n";

const OUTPUTS_BROKEN: &str = "output \"name\" {\n  value = \n}\n";

const TFVARS: &str = "name = \"example\"\n";

const CHILD: &str =
    "variable \"name\" {\n  type = string\n}\n\noutput \"name\" {\n  value = upper(var.name)\n}\n";

const TERRAFORM_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_terraform_linux_x86_64";

const TOFU_SHA256: &str = "723bebb0df8411daff1e5fc3862e28ff5581fe5634087f4c29a9aa41d78208b5";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

http_archive = use_repo_rule("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

http_archive(
    name = "tofu",
    build_file_content = "exports_files([\"tofu\"])",
    sha256 = "TOFU_SHA256",
    urls = ["https://github.com/opentofu/opentofu/releases/download/v1.13.2/tofu_1.13.2_linux_amd64.zip"],
)
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "terraform_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "terraform",
    format = ["terraform"],
)

workspace_policy(
    name = "policy",
    families = [":terraform_family"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//terraform/rules:defs.bzl", "terraform_module")

terraform_module(
    name = "app",
    srcs = [
        "main.tf",
        "outputs.tf",
        "terraform.tfvars",
    ],
    deps = ["//app/modules/child"],
)
"#;

const CHILD_BUILD: &str = r#"load("@rules_dx//terraform/rules:defs.bzl", "terraform_module")

terraform_module(
    name = "child",
    srcs = ["main.tf"],
    visibility = ["//visibility:public"],
)
"#;

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
    let marker = resolve_runfiles("quality/tests/terraform_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("TOFU_SHA256", TOFU_SHA256),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.tf"), MAIN);
    write(&root.join("app/outputs.tf"), OUTPUTS);
    write(&root.join("app/terraform.tfvars"), TFVARS);
    write(&root.join("app/modules/child/BUILD.bazel"), CHILD_BUILD);
    write(&root.join("app/modules/child/main.tf"), CHILD);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("terraform-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let main = root.join("app/main.tf");
    write(&main, MAIN_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/main.tf:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/outputs.tf"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&main), MAIN_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&main), MAIN);
    assert_eq!(read(&root.join("app/outputs.tf")), OUTPUTS);
    assert_eq!(read(&root.join("app/modules/child/main.tf")), CHILD);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let outputs = root.join("app/outputs.tf");
    write(&outputs, OUTPUTS_BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/outputs.tf:"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&outputs), OUTPUTS_BROKEN);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_missing_provider_never_runs_because_format_does_not_init() {
    let root = consumer();
    write(
        &root.join("app/providers.tf"),
        "terraform {\n  required_providers {\n    aws = {\n      source  = \"hashicorp/aws\"\n      version = \"~> 5.0\"\n    }\n  }\n}\n",
    );
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("\"main.tf\",", "\"main.tf\",\n        \"providers.tf\","),
    );
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(!root.join("app/.terraform").exists());
    assert!(!root.join("app/.terraform.lock.hcl").exists());
}

#[test]
fn the_declared_terraform_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("terraform-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "block releases.hashicorp.com\nallow *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    write(&root.join("app/main.tf"), MAIN_DIRTY);
    let built = dx_with(&root, "format", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/main.tf:"),
        "{}",
        built.combined()
    );

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        TERRAFORM_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("terraform_1.16.1_linux_amd64"),
        "{}",
        miss.combined()
    );
}

#[test]
fn the_declared_module_is_real_and_its_files_reach_the_format_action() {
    let root = consumer();
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_terraform_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //app:app)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    for staged in ["app/main.tf", "app/outputs.tf", "app/terraform.tfvars"] {
        assert!(
            plan.stdout.contains(staged),
            "missing {staged}: {}",
            plan.stdout
        );
    }
}

#[test]
fn an_opentofu_override_formats_the_same_files_and_is_the_tool_that_runs() {
    let root = consumer();
    let tofu = "--@rules_dx//config:tool_terraform=@tofu//:tofu";
    let clean = dx_with(&root, "format", &[], &[tofu]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let main = root.join("app/main.tf");
    write(&main, MAIN_DIRTY);
    let dirty = dx_with(&root, "format", &[], &[tofu]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/main.tf:"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&main), MAIN_DIRTY);

    let applied = dx_with(&root, "format", &["--apply"], &[tofu]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&main), MAIN);

    write(&root.join("app/outputs.tf"), OUTPUTS_BROKEN);
    let broken = dx_with(&root, "format", &[], &[tofu]);
    assert_eq!(broken.status.code(), Some(1), "{}", broken.combined());
    assert!(
        broken.combined().contains("app/outputs.tf:"),
        "{}",
        broken.combined()
    );
    write(&root.join("app/outputs.tf"), OUTPUTS);

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_terraform_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        tofu,
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //app:app)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    assert!(plan.stdout.contains("tofu"), "{}", plan.stdout);
    assert!(
        !plan.stdout.contains("dx_terraform_linux_x86_64/terraform"),
        "{}",
        plan.stdout
    );
}
