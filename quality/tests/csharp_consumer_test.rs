use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const GREETER_DIRTY: &str = "namespace Core;\n\npublic static class Greeter\n{\n  public static string Greet(string name) { return \"hello \"+name; }\n}\n";

const GREETER_FORMATTED: &str = "namespace Core;\n\npublic static class Greeter\n{\n    public static string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n}\n";

const GREETER_TABS: &str = "namespace Core;\n\npublic static class Greeter\n{\n\tpublic static string Greet(string name)\n\t{\n\t\treturn \"hello \" + name;\n\t}\n}\n";

const PROGRAM: &str = "namespace App;\n\npublic static class Program\n{\n    public static int Main()\n    {\n        return Core.Greeter.Greet(\"x\").Length > 0 ? 0 : 1;\n    }\n}\n";

const LINT_GREETER: &str = "namespace Core;\n\npublic class Greeter\n{\n    public string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n}\n";

const LINT_PROGRAM: &str = "namespace App;\n\npublic class Program\n{\n    public int Check(string name)\n    {\n        Xunit.Assert.NotNull(name);\n        return new Core.Greeter().Greet(name).Length;\n    }\n\n    public static int Main()\n    {\n        return 0;\n    }\n}\n";

const TOP_LEVEL_PROGRAM: &str = "#if TRACE && DEBUG\nSystem.Console.WriteLine(new Core.Greeter().Greet(\"x\"));\n#endif\nreturn 0;\n";

const GENERATED_APP_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_binary")

genrule(
    name = "generated",
    outs = ["Generated.cs"],
    cmd = "printf 'namespace App;\\npublic static class Generated { public static int Value = ; }\\n' > $@",
)

csharp_binary(
    name = "app",
    srcs = ["Program.cs", ":generated"],
    analyzer_configs = CONFIGS,
    deps = [
        "//core",
        "@paket.main//xunit.v3.assert",
    ],
)
"#;

const STRICT: &str =
    "is_global = true\nglobal_level = 100\n\ndotnet_diagnostic.CA1822.severity = warning\n";

const QUIET: &str =
    "is_global = true\nglobal_level = 100\n\ndotnet_diagnostic.CA1822.severity = none\n";

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
        "typecheck",
    ],
    format = ["csharpier"],
    lint = ["roslyn"],
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

exports_files([
    "quiet.globalconfig",
    "strict.globalconfig",
])
"#;

const CORE_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_library")

csharp_library(
    name = "core",
    analyzer_configs = CONFIGS,
    aspect_hints = HINTS,
    srcs = ["Greeter.cs"],
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_binary")

csharp_binary(
    name = "app",
    srcs = ["Program.cs"],
    analyzer_configs = CONFIGS,
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

fn consumer() -> Case {
    let rules = rules_dx_root();
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
    write(&root.join("tabs.csharpier.yaml"), "useTabs: true\n");
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[]").replace("CONFIGS", "[]"),
    );
    write(&root.join("core/Greeter.cs"), GREETER_DIRTY);
    write(&root.join("quiet.globalconfig"), QUIET);
    write(&root.join("strict.globalconfig"), STRICT);
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("CONFIGS", "[]"),
    );
    write(&root.join("app/Program.cs"), PROGRAM);
    root
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_command(root, "format", args)
}

fn dx_command(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
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
        &CORE_BUILD
            .replace("HINTS", "[\"//:tabs\"]")
            .replace("CONFIGS", "[]"),
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

fn lint_consumer(configs: &str) -> Case {
    let root = consumer();
    write(&root.join("core/Greeter.cs"), LINT_GREETER);
    write(&root.join("app/Program.cs"), LINT_PROGRAM);
    set_configs(&root, configs);
    root
}

fn set_configs(root: &Case, configs: &str) {
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD
            .replace("HINTS", "[]")
            .replace("CONFIGS", configs),
    );
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("CONFIGS", configs),
    );
}

#[test]
fn lint_runs_the_sdk_analyzers_across_projects_and_a_declared_config_changes_them() {
    let root = lint_consumer("[\"//:strict.globalconfig\"]");
    let greeter = root.join("core/Greeter.cs");
    let program = root.join("app/Program.cs");

    let dirty = dx_command(&root, "lint", &[]);
    let text = dirty.combined();
    assert_eq!(dirty.status.code(), Some(1), "{text}");
    let core = text
        .lines()
        .find(|line| line.contains("core/Greeter.cs:5:"))
        .unwrap_or_else(|| panic!("no finding in core: {text}"));
    assert!(core.contains("CA1822"), "{core}");
    let app = text
        .lines()
        .find(|line| line.contains("app/Program.cs:5:"))
        .unwrap_or_else(|| panic!("no finding in app: {text}"));
    assert!(app.contains("CA1822"), "{app}");
    assert_eq!(read(&greeter), LINT_GREETER);
    assert_eq!(read(&program), LINT_PROGRAM);

    let applied = dx_command(&root, "lint", &["--apply"]);
    assert_eq!(applied.status.code(), Some(1), "{}", applied.combined());
    assert_eq!(read(&greeter), LINT_GREETER);
    assert_eq!(read(&program), LINT_PROGRAM);

    set_configs(&root, "[\"//:quiet.globalconfig\"]");
    let quiet = dx_command(&root, "lint", &[]);
    assert_eq!(quiet.status.code(), Some(0), "{}", quiet.combined());
}

#[test]
fn a_compile_error_is_reported_at_its_position_and_never_counts_as_clean() {
    let root = lint_consumer("[]");
    let greeter = root.join("core/Greeter.cs");
    write(&greeter, SYNTAX_ERROR);
    let out = dx_command(&root, "lint", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    let line = text
        .lines()
        .find(|line| line.contains("core/Greeter.cs:3:"))
        .unwrap_or_else(|| panic!("no finding at line 3: {text}"));
    assert!(line.contains("CS1519"), "{line}");
    assert_eq!(read(&greeter), SYNTAX_ERROR);
}

#[test]
fn top_level_statements_in_a_binary_lint_clean() {
    let root = lint_consumer("[\"//:quiet.globalconfig\"]");
    write(&root.join("app/Program.cs"), TOP_LEVEL_PROGRAM);
    let out = dx_command(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_compile_error_in_a_generated_source_never_counts_as_clean() {
    let root = lint_consumer("[\"//:quiet.globalconfig\"]");
    write(
        &root.join("app/BUILD.bazel"),
        &GENERATED_APP_BUILD.replace("CONFIGS", "[\"//:quiet.globalconfig\"]"),
    );
    let out = dx_command(&root, "lint", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("roslyn"), "{text}");
}

const MULTI_TFM_GREETER: &str = "namespace Core;\n\npublic class Greeter\n{\n    public string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n#if NET8_0\n    public int Legacy()\n    {\n        return 8;\n    }\n#endif\n}\n";

const MULTI_TFM_CORE_BUILD: &str = r#"load("@rules_dx//csharp/rules:defs.bzl", "csharp_library")

csharp_library(
    name = "core",
    analyzer_configs = ["//:strict.globalconfig"],
    srcs = ["Greeter.cs"],
    target_frameworks = ["net8.0", "net10.0"],
    visibility = ["//visibility:public"],
)
"#;

#[test]
fn lint_analyzes_every_declared_target_framework() {
    let root = lint_consumer("[\"//:quiet.globalconfig\"]");
    let greeter = root.join("core/Greeter.cs");
    write(&greeter, MULTI_TFM_GREETER);
    write(&root.join("core/BUILD.bazel"), MULTI_TFM_CORE_BUILD);

    let out = dx_command(&root, "lint", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    let shared: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("core/Greeter.cs:5:") && line.contains("CA1822"))
        .collect();
    assert_eq!(shared.len(), 1, "{text}");
    assert!(!shared[0].contains("[net"), "{}", shared[0]);
    let net8 = text
        .lines()
        .find(|line| line.contains("core/Greeter.cs:10:"))
        .unwrap_or_else(|| panic!("the net8.0-only member went unanalyzed: {text}"));
    assert!(net8.contains("CA1822"), "{net8}");
    assert!(net8.contains("[net8.0]"), "{net8}");
    assert_eq!(read(&greeter), MULTI_TFM_GREETER);
}
