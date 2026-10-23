use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const GREETER_DIRTY: &str = "namespace Core;\n\npublic static class Greeter\n{\n  public static string Greet(string name) { return \"hello \"+name; }\n}\n";

const GREETER_FORMATTED: &str = "namespace Core;\n\npublic static class Greeter\n{\n    public static string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n}\n";

const GREETER_TABS: &str = "namespace Core;\n\npublic static class Greeter\n{\n\tpublic static string Greet(string name)\n\t{\n\t\treturn \"hello \" + name;\n\t}\n}\n";

const PROGRAM: &str = "namespace App;\n\npublic static class Program\n{\n    public static int Main()\n    {\n        return Core.Greeter.Greet(\"x\").Length > 0 ? 0 : 1;\n    }\n}\n";

const SYNTAX_ERROR: &str = "namespace Core;\n\npublic static class Broken {{{\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_dotnet", version = "0.22.1")

dotnet = use_extension("@rules_dotnet//dotnet:extensions.bzl", "dotnet")
dotnet.toolchain(dotnet_version = "10.0.201")
use_repo(dotnet, "dotnet_toolchains")

nuget = use_extension("@rules_dx//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
use_repo(nuget, "paket.main")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "csharpier_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "csharp_family",
    family_id = "csharp",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["csharpier"],
)

workspace_policy(
    name = "policy",
    families = [":csharp_family"],
)

csharpier_config(
    name = "tabs",
    src = "tabs.csharpier.yaml",
    visibility = ["//visibility:public"],
)
"#;

const CORE_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_library")

csharp_library(
    name = "core",
    aspect_hints = HINTS,
    srcs = ["Greeter.cs"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_binary")

csharp_binary(
    name = "app",
    srcs = ["Program.cs"],
    deps = [
        "//core",
        "@paket.main//xunit.v3.assert",
    ],
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
    let marker = resolve_runfiles("quality/tests/csharp_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch("csharp-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
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
    write(&root.join("tabs.csharpier.yaml"), "useTabs: true\n");
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[]"),
    );
    write(&root.join("core/Greeter.cs"), GREETER_DIRTY);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/Program.cs"), PROGRAM);
    root
}

fn dx(root: &Path, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["format"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    run(&resolve_runfiles(&rel), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}

#[test]
fn check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let greeter = root.join("core/Greeter.cs");
    let program = root.join("app/Program.cs");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/Greeter.cs:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("app/Program.cs"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeter), GREETER_DIRTY);
    assert_eq!(read(&program), PROGRAM);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
    assert_eq!(read(&program), PROGRAM);

    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
}

#[test]
fn a_bound_config_changes_the_formatting() {
    let root = consumer();
    let greeter = root.join("core/Greeter.cs");
    write(&greeter, GREETER_FORMATTED);
    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[\"//:tabs\"]"),
    );
    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/Greeter.cs:"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeter), GREETER_FORMATTED);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeter), GREETER_TABS);
    assert_eq!(read(&root.join("app/Program.cs")), PROGRAM);
}

#[test]
fn a_syntax_error_is_reported_at_its_position() {
    let root = consumer();
    let greeter = root.join("core/Greeter.cs");
    write(&greeter, SYNTAX_ERROR);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("core/Greeter.cs:3:"),
        "{}",
        out.combined()
    );
    assert!(out.combined().contains("CS1519"), "{}", out.combined());
    assert_eq!(read(&greeter), SYNTAX_ERROR);
}
