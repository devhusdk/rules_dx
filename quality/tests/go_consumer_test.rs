use std::os::unix::fs::PermissionsExt;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const GREET: &str = "package greet\n\nconst Mode = 0o644\n\nfunc Greet(name string) string {\n\treturn \"hello \" + name\n}\n";

const GREET_DIRTY: &str = "package greet\n\nconst Mode = 0644\n\nfunc Greet( name string)string{\n\treturn \"hello \"+name\n}\n";

const HELPER: &str =
    "package greet\n\nfunc shout(name string) string {\n\treturn Greet(name) + \"!\"\n}\n";

const HELPER_BROKEN: &str =
    "package greet\n\nfunc shout(name string) string {\n\treturn Greet(name +\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_go", version = "0.64.2")
bazel_dep(name = "rules_shell", version = "0.8.0")

go_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")
go_sdk.download(version = "1.26.6")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "go_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "go",
    format = ["gofumpt"],
)

workspace_policy(
    name = "policy",
    families = [":go_family"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//go/rules:defs.bzl", "go_library")

go_library(
    name = "greet",
    srcs = [
        "greet.go",
        "helper.go",
    ],
    importpath = "example.com/consumer/greet",
)
"#;

const TOOLS_BUILD: &str = r#"load("@rules_shell//shell:sh_binary.bzl", "sh_binary")

sh_binary(
    name = "gofumpt",
    srcs = ["gofumpt.sh"],
)

sh_binary(
    name = "gotype",
    srcs = ["gotype.sh"],
)
"#;

const TYPECHECK_BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "go_family",
    disabled = ["lint"],
    family_id = "go",
    format = ["gofumpt"],
    typecheck = ["gotype"],
)

workspace_policy(
    name = "policy",
    families = [":go_family"],
)
"#;

const TYPECHECK_APP_BUILD: &str = r#"load("@rules_dx//go/rules:defs.bzl", "go_library", "go_test")

go_library(
    name = "greet",
    srcs = [
        "greet.go",
        "helper.go",
    ],
    importpath = "example.com/consumer/greet",
)

go_test(
    name = "greet_test",
    srcs = ["greet_test.go"],
    embed = [":greet"],
)
"#;

const GREET_FMT: &str = "package greet\n\nimport \"fmt\"\n\nfunc Greet(name string) string {\n\treturn fmt.Sprintf(\"hello %s\", name)\n}\n";

const HELPER_TYPE_ERROR: &str =
    "package greet\n\nfunc shout(name string) int {\n\treturn Greet(name)\n}\n";

const GREET_EXTERNAL_TEST: &str = "package greet_test\n\nimport (\n\t\"testing\"\n\n\t\"example.com/consumer/greet\"\n)\n\nfunc TestGreet(t *testing.T) {\n\tif greet.Greet(\"x\") == \"\" {\n\t\tt.Fatal(\"empty\")\n\t}\n}\n";

const GREET_EXTERNAL_TEST_ERROR: &str = "package greet_test\n\nimport (\n\t\"testing\"\n\n\t\"example.com/consumer/greet\"\n)\n\nfunc TestGreet(t *testing.T) {\n\tvar n int = greet.Greet(\"x\")\n\t_ = n\n}\n";

const FAKE_GOTYPE: &str = "#!/bin/sh\necho \"override gotype ran\" >&2\nexit 3\n";

const FAKE_GOFUMPT: &str = "#!/bin/sh\necho \"override gofumpt ran\" >&2\nexit 3\n";

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
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("greet/BUILD.bazel"), APP_BUILD);
    write(&root.join("greet/greet.go"), GREET);
    write(&root.join("greet/helper.go"), HELPER);
    write(&root.join("tools/BUILD.bazel"), TOOLS_BUILD);
    let fake = root.join("tools/gofumpt.sh");
    write(&fake, FAKE_GOFUMPT);
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod {fake:?}: {error}"));
    let fake_type = root.join("tools/gotype.sh");
    write(&fake_type, FAKE_GOTYPE);
    std::fs::set_permissions(&fake_type, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod {fake_type:?}: {error}"));
    root
}

fn typecheck_consumer() -> Case {
    let root = consumer();
    write(&root.join("BUILD.bazel"), TYPECHECK_BUILD);
    write(&root.join("greet/BUILD.bazel"), TYPECHECK_APP_BUILD);
    write(&root.join("greet/greet.go"), GREET_FMT);
    write(&root.join("greet/greet_test.go"), GREET_EXTERNAL_TEST);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("go-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//greet/...", "--"]);
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

    let greet = root.join("greet/greet.go");
    write(&greet, GREET_DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("greet/greet.go:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("greet/helper.go"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greet), GREET_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greet), GREET);
    assert_eq!(read(&root.join("greet/helper.go")), HELPER);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_syntax_error_is_reported_and_never_clean() {
    let root = consumer();
    let helper = root.join("greet/helper.go");
    write(&helper, HELPER_BROKEN);
    let out = dx(&root, "format", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.combined().contains("gofumpt"), "{}", out.combined());
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&helper), HELPER_BROKEN);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn an_override_is_the_gofumpt_that_runs() {
    let root = consumer();
    let flag = "--@rules_dx//config:tool_gofumpt=//tools:gofumpt";
    let out = dx_with(&root, "format", &[], &[flag]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_go_format_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        flag,
        "--include_aspects",
        "--output=text",
        "mnemonic(\"DxRealQualityFormat\", //greet:greet)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    assert!(plan.stdout.contains("tools/gofumpt"), "{}", plan.stdout);
    assert!(!plan.stdout.contains("cc_mvdan_gofumpt"), "{}", plan.stdout);
    for staged in ["greet/greet.go", "greet/helper.go"] {
        assert!(
            plan.stdout.contains(staged),
            "missing {staged}: {}",
            plan.stdout
        );
    }
}

#[test]
fn typecheck_passes_a_clean_package_with_stdlib_imports_and_an_external_test() {
    let root = typecheck_consumer();
    let out = dx(&root, "typecheck", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn typecheck_reports_a_type_error_and_exits_non_zero() {
    let root = typecheck_consumer();
    write(&root.join("greet/helper.go"), HELPER_TYPE_ERROR);
    let out = dx(&root, "typecheck", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("greet/helper.go"),
        "{}",
        out.combined()
    );
}

#[test]
fn typecheck_reports_a_syntax_error_and_never_passes() {
    let root = typecheck_consumer();
    write(&root.join("greet/helper.go"), HELPER_BROKEN);
    let out = dx(&root, "typecheck", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn typecheck_reports_an_error_in_the_external_test_package() {
    let root = typecheck_consumer();
    write(&root.join("greet/greet_test.go"), GREET_EXTERNAL_TEST_ERROR);
    let out = dx(&root, "typecheck", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("greet/greet_test.go"),
        "{}",
        out.combined()
    );
}

#[test]
fn an_override_is_the_gotype_driver_that_runs() {
    let root = typecheck_consumer();
    let flag = "--@rules_dx//config:tool_gotype=//tools:gotype";
    let out = dx_with(&root, "typecheck", &[], &[flag]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());

    let plan = root.bazel(&[
        "aquery",
        "--aspects=@rules_dx//quality:real_aspects.bzl%real_go_typecheck_aspect",
        "--output_groups=dx_results",
        "--@rules_dx//config:workspace=//:policy",
        flag,
        "--include_aspects",
        "--output=text",
        "mnemonic(\"GoTypeCheck\", //greet:greet)",
    ]);
    assert!(plan.status.success(), "{}", plan.combined());
    assert!(plan.stdout.contains("tools/gotype"), "{}", plan.stdout);
    assert!(!plan.stdout.contains("dx_gotype"), "{}", plan.stdout);
}
